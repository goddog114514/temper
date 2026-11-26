//! HTTP surface. Handlers stay thin: validate, call the core, persist, answer.

use std::collections::BTreeMap;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use temper_core::theory::{chord_quality, chord_tones, key_signature, scale_tones};
use temper_core::{analyze_pcs, identify_chords, search_voicings, VoicingRequest};

use crate::error::{ApiJson, AppError};
use crate::practice::{self, Spec};
use crate::state::{Device, SharedState};
use crate::store::{self, NewQuestion, NewSession, StateRow};

pub fn api_router() -> Router<SharedState> {
    Router::new()
        .route("/health", get(health))
        .route("/theory/catalog", get(theory_catalog))
        .route("/theory/analyze", post(theory_analyze))
        .route("/theory/voicings", get(theory_voicings))
        .route("/theory/chord", get(theory_chord))
        .route("/theory/scale", get(theory_scale))
        .route("/theory/key", get(theory_key))
        .route("/state", get(get_state).put(put_state_endpoint))
        .route("/progress", get(get_progress).put(put_progress))
        .route("/profile", get(profile))
        .route("/practice/sessions", post(start_session))
        .route("/practice/sessions/{id}", get(get_session))
        .route("/practice/sessions/{id}/answers", post(submit_answer))
        .route("/practice/sessions/{id}/finish", post(finish))
        .route("/practice/history", get(history))
        .route("/presets", get(list_presets).post(create_preset))
        .route("/presets/{id}", delete(remove_preset))
        .route("/shares", post(create_share))
        .route("/shares/{code}", get(get_share))
        .fallback(api_not_found)
}

pub fn page_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(index_page))
        .route("/studio", get(studio_page))
        .route("/practice", get(practice_page))
        .route("/profile", get(profile_page))
        .route("/s/{code}", get(share_page))
}

async fn api_not_found() -> AppError {
    AppError::not_found("API endpoint")
}

// ---------------------------------------------------------------- health

async fn health(State(state): State<SharedState>) -> Response {
    let db_ok = state.db.health();
    let status = if db_ok && state.native_ok { "ok" } else { "degraded" };
    let body = json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "core": temper_core::VERSION,
        "uptimeMs": state.uptime_ms(),
        "native": {
            "ok": state.native_ok,
            "abi": temper_core::native::abi_version(),
            "engine": "c++20",
        },
        "db": { "ok": db_ok, "engine": "sqlite/wal" },
    });
    (StatusCode::OK, Json(body)).into_response()
}

// ---------------------------------------------------------------- theory

async fn theory_catalog(State(state): State<SharedState>) -> Response {
    let mut response = Response::new(Body::from(state.catalog_json.clone()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    response
}

#[derive(Deserialize)]
struct AnalyzeBody {
    notes: Vec<i32>,
}

async fn theory_analyze(Json(body): Json<AnalyzeBody>) -> Result<Json<Value>, AppError> {
    if body.notes.is_empty() || body.notes.len() > 16 {
        return Err(AppError::validation("notes", "需要 1–16 个 MIDI 音符"));
    }
    if body.notes.iter().any(|n| !(0..=127).contains(n)) {
        return Err(AppError::validation("notes", "MIDI 音符需在 0–127"));
    }
    let pcs: Vec<i32> = body.notes.iter().map(|m| m.rem_euclid(12)).collect();
    let analysis = analyze_pcs(&pcs).map_err(native_error)?;
    let matched = identify_chords(&body.notes);
    Ok(Json(json!({
        "pitchClasses": pcs,
        "matched": matched,
        "analysis": analysis,
        "engine": "rust+cp",
    })))
}

#[derive(Deserialize)]
struct VoicingQuery {
    root: Option<u8>,
    quality: Option<String>,
    inversion: Option<u8>,
    #[serde(rename = "maxFret")]
    max_fret: Option<i32>,
    #[serde(rename = "maxSpan")]
    max_span: Option<i32>,
    #[serde(rename = "minStrings")]
    min_strings: Option<i32>,
    limit: Option<i32>,
}

async fn theory_voicings(Query(q): Query<VoicingQuery>) -> Result<Json<Value>, AppError> {
    let root = q.root.unwrap_or(0);
    if root > 11 {
        return Err(AppError::validation("root", "根音需为 0–11"));
    }
    let quality_id = q.quality.as_deref().unwrap_or("maj7");
    let inversion = q.inversion.unwrap_or(0).min(3);
    let tones = chord_tones(root, quality_id, inversion)
        .ok_or_else(|| AppError::validation("quality", "未知和弦性质"))?;
    let quality = chord_quality(quality_id)
        .ok_or_else(|| AppError::validation("quality", "未知和弦性质"))?;
    // The root must stay first: the C++ scorer uses chord_pcs[0] as the root.
    let mut pcs: Vec<i32> = vec![root as i32];
    for tone in &tones {
        let pc = tone.pc as i32;
        if !pcs.contains(&pc) {
            pcs.push(pc);
        }
    }
    let req = VoicingRequest {
        chord_pcs: pcs,
        max_fret: q.max_fret.unwrap_or(15).clamp(3, 24),
        max_span: q.max_span.unwrap_or(4).clamp(1, 8),
        min_strings: q.min_strings.unwrap_or(4).clamp(3, 6),
        max_results: q.limit.unwrap_or(12).clamp(1, 48),
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let voicings = search_voicings(&req).map_err(native_error)?;
    let symbol = chord_label(&tones, quality.symbol, inversion > 0);
    Ok(Json(json!({
        "root": root,
        "quality": quality_id,
        "inversion": inversion,
        "symbol": symbol,
        "tones": tones,
        "voicings": voicings,
        "engine": "cpp",
        "tookUs": started.elapsed().as_micros() as u64,
    })))
}

fn chord_label(tones: &[temper_core::theory::Tone], quality_symbol: &str, with_bass: bool) -> String {
    let root = tones
        .iter()
        .find(|t| t.degree == "1")
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let mut label = format!("{root}{quality_symbol}");
    if with_bass {
        if let Some(bass) = tones.first() {
            if bass.name != root {
                label.push('/');
                label.push_str(&bass.name);
            }
        }
    }
    label
}

#[derive(Deserialize)]
struct ChordQuery {
    root: Option<u8>,
    quality: Option<String>,
    inversion: Option<u8>,
}

async fn theory_chord(Query(q): Query<ChordQuery>) -> Result<Json<Value>, AppError> {
    let root = q.root.unwrap_or(0);
    if root > 11 {
        return Err(AppError::validation("root", "根音需为 0–11"));
    }
    let quality_id = q.quality.as_deref().unwrap_or("maj7");
    let inversion = q.inversion.unwrap_or(0);
    let tones = chord_tones(root, quality_id, inversion)
        .ok_or_else(|| AppError::validation("quality", "未知和弦性质"))?;
    let quality = chord_quality(quality_id).unwrap();
    let bass = tones.first().map(|t| t.name.clone()).unwrap_or_default();
    let inv = inversion.min((tones.len() - 1) as u8);
    let symbol = chord_label(&tones, quality.symbol, inv > 0);
    Ok(Json(json!({
        "root": root,
        "quality": { "id": quality.id, "label": quality.label, "symbol": quality.symbol },
        "inversion": inv,
        "symbol": symbol,
        "bass": bass,
        "tones": tones,
        "midis": tones.iter().map(|t| t.midi).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct ScaleQuery {
    tonic: Option<u8>,
    id: Option<String>,
}

async fn theory_scale(Query(q): Query<ScaleQuery>) -> Result<Json<Value>, AppError> {
    let tonic = q.tonic.unwrap_or(0);
    if tonic > 11 {
        return Err(AppError::validation("tonic", "主音需为 0–11"));
    }
    let id = q.id.as_deref().unwrap_or("major");
    let info = temper_core::theory::scale(id)
        .ok_or_else(|| AppError::validation("id", "未知音阶"))?;
    let tones = scale_tones(tonic, id).ok_or_else(|| AppError::validation("id", "未知音阶"))?;
    Ok(Json(json!({
        "id": info.id,
        "name": info.name,
        "steps": info.steps,
        "degrees": info.degrees,
        "desc": info.desc,
        "tones": tones,
        "midis": tones.iter().map(|t| t.midi).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct KeyQuery {
    pc: Option<u8>,
}

async fn theory_key(Query(q): Query<KeyQuery>) -> Result<Json<Value>, AppError> {
    let pc = q.pc.unwrap_or(0);
    if pc > 11 {
        return Err(AppError::validation("pc", "调式主音需为 0–11"));
    }
    Ok(Json(json!(key_signature(pc))))
}

fn native_error(err: temper_core::native::NativeError) -> AppError {
    match err {
        temper_core::native::NativeError::Invalid(message) => AppError::validation("query", message),
        other => {
            tracing::error!(error = %other, "native core failure");
            AppError::unavailable("和弦引擎暂时不可用")
        }
    }
}

// ---------------------------------------------------------------- state

async fn get_state(
    State(state): State<SharedState>,
    device: Device,
) -> Result<Json<Value>, AppError> {
    store::touch_user(&state.db, &device.0)?;
    let row = store::get_state(&state.db, &device.0)?;
    Ok(Json(state_json(&row)))
}

#[derive(Deserialize)]
struct StateBody {
    instrument: Option<String>,
    #[serde(rename = "keyPc")]
    key_pc: Option<u8>,
    tempo: Option<u16>,
    #[serde(rename = "presetId")]
    preset_id: Option<String>,
    degrees: Option<Vec<u8>>,
    sound: Option<bool>,
}

async fn put_state_endpoint(
    State(state): State<SharedState>,
    device: Device,
    ApiJson(body): ApiJson<StateBody>,
) -> Result<Json<Value>, AppError> {
    store::touch_user(&state.db, &device.0)?;
    let mut row = store::get_state(&state.db, &device.0)?;
    if let Some(instrument) = body.instrument {
        if instrument != "piano" && instrument != "guitar" {
            return Err(AppError::validation("instrument", "乐器需为 piano 或 guitar"));
        }
        row.instrument = instrument;
    }
    if let Some(key_pc) = body.key_pc {
        if key_pc > 11 {
            return Err(AppError::validation("keyPc", "调式主音需为 0–11"));
        }
        row.key_pc = key_pc;
    }
    if let Some(tempo) = body.tempo {
        if !(40..=240).contains(&tempo) {
            return Err(AppError::validation("tempo", "速度需为 40–240 BPM"));
        }
        row.tempo = tempo;
    }
    if let Some(preset) = body.preset_id {
        if preset.len() > 64 {
            return Err(AppError::validation("presetId", "预设标识过长"));
        }
        row.preset_id = Some(preset);
    }
    if let Some(degrees) = body.degrees {
        validate_degrees(&degrees)?;
        row.degrees = degrees;
    }
    if let Some(sound) = body.sound {
        row.sound = sound;
    }
    store::put_state(&state.db, &device.0, &row)?;
    Ok(Json(state_json(&row)))
}

fn validate_degrees(degrees: &[u8]) -> Result<(), AppError> {
    if degrees.is_empty() || degrees.len() > 16 {
        return Err(AppError::validation("degrees", "进行需要 1–16 个和弦"));
    }
    if degrees.iter().any(|d| *d > 6) {
        return Err(AppError::validation("degrees", "级数需为 0–6"));
    }
    Ok(())
}

fn state_json(row: &StateRow) -> Value {
    json!({
        "instrument": row.instrument,
        "keyPc": row.key_pc,
        "tempo": row.tempo,
        "presetId": row.preset_id,
        "degrees": row.degrees,
        "sound": row.sound,
        "updatedAt": row.updated_at,
    })
}

// ---------------------------------------------------------------- progress

async fn get_progress(
    State(state): State<SharedState>,
    device: Device,
) -> Result<Json<Value>, AppError> {
    store::touch_user(&state.db, &device.0)?;
    let items = store::get_progress(&state.db, &device.0)?;
    Ok(Json(json!({
        "items": items.into_iter().map(|(id, done)| json!({ "id": id, "done": done })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct ProgressItem {
    id: String,
    done: bool,
}

#[derive(Deserialize)]
struct ProgressBody {
    items: Vec<ProgressItem>,
}

async fn put_progress(
    State(state): State<SharedState>,
    device: Device,
    ApiJson(body): ApiJson<ProgressBody>,
) -> Result<Json<Value>, AppError> {
    if body.items.len() > 64 {
        return Err(AppError::validation("items", "最多 64 个条目"));
    }
    for item in &body.items {
        if item.id.is_empty() || item.id.len() > 64 {
            return Err(AppError::validation("items.id", "条目标识需为 1–64 字符"));
        }
    }
    store::touch_user(&state.db, &device.0)?;
    let pairs: Vec<(String, bool)> = body.items.iter().map(|i| (i.id.clone(), i.done)).collect();
    store::put_progress(&state.db, &device.0, &pairs)?;
    let done = pairs.iter().filter(|(_, d)| *d).count();
    Ok(Json(json!({ "items": pairs.len(), "done": done })))
}

// ---------------------------------------------------------------- profile

async fn profile(
    State(state): State<SharedState>,
    device: Device,
) -> Result<Json<Value>, AppError> {
    let user = store::touch_user(&state.db, &device.0)?;
    let stats = store::stats(&state.db, &device.0)?;
    let weak = store::weak_tags(&state.db, &device.0, 6)?;
    let recent = store::history(&state.db, &device.0, 8)?;
    let progress = store::get_progress(&state.db, &device.0)?;
    let accuracy = if stats.answered > 0 {
        (stats.correct as f64 / stats.answered as f64 * 1000.0).round() / 10.0
    } else {
        0.0
    };
    Ok(Json(json!({
        "user": { "id": user.id, "createdAt": user.created_at },
        "stats": {
            "sessions": stats.sessions,
            "answered": stats.answered,
            "correct": stats.correct,
            "accuracy": accuracy,
            "score": stats.score,
            "minutes": stats.minutes,
            "streakDays": stats.streak_days,
            "byMode": stats.by_mode.iter().map(|m| json!({
                "mode": m.mode,
                "sessions": m.sessions,
                "answered": m.answered,
                "correct": m.correct,
                "accuracy": if m.answered > 0 { (m.correct as f64 / m.answered as f64 * 1000.0).round() / 10.0 } else { 0.0 },
            })).collect::<Vec<_>>(),
        },
        "weakTags": weak.iter().map(|w| json!({
            "tag": w.tag,
            "attempts": w.attempts,
            "correct": w.correct,
            "accuracy": if w.attempts > 0 { (w.correct as f64 / w.attempts as f64 * 1000.0).round() / 10.0 } else { 0.0 },
        })).collect::<Vec<_>>(),
        "progress": progress.iter().map(|(id, done)| json!({ "id": id, "done": done })).collect::<Vec<_>>(),
        "recent": recent.iter().map(|h| json!({
            "id": h.id,
            "mode": h.mode,
            "difficulty": h.difficulty,
            "total": h.total,
            "answered": h.answered,
            "correct": h.correct,
            "score": h.score,
            "startedAt": h.started_at,
            "finishedAt": h.finished_at,
        })).collect::<Vec<_>>(),
    })))
}

// ---------------------------------------------------------------- practice

#[derive(Deserialize)]
struct StartSessionBody {
    mode: String,
    difficulty: Option<u8>,
    length: Option<u16>,
}

async fn start_session(
    State(state): State<SharedState>,
    device: Device,
    ApiJson(body): ApiJson<StartSessionBody>,
) -> Result<Json<Value>, AppError> {
    store::touch_user(&state.db, &device.0)?;
    let difficulty = body.difficulty.unwrap_or(2);
    let length = body.length.unwrap_or_else(|| practice::default_length(&body.mode));
    let spec = Spec { mode: body.mode.clone(), difficulty, length };
    let generated = practice::generate(&spec)?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let questions: Vec<NewQuestion> = generated
        .iter()
        .enumerate()
        .map(|(i, q)| NewQuestion {
            id: uuid::Uuid::new_v4().to_string(),
            index: i as u16,
            kind: q.kind.to_string(),
            prompt: q.prompt.clone(),
            answer: q.answer.clone(),
            tag: q.tag.clone(),
        })
        .collect();
    store::create_session(
        &state.db,
        &NewSession {
            id: &session_id,
            user: &device.0,
            mode: &spec.mode,
            difficulty,
            total: length,
        },
        &questions,
    )?;
    let public: Vec<Value> = questions
        .iter()
        .map(|q| {
            json!({
                "id": q.id,
                "index": q.index,
                "kind": q.kind,
                "prompt": q.prompt,
                "tag": q.tag,
            })
        })
        .collect();
    Ok(Json(json!({
        "session": {
            "id": session_id,
            "mode": spec.mode,
            "difficulty": difficulty,
            "total": length,
        },
        "questions": public,
    })))
}

async fn get_session(
    State(state): State<SharedState>,
    device: Device,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let session = load_session(&state, &device.0, &id)?;
    let questions = store::questions(&state.db, &id)?;
    Ok(Json(json!({
        "session": session_json(&session),
        "questions": questions.iter().map(question_public).collect::<Vec<_>>(),
        "summary": summary_json(&session, &questions),
    })))
}

fn load_session(state: &SharedState, user: &str, id: &str) -> Result<store::SessionRow, AppError> {
    let session = store::session(&state.db, id)?.ok_or_else(|| AppError::not_found("练习"))?;
    if session.user_id != user {
        return Err(AppError::not_found("练习"));
    }
    Ok(session)
}

fn session_json(session: &store::SessionRow) -> Value {
    json!({
        "id": session.id,
        "mode": session.mode,
        "difficulty": session.difficulty,
        "total": session.total,
        "answered": session.answered,
        "correct": session.correct,
        "score": session.score,
        "startedAt": session.started_at,
        "finishedAt": session.finished_at,
    })
}

fn question_public(q: &store::QuestionRow) -> Value {
    let mut value = json!({
        "id": q.id,
        "index": q.index,
        "kind": q.kind,
        "prompt": q.prompt,
        "tag": q.tag,
        "answered": q.answered,
    });
    if q.answered {
        if let Some(map) = value.as_object_mut() {
            map.insert("expected".into(), q.answer.clone());
        }
    }
    value
}

#[derive(Deserialize)]
struct AnswerBody {
    #[serde(rename = "questionId")]
    question_id: String,
    answer: Option<String>,
    #[serde(rename = "tapsMs")]
    taps_ms: Option<Vec<i64>>,
    #[serde(rename = "elapsedMs")]
    elapsed_ms: Option<i64>,
}

async fn submit_answer(
    State(state): State<SharedState>,
    device: Device,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<AnswerBody>,
) -> Result<Json<Value>, AppError> {
    let session = load_session(&state, &device.0, &id)?;
    if session.finished_at.is_some() {
        return Err(AppError::conflict("这组练习已经结束"));
    }
    let question = store::question(&state.db, &id, &body.question_id)?
        .ok_or_else(|| AppError::not_found("题目"))?;
    if question.answered {
        return Err(AppError::conflict("这道题已经作答"));
    }
    let elapsed = body.elapsed_ms.unwrap_or(0).clamp(0, 120_000);
    let score = practice::grade(
        &question,
        body.answer.as_deref(),
        body.taps_ms.as_deref(),
        session.difficulty,
    )?;
    let speed_bonus = if elapsed < 1_500 {
        5
    } else if elapsed < 3_000 {
        3
    } else {
        1
    };
    let awarded = if score.correct { 10 + speed_bonus } else { 0 };
    let response = json!({
        "answer": body.answer,
        "tapsMs": body.taps_ms,
        "elapsedMs": elapsed,
        "correct": score.correct,
    });
    if !store::record_answer(&state.db, &question.id, score.correct, elapsed, &response)? {
        return Err(AppError::conflict("这道题已经作答"));
    }
    store::bump_session(&state.db, &id, score.correct, awarded)?;
    let session = load_session(&state, &device.0, &id)?;
    let questions = store::questions(&state.db, &id)?;
    Ok(Json(json!({
        "correct": score.correct,
        "expected": score.expected,
        "explanation": score.explanation,
        "detail": score.detail,
        "awarded": awarded,
        "streak": trailing_streak(&questions),
        "session": session_json(&session),
    })))
}

fn trailing_streak(questions: &[store::QuestionRow]) -> u32 {
    let mut streak = 0;
    for q in questions.iter().rev() {
        match q.answered {
            false => continue,
            true => {
                if q.correct == Some(true) {
                    streak += 1;
                } else {
                    break;
                }
            }
        }
    }
    streak
}

async fn finish(
    State(state): State<SharedState>,
    device: Device,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let session = load_session(&state, &device.0, &id)?;
    if session.finished_at.is_none() {
        store::finish_session(&state.db, &id, session.score)?;
    }
    let session = load_session(&state, &device.0, &id)?;
    let questions = store::questions(&state.db, &id)?;
    Ok(Json(json!({
        "session": session_json(&session),
        "summary": summary_json(&session, &questions),
    })))
}

fn summary_json(session: &store::SessionRow, questions: &[store::QuestionRow]) -> Value {
    let answered = session.answered;
    let accuracy = if answered > 0 {
        (session.correct as f64 / answered as f64 * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let mut by_tag: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for q in questions.iter().filter(|q| q.answered) {
        let entry = by_tag.entry(q.tag.clone()).or_insert((0, 0));
        entry.0 += 1;
        if q.correct == Some(true) {
            entry.1 += 1;
        }
    }
    let tags: Vec<Value> = by_tag
        .into_iter()
        .map(|(tag, (attempts, correct))| {
            json!({
                "tag": tag,
                "attempts": attempts,
                "correct": correct,
                "accuracy": if attempts > 0 { (correct as f64 / attempts as f64 * 1000.0).round() / 10.0 } else { 0.0 },
            })
        })
        .collect();
    json!({
        "answered": answered,
        "correct": session.correct,
        "total": session.total,
        "accuracy": accuracy,
        "score": session.score,
        "finished": session.finished_at.is_some(),
        "remaining": session.total.saturating_sub(answered),
        "questionCount": questions.len(),
        "byTag": tags,
    })
}

async fn history(
    State(state): State<SharedState>,
    device: Device,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, AppError> {
    store::touch_user(&state.db, &device.0)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let rows = store::history(&state.db, &device.0, limit)?;
    Ok(Json(json!({
        "sessions": rows.iter().map(|h| json!({
            "id": h.id,
            "mode": h.mode,
            "difficulty": h.difficulty,
            "total": h.total,
            "answered": h.answered,
            "correct": h.correct,
            "score": h.score,
            "startedAt": h.started_at,
            "finishedAt": h.finished_at,
            "accuracy": if h.answered > 0 { (h.correct as f64 / h.answered as f64 * 1000.0).round() / 10.0 } else { 0.0 },
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<u32>,
}

// ---------------------------------------------------------------- presets

async fn list_presets(
    State(state): State<SharedState>,
    device: Device,
) -> Result<Json<Value>, AppError> {
    store::touch_user(&state.db, &device.0)?;
    let custom = store::list_presets(&state.db, &device.0)?;
    Ok(Json(json!({
        "builtIn": temper_core::PRESETS,
        "custom": custom.iter().map(|p| json!({
            "id": p.id,
            "name": p.name,
            "degrees": p.degrees,
            "keyPc": p.key_pc,
            "tempo": p.tempo,
            "createdAt": p.created_at,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct CreatePresetBody {
    name: String,
    degrees: Vec<u8>,
    #[serde(rename = "keyPc")]
    key_pc: Option<u8>,
    tempo: Option<u16>,
}

async fn create_preset(
    State(state): State<SharedState>,
    device: Device,
    ApiJson(body): ApiJson<CreatePresetBody>,
) -> Result<Json<Value>, AppError> {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 48 {
        return Err(AppError::validation("name", "名称需为 1–48 个字符"));
    }
    validate_degrees(&body.degrees)?;
    let key_pc = body.key_pc.unwrap_or(0);
    if key_pc > 11 {
        return Err(AppError::validation("keyPc", "调式主音需为 0–11"));
    }
    let tempo = body.tempo.unwrap_or(96);
    if !(40..=240).contains(&tempo) {
        return Err(AppError::validation("tempo", "速度需为 40–240 BPM"));
    }
    store::touch_user(&state.db, &device.0)?;
    let row = store::create_preset(&state.db, &device.0, name, &body.degrees, key_pc, tempo)?;
    Ok(Json(json!({
        "id": row.id,
        "name": row.name,
        "degrees": row.degrees,
        "keyPc": row.key_pc,
        "tempo": row.tempo,
        "createdAt": row.created_at,
    })))
}

async fn remove_preset(
    State(state): State<SharedState>,
    device: Device,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    store::touch_user(&state.db, &device.0)?;
    if store::delete_preset(&state.db, &device.0, &id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("预设"))
    }
}

// ---------------------------------------------------------------- shares

#[derive(Deserialize)]
struct ShareBody {
    title: Option<String>,
    #[serde(rename = "keyPc")]
    key_pc: Option<u8>,
    tempo: Option<u16>,
    #[serde(rename = "presetId")]
    preset_id: Option<String>,
    degrees: Vec<u8>,
    instrument: Option<String>,
}

async fn create_share(
    State(state): State<SharedState>,
    device: Device,
    ApiJson(body): ApiJson<ShareBody>,
) -> Result<Json<Value>, AppError> {
    validate_degrees(&body.degrees)?;
    let key_pc = body.key_pc.unwrap_or(0);
    if key_pc > 11 {
        return Err(AppError::validation("keyPc", "调式主音需为 0–11"));
    }
    let tempo = body.tempo.unwrap_or(96);
    if !(40..=240).contains(&tempo) {
        return Err(AppError::validation("tempo", "速度需为 40–240 BPM"));
    }
    let instrument = body.instrument.unwrap_or_else(|| "piano".into());
    if instrument != "piano" && instrument != "guitar" {
        return Err(AppError::validation("instrument", "乐器需为 piano 或 guitar"));
    }
    let key_name = key_signature(key_pc).key_name;
    let title = body
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(80).collect::<String>())
        .unwrap_or_else(|| format!("{key_name} 大调 · {} 小节", body.degrees.len()));
    let payload = json!({
        "keyPc": key_pc,
        "keyName": key_name,
        "tempo": tempo,
        "presetId": body.preset_id,
        "degrees": body.degrees,
        "instrument": instrument,
    });
    store::touch_user(&state.db, &device.0)?;
    let code = store::create_share(&state.db, &device.0, &title, &payload)?;
    Ok(Json(json!({
        "code": code,
        "url": format!("/s/{code}"),
        "title": title,
        "payload": payload,
    })))
}

async fn get_share(
    State(state): State<SharedState>,
    Path(code): Path<String>,
) -> Result<Json<Value>, AppError> {
    if code.len() < 4 || code.len() > 32 || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(AppError::validation("code", "分享码格式不正确"));
    }
    let share = store::get_share(&state.db, &code)?
        .ok_or_else(|| AppError::not_found("分享"))?;
    Ok(Json(json!({
        "code": share.code,
        "title": share.title,
        "payload": share.payload,
        "createdAt": share.created_at,
        "views": share.views,
    })))
}

// ---------------------------------------------------------------- pages

async fn index_page(State(state): State<SharedState>) -> Response {
    serve_page(&state, "index.html").await
}

async fn studio_page(State(state): State<SharedState>) -> Response {
    serve_page(&state, "studio.html").await
}

async fn practice_page(State(state): State<SharedState>) -> Response {
    serve_page(&state, "practice.html").await
}

async fn profile_page(State(state): State<SharedState>) -> Response {
    serve_page(&state, "profile.html").await
}

async fn share_page(State(state): State<SharedState>, Path(_code): Path<String>) -> Response {
    serve_page(&state, "share.html").await
}

async fn serve_page(state: &SharedState, name: &str) -> Response {
    let path = state.config.static_dir.join(name);
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, path = %path.display(), "page missing");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "页面文件缺失：请检查 TEMPER_STATIC 指向 web/",
            )
                .into_response()
        }
    }
}

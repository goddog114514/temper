//! End-to-end API tests through the real router, a real SQLite file and the
//! real C++ core. No mocks.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use temper_server::state::{AppState, Config};
use temper_server::store::Db;
use tower::ServiceExt;

struct Harness {
    app: Router,
    _dir: tempfile::TempDir,
    web: PathBuf,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().expect("temp dir");
    let web = dir.path().join("web");
    std::fs::create_dir_all(&web).unwrap();
    std::fs::write(web.join("index.html"), "<h1>调律 TEMPER</h1>").unwrap();
    std::fs::write(web.join("studio.html"), "<h1>工坊</h1>").unwrap();
    std::fs::write(web.join("practice.html"), "<h1>练习</h1>").unwrap();
    std::fs::write(web.join("profile.html"), "<h1>档案</h1>").unwrap();
    std::fs::write(web.join("share.html"), "<h1>分享</h1>").unwrap();

    let config = Config {
        addr: "127.0.0.1:0".parse().unwrap(),
        db_path: dir.path().join("temper.db"),
        static_dir: web.clone(),
        cors: false,
    };
    let state = AppState::new(config).expect("state");
    // Sanity: the pool and migrations are usable before serving.
    {
        let db = Db::open(dir.path().join("probe.db")).unwrap();
        assert!(db.health());
    }
    Harness { app: temper_server::app(state), _dir: dir, web }
}

fn request(method: Method, uri: &str, device: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(device) = device {
        builder = builder.header("x-temper-device", device);
    }
    match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(req).await.expect("response");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, value)
}

const DEVICE: &str = "test-device-001";
const OTHER: &str = "test-device-002";

#[tokio::test]
async fn health_and_catalog() {
    let h = harness();
    let (status, body) = send(&h.app, request(Method::GET, "/api/health", None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["native"]["ok"], true);
    assert_eq!(body["db"]["ok"], true);

    let (status, body) = send(&h.app, request(Method::GET, "/api/theory/catalog", None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["chords"].as_array().unwrap().len(), 11);
    assert_eq!(body["presets"].as_array().unwrap().len(), 7);
    assert_eq!(body["keys"].as_array().unwrap().len(), 12);
    assert_eq!(body["tuning"], json!([40, 45, 50, 55, 59, 64]));
}

#[tokio::test]
async fn voicings_come_from_the_native_core() {
    let h = harness();
    let (status, body) = send(
        &h.app,
        request(Method::GET, "/api/theory/voicings?root=0&quality=maj7&limit=6", None, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["engine"], "cpp");
    assert_eq!(body["symbol"], "Cmaj7");
    let voicings = body["voicings"].as_array().unwrap();
    assert!(!voicings.is_empty());
    assert!(voicings.len() <= 6);
    assert!(voicings[0]["keys"].is_array());

    let (status, body) = send(
        &h.app,
        request(Method::GET, "/api/theory/voicings?root=0&quality=nope", None, None),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_failed");
}

#[tokio::test]
async fn analyze_identifies_chords_and_intervals() {
    let h = harness();
    let (status, body) = send(
        &h.app,
        request(Method::POST, "/api/theory/analyze", None, Some(json!({ "notes": [60, 64, 67, 71] }))),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["matched"][0]["symbol"], "Cmaj7");
    assert_eq!(body["analysis"]["cardinality"], 4);
    assert_eq!(body["analysis"]["intervalVector"], json!([1, 0, 1, 2, 2, 0]));
}

#[tokio::test]
async fn state_is_persisted_per_device() {
    let h = harness();
    let (status, _) = send(&h.app, request(Method::GET, "/api/state", None, None)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send(&h.app, request(Method::GET, "/api/state", Some(DEVICE), None)).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &h.app,
        request(
            Method::PUT,
            "/api/state",
            Some(DEVICE),
            Some(json!({ "instrument": "guitar", "keyPc": 5, "tempo": 104, "degrees": [0,4,5,3,0,2,3,4], "presetId": "canon" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["instrument"], "guitar");
    assert_eq!(body["degrees"].as_array().unwrap().len(), 8);

    let (_, body) = send(&h.app, request(Method::GET, "/api/state", Some(DEVICE), None)).await;
    assert_eq!(body["tempo"], 104);

    let (_, body) = send(&h.app, request(Method::GET, "/api/state", Some(OTHER), None)).await;
    assert_eq!(body["instrument"], "piano");

    let (status, body) = send(
        &h.app,
        request(Method::PUT, "/api/state", Some(DEVICE), Some(json!({ "tempo": 999 }))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["details"]["field"], "tempo");
}

#[tokio::test]
async fn practice_session_lifecycle() {
    let h = harness();
    let (status, body) = send(
        &h.app,
        request(
            Method::POST,
            "/api/practice/sessions",
            Some(DEVICE),
            Some(json!({ "mode": "interval", "difficulty": 2, "length": 4 })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session_id = body["session"]["id"].as_str().unwrap().to_string();
    let questions = body["questions"].as_array().unwrap().clone();
    assert_eq!(questions.len(), 4);
    assert!(questions[0]["prompt"]["choices"].as_array().unwrap().len() >= 3);

    let mut answered = 0;
    for q in &questions {
        let (status, result) = send(
            &h.app,
            request(
                Method::POST,
                &format!("/api/practice/sessions/{session_id}/answers"),
                Some(DEVICE),
                Some(json!({
                    "questionId": q["id"],
                    "answer": q["prompt"]["choices"][0]["id"],
                    "elapsedMs": 1800
                })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert!(result["expected"]["id"].is_string());
        assert!(result["explanation"].is_string());
        answered += 1;
    }
    assert_eq!(answered, 4);

    // Duplicate answers are rejected.
    let (status, body) = send(
        &h.app,
        request(
            Method::POST,
            &format!("/api/practice/sessions/{session_id}/answers"),
            Some(DEVICE),
            Some(json!({ "questionId": questions[0]["id"], "answer": "M3" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "conflict");

    let (status, body) = send(
        &h.app,
        request(
            Method::POST,
            &format!("/api/practice/sessions/{session_id}/finish"),
            Some(DEVICE),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["summary"]["answered"], 4);
    assert_eq!(body["summary"]["finished"], true);

    // Another device cannot read the session.
    let (status, _) = send(
        &h.app,
        request(Method::GET, &format!("/api/practice/sessions/{session_id}"), Some(OTHER), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, profile) = send(&h.app, request(Method::GET, "/api/profile", Some(DEVICE), None)).await;
    assert_eq!(profile["stats"]["sessions"], 1);
    assert_eq!(profile["stats"]["answered"], 4);
    assert_eq!(profile["stats"]["streakDays"], 1);
}

#[tokio::test]
async fn rhythm_session_scores_taps() {
    let h = harness();
    let (_, body) = send(
        &h.app,
        request(
            Method::POST,
            "/api/practice/sessions",
            Some(DEVICE),
            Some(json!({ "mode": "rhythm", "difficulty": 1, "length": 4 })),
        ),
    )
    .await;
    let session_id = body["session"]["id"].as_str().unwrap().to_string();
    for q in body["questions"].as_array().unwrap() {
        // Perfect taps: derive from the public pattern.
        let bpm = q["prompt"]["bpm"].as_f64().unwrap();
        let pattern = q["prompt"]["pattern"].as_array().unwrap();
        let eighth = 60_000.0 / bpm / 2.0;
        let taps: Vec<i64> = pattern
            .iter()
            .enumerate()
            .filter(|(_, v)| v.as_i64() == Some(1))
            .map(|(i, _)| (i as f64 * eighth).round() as i64)
            .collect();
        let (status, result) = send(
            &h.app,
            request(
                Method::POST,
                &format!("/api/practice/sessions/{session_id}/answers"),
                Some(DEVICE),
                Some(json!({ "questionId": q["id"], "tapsMs": taps })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["correct"], true, "{result}");
        assert!(result["detail"]["hits"].as_u64().unwrap() >= 1);
    }
}

#[tokio::test]
async fn shares_roundtrip_and_404() {
    let h = harness();
    let (status, body) = send(
        &h.app,
        request(
            Method::POST,
            "/api/shares",
            Some(DEVICE),
            Some(json!({
                "keyPc": 2, "tempo": 104, "degrees": [0,4,5,2,3,0,3,4], "presetId": "canon"
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let code = body["code"].as_str().unwrap().to_string();
    assert!(code.len() >= 5);

    let (status, body) = send(&h.app, request(Method::GET, &format!("/api/shares/{code}"), None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["payload"]["keyPc"], 2);
    assert_eq!(body["payload"]["degrees"].as_array().unwrap().len(), 8);
    assert_eq!(body["views"], 0);

    let (status, body) = send(&h.app, request(Method::GET, "/api/shares/zzzzzz", None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn custom_presets_and_progress() {
    let h = harness();
    let (status, body) = send(
        &h.app,
        request(
            Method::PUT,
            "/api/progress",
            Some(DEVICE),
            Some(json!({ "items": [{ "id": "circle", "done": true }, { "id": "intervals", "done": false }] })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["done"], 1);

    let (_, body) = send(&h.app, request(Method::GET, "/api/progress", Some(DEVICE), None)).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    let (status, body) = send(
        &h.app,
        request(
            Method::POST,
            "/api/presets",
            Some(DEVICE),
            Some(json!({ "name": "夜里的 1564", "degrees": [0,4,5,3], "keyPc": 9, "tempo": 72 })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let preset_id = body["id"].as_str().unwrap().to_string();

    let (_, body) = send(&h.app, request(Method::GET, "/api/presets", Some(DEVICE), None)).await;
    assert_eq!(body["builtIn"].as_array().unwrap().len(), 7);
    assert_eq!(body["custom"].as_array().unwrap().len(), 1);

    let (status, _) = send(
        &h.app,
        request(Method::DELETE, &format!("/api/presets/{preset_id}"), Some(DEVICE), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn pages_and_unknown_api() {
    let h = harness();
    for page in ["/", "/studio", "/practice", "/profile", "/s/abc123"] {
        let response = h
            .app
            .clone()
            .oneshot(request(Method::GET, page, None, None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "page {page}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
    }
    let (status, body) = send(&h.app, request(Method::GET, "/api/unknown", None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(h.web.exists());
}

#[tokio::test]
async fn malformed_json_returns_envelope() {
    let h = harness();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/practice/sessions")
        .header("x-temper-device", DEVICE)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{not json"))
        .unwrap();
    let (status, body) = send(&h.app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_json");
}

//! Practice engine: question generation, answer scoring, difficulty rules.
//!
//! Answers never leave the server before the question is answered; the public
//! payload contains only what the learner needs to hear or see.

use rand::seq::SliceRandom;
use rand::Rng;
use serde_json::{json, Value};

use temper_core::theory::{chord_tones, note_name, pc_name, scale_tones, CHORDS, INTERVALS, SCALES};

use crate::error::AppError;
use crate::store::QuestionRow;

pub const MODES: &[&str] = &["interval", "chord", "scale", "sight", "rhythm"];

pub const DEFAULT_LENGTH: u16 = 8;

#[derive(Debug, Clone)]
pub struct Generated {
    pub kind: &'static str,
    pub prompt: Value,
    pub answer: Value,
    pub tag: String,
}

pub struct Spec {
    pub mode: String,
    pub difficulty: u8,
    pub length: u16,
}

impl Spec {
    pub fn validate(&self) -> Result<(), AppError> {
        if !MODES.contains(&self.mode.as_str()) {
            return Err(AppError::validation("mode", "未知的练习类型"));
        }
        if !(1..=3).contains(&self.difficulty) {
            return Err(AppError::validation("difficulty", "难度需为 1–3"));
        }
        if !(4..=20).contains(&self.length) {
            return Err(AppError::validation("length", "题目数量需为 4–20"));
        }
        Ok(())
    }
}

pub fn default_length(mode: &str) -> u16 {
    match mode {
        "rhythm" => 6,
        "sight" => 10,
        _ => DEFAULT_LENGTH,
    }
}

pub fn generate(spec: &Spec) -> Result<Vec<Generated>, AppError> {
    spec.validate()?;
    let mut out = Vec::with_capacity(spec.length as usize);
    let mut rng = rand::thread_rng();
    for _ in 0..spec.length {
        let q = match spec.mode.as_str() {
            "interval" => interval_question(spec.difficulty, &mut rng),
            "chord" => chord_question(spec.difficulty, &mut rng),
            "scale" => scale_question(spec.difficulty, &mut rng),
            "sight" => sight_question(spec.difficulty, &mut rng),
            "rhythm" => rhythm_question(spec.difficulty, &mut rng),
            other => return Err(AppError::validation("mode", format!("未知模式 {other}"))),
        };
        out.push(q);
    }
    Ok(out)
}

fn choice_count(difficulty: u8) -> usize {
    match difficulty {
        1 => 3,
        2 => 4,
        _ => 5,
    }
}

fn shuffle_choices(correct: (String, String), pool: Vec<(String, String)>, n: usize, rng: &mut impl Rng) -> Vec<Value> {
    let mut others: Vec<(String, String)> = pool.into_iter().filter(|(id, _)| *id != correct.0).collect();
    others.shuffle(rng);
    let mut choices = Vec::with_capacity(n);
    choices.push(correct);
    choices.extend(others.into_iter().take(n.saturating_sub(1)));
    choices.shuffle(rng);
    choices
        .into_iter()
        .map(|(id, label)| json!({ "id": id, "label": label }))
        .collect()
}

fn interval_question(difficulty: u8, rng: &mut impl Rng) -> Generated {
    // Prefer the intervals learners actually meet; include the octave only at
    // the top difficulty so the answer set stays honest.
    let pool: Vec<_> = INTERVALS
        .iter()
        .filter(|i| i.semitones > 0 && (i.semitones < 12 || difficulty >= 3))
        .collect();
    let target = pool[rng.gen_range(0..pool.len())];
    let base = rng.gen_range(55..=69);
    let ascending = rng.gen_bool(0.5);
    let (low, high) = if ascending { (base, base + target.semitones as i32) } else { (base + target.semitones as i32, base) };
    let choices = shuffle_choices(
        (target.id.to_string(), target.short.to_string()),
        pool.iter().map(|i| (i.id.to_string(), i.short.to_string())).collect(),
        choice_count(difficulty),
        rng,
    );
    Generated {
        kind: "interval",
        prompt: json!({
            "root": low.min(high),
            "top": low.max(high),
            "melodic": !ascending,
            "choices": choices,
            "hint": "先听低音，再判断两音之间的距离",
        }),
        answer: json!({ "id": target.id, "semitones": target.semitones, "name": target.short }),
        tag: format!("interval:{}", target.id),
    }
}

fn chord_question(difficulty: u8, rng: &mut impl Rng) -> Generated {
    let qualities: Vec<_> = if difficulty == 1 {
        CHORDS.iter().filter(|q| q.intervals.len() == 3).collect()
    } else {
        CHORDS.iter().collect()
    };
    let quality = qualities[rng.gen_range(0..qualities.len())];
    let root = rng.gen_range(0..12u8);
    let midis: Vec<i32> = quality
        .intervals
        .iter()
        .map(|semi| 60 + root as i32 + *semi as i32)
        .collect();
    let pool: Vec<(String, String)> = qualities
        .iter()
        .map(|q| (q.id.to_string(), q.label.to_string()))
        .collect();
    let choices = shuffle_choices(
        (quality.id.to_string(), quality.label.to_string()),
        pool,
        choice_count(difficulty),
        rng,
    );
    Generated {
        kind: "chord",
        prompt: json!({
            "rootPc": root,
            "midis": midis,
            "choices": choices,
            "hint": "听最低音与三音的距离，再判断七音",
        }),
        answer: json!({
            "id": quality.id,
            "label": quality.label,
            "symbol": format!("{}{}", pc_name(root), quality.symbol),
            "tones": chord_tones(root, quality.id, 0).map(|t| t.into_iter().map(|t| t.name).collect::<Vec<_>>()).unwrap_or_default(),
        }),
        tag: format!("chord:{}", quality.id),
    }
}

fn scale_question(difficulty: u8, rng: &mut impl Rng) -> Generated {
    let pool: Vec<_> = if difficulty == 1 {
        SCALES.iter().filter(|s| s.id == "major" || s.id == "minor").collect()
    } else {
        SCALES.iter().collect()
    };
    let scale = pool[rng.gen_range(0..pool.len())];
    let tonic = rng.gen_range(0..12u8);
    let tones = scale_tones(tonic, scale.id).unwrap_or_default();
    let midis: Vec<i32> = tones.iter().map(|t| t.midi).collect();
    let choices = shuffle_choices(
        (scale.id.to_string(), scale.name.to_string()),
        SCALES.iter().map(|s| (s.id.to_string(), s.name.to_string())).collect(),
        choice_count(difficulty),
        rng,
    );
    Generated {
        kind: "scale",
        prompt: json!({
            "tonicPc": tonic,
            "midis": midis,
            "choices": choices,
            "hint": "先找主音，再听第三、第六与第七级",
        }),
        answer: json!({
            "id": scale.id,
            "name": scale.name,
            "formula": scale.steps,
            "notes": tones.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
        }),
        tag: format!("scale:{}", scale.id),
    }
}

fn sight_question(difficulty: u8, rng: &mut impl Rng) -> Generated {
    let (lo, hi) = match difficulty {
        1 => (60, 77),
        2 => (55, 81),
        _ => (48, 84),
    };
    let midi = rng.gen_range(lo..=hi);
    let clef = if midi < 60 { "bass" } else { "treble" };
    let correct = note_name(midi);
    let mut pool: Vec<(String, String)> = Vec::new();
    let mut candidates: Vec<i32> = (lo..=hi).filter(|m| (m % 12) != (midi % 12)).collect();
    candidates.shuffle(rng);
    for candidate in candidates.into_iter().take(8) {
        let name = note_name(candidate);
        pool.push((name.clone(), name));
    }
    let choices = shuffle_choices(
        (correct.clone(), correct.clone()),
        pool,
        choice_count(difficulty),
        rng,
    );
    Generated {
        kind: "sight",
        prompt: json!({
            "midi": midi,
            "clef": clef,
            "choices": choices,
            "hint": if clef == "treble" { "高音谱表：一线 E4，间音 F4 A4 C5 E5" } else { "低音谱表：一线 G2，间音 A2 C3 E3 G3" },
        }),
        answer: json!({ "name": correct, "midi": midi, "pc": midi % 12 }),
        tag: if clef == "treble" { "sight:treble".to_string() } else { "sight:bass".to_string() },
    }
}

fn rhythm_question(difficulty: u8, rng: &mut impl Rng) -> Generated {
    let meter = match difficulty {
        1 => "4/4",
        2 => ["4/4", "3/4", "6/8"].choose(rng).copied().unwrap_or("4/4"),
        _ => ["4/4", "3/4", "6/8", "5/4", "7/8"].choose(rng).copied().unwrap_or("4/4"),
    };
    let beats: usize = meter.split('/').next().and_then(|b| b.parse().ok()).unwrap_or(4);
    let grid = beats * 2; // eighth-note grid
    let bpm = match difficulty {
        1 => 80,
        2 => 96,
        _ => 112,
    };
    let density = match difficulty {
        1 => 0.5,
        2 => 0.65,
        _ => 0.75,
    };
    let mut pattern = vec![0u8; grid];
    // Always mark the downbeat, then scatter hits; guarantee at least 4 hits.
    pattern[0] = 1;
    for slot in pattern.iter_mut().skip(1) {
        if rng.gen_bool(density) {
            *slot = 1;
        }
    }
    if pattern.iter().filter(|v| **v == 1).count() < 4 {
        for slot in pattern.iter_mut().skip(1).take(3) {
            *slot = 1;
        }
    }
    let eighth_ms = 60_000f64 / bpm as f64 / 2.0;
    let expected: Vec<i64> = pattern
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == 1)
        .map(|(i, _)| (i as f64 * eighth_ms).round() as i64)
        .collect();
    Generated {
        kind: "rhythm",
        prompt: json!({
            "bpm": bpm,
            "meter": meter,
            "grid": grid,
            "pattern": pattern,
            "count": expected.len(),
            "hint": "按空格或点击节拍垫，跟随重音",
        }),
        answer: json!({
            "expectedMs": expected,
            "bpm": bpm,
            "meter": meter,
        }),
        tag: format!("rhythm:{meter}"),
    }
}

#[derive(Debug, Clone)]
pub struct Score {
    pub correct: bool,
    pub expected: Value,
    pub explanation: String,
    pub detail: Value,
}

pub fn grade(question: &QuestionRow, answer: Option<&str>, taps: Option<&[i64]>, difficulty: u8) -> Result<Score, AppError> {
    match question.kind.as_str() {
        "rhythm" => grade_rhythm(question, taps, difficulty),
        _ => grade_choice(question, answer),
    }
}

fn grade_choice(question: &QuestionRow, answer: Option<&str>) -> Result<Score, AppError> {
    let submitted = answer
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation("answer", "缺少答案"))?;
    let expected_id = question
        .answer
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| question.answer.get("name").and_then(|v| v.as_str()))
        .unwrap_or_default();
    let correct = submitted.eq_ignore_ascii_case(expected_id)
        || submitted == question.answer.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let explanation = explanation_for(question, &question.answer);
    Ok(Score {
        correct,
        expected: question.answer.clone(),
        explanation,
        detail: json!({ "submitted": submitted, "expectedId": expected_id }),
    })
}

fn explanation_for(question: &QuestionRow, answer: &Value) -> String {
    match question.kind.as_str() {
        "interval" => {
            let name = answer.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let semis = answer.get("semitones").and_then(|v| v.as_u64()).unwrap_or(0);
            format!("{name} = {semis} 个半音")
        }
        "chord" => {
            let symbol = answer.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            let tones = answer
                .get("tones")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" · "))
                .unwrap_or_default();
            format!("{symbol}：{tones}")
        }
        "scale" => {
            let name = answer.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let notes = answer
                .get("notes")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" · "))
                .unwrap_or_default();
            format!("{name}：{notes}")
        }
        "sight" => {
            let name = answer.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let midi = answer.get("midi").and_then(|v| v.as_i64()).unwrap_or(0);
            format!("{name}（MIDI {midi}）")
        }
        _ => String::new(),
    }
}

fn grade_rhythm(question: &QuestionRow, taps: Option<&[i64]>, difficulty: u8) -> Result<Score, AppError> {
    let taps = taps.ok_or_else(|| AppError::validation("tapsMs", "缺少节拍数据"))?;
    if taps.len() > 64 {
        return Err(AppError::validation("tapsMs", "节拍数据过多"));
    }
    let expected: Vec<i64> = question
        .answer
        .get("expectedMs")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();
    if expected.is_empty() {
        return Err(AppError::internal("节奏题缺少参考答案"));
    }
    let tolerance = match difficulty {
        1 => 140,
        2 => 100,
        _ => 70,
    };
    let mut used = vec![false; taps.len()];
    let mut deviations = Vec::with_capacity(expected.len());
    let mut hits = 0usize;
    for target in &expected {
        let mut best: Option<(usize, i64)> = None;
        for (i, tap) in taps.iter().enumerate() {
            if used[i] {
                continue;
            }
            let delta = tap - target;
            if best.is_none_or(|(_, d)| delta.abs() < d.abs()) {
                best = Some((i, delta));
            }
        }
        if let Some((index, delta)) = best {
            used[index] = true;
            deviations.push(delta);
            if delta.abs() <= tolerance {
                hits += 1;
            }
        } else {
            deviations.push(i64::MAX);
        }
    }
    let hit_ratio = hits as f64 / expected.len() as f64;
    let correct = hit_ratio >= 0.8 && (taps.len() as i64 - expected.len() as i64).abs() <= 2;
    let avg: i64 = deviations
        .iter()
        .filter(|d| **d != i64::MAX)
        .copied()
        .sum::<i64>()
        / deviations.iter().filter(|d| **d != i64::MAX).count().max(1) as i64;
    let trend = if avg > 15 { "偏晚" } else if avg < -15 { "偏早" } else { "稳定" };
    Ok(Score {
        correct,
        expected: question.answer.clone(),
        explanation: format!("命中 {hits}/{} 拍，平均偏差 {avg:+} ms（{trend}）", expected.len()),
        detail: json!({
            "hits": hits,
            "total": expected.len(),
            "toleranceMs": tolerance,
            "deviations": deviations,
            "averageMs": avg,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_every_mode_with_choices() {
        for mode in MODES {
            let spec = Spec { mode: mode.to_string(), difficulty: 2, length: 4 };
            let questions = generate(&spec).expect("generate");
            assert_eq!(questions.len(), 4);
            for q in questions {
                assert!(q.tag.contains(':'));
                assert!(q.prompt.get("hint").is_some());
                if *mode != "rhythm" {
                    let choices = q.prompt.get("choices").and_then(|c| c.as_array()).unwrap();
                    assert!(choices.len() >= 3);
                    let expected_id = q.answer.get("id").or_else(|| q.answer.get("name")).and_then(|v| v.as_str()).unwrap();
                    assert!(choices.iter().any(|c| c["id"] == expected_id));
                } else {
                    assert!(q.answer.get("expectedMs").is_some());
                }
            }
        }
    }

    #[test]
    fn rejects_unknown_mode() {
        let spec = Spec { mode: "nope".into(), difficulty: 1, length: 4 };
        assert!(generate(&spec).is_err());
    }

    #[test]
    fn rhythm_grading_rewards_accuracy() {
        let expected = [0i64, 500, 1000, 1500];
        let question = QuestionRow {
            id: "q".into(),
            session_id: "s".into(),
            index: 0,
            kind: "rhythm".into(),
            prompt: json!({}),
            answer: json!({ "expectedMs": expected }),
            tag: "rhythm:4/4".into(),
            answered: false,
            correct: None,
        };
        let tight = grade(&question, None, Some(&[10, 495, 1008, 1490]), 2).unwrap();
        assert!(tight.correct);
        let loose = grade(&question, None, Some(&[300, 900, 1400, 2000]), 2).unwrap();
        assert!(!loose.correct);
    }

    #[test]
    fn choice_grading_accepts_note_names() {
        let question = QuestionRow {
            id: "q".into(),
            session_id: "s".into(),
            index: 0,
            kind: "sight".into(),
            prompt: json!({}),
            answer: json!({ "name": "E4", "midi": 64 }),
            tag: "sight:treble".into(),
            answered: false,
            correct: None,
        };
        assert!(grade(&question, Some("E4"), None, 1).unwrap().correct);
        assert!(!grade(&question, Some("F4"), None, 1).unwrap().correct);
    }
}

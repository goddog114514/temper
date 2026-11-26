//! Safe, allocation-bounded wrappers around the C++ core.
//!
//! Concurrency: the C++ functions are pure and reentrant, so there is no lock
//! on this path. Rust owns the buffers; C++ fills them and returns a count.

use std::fmt;
use std::sync::OnceLock;

use serde::Serialize;

use crate::ffi::{
    self, NativeAnalysis, NativeVoicing, ABI_VERSION, MAX_RESULTS, MAX_STRINGS,
};
use crate::theory::STANDARD_TUNING;

static ABI_CHECK: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeError {
    Unavailable(String),
    Invalid(String),
    Status { code: i32, detail: String },
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NativeError::Unavailable(msg) => write!(f, "native core unavailable: {msg}"),
            NativeError::Invalid(msg) => write!(f, "invalid native request: {msg}"),
            NativeError::Status { code, detail } => {
                write!(f, "native core rejected the call ({code}): {detail}")
            }
        }
    }
}

impl std::error::Error for NativeError {}

fn ensure_abi() -> Result<(), NativeError> {
    let result = ABI_CHECK.get_or_init(|| {
        // SAFETY: no arguments, no allocation, constant return value.
        let version = unsafe { ffi::temper_abi_version() };
        if version == ABI_VERSION {
            Ok(())
        } else {
            Err(format!(
                "ABI mismatch: header {ABI_VERSION}, library {version}"
            ))
        }
    });
    result.clone().map_err(NativeError::Unavailable)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voicing {
    pub frets: Vec<i32>,
    pub string_count: i32,
    pub lo_string: i32,
    pub hi_string: i32,
    pub span: i32,
    pub max_fret: i32,
    pub bass_pc: i32,
    pub note_count: i32,
    pub score: i32,
    pub midis: Vec<i32>,
    /// UI keys, high string first: "6:0" means low E open.
    pub keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VoicingRequest {
    pub tuning: Vec<i32>,
    pub chord_pcs: Vec<i32>,
    pub max_fret: i32,
    pub max_span: i32,
    pub min_strings: i32,
    pub max_results: i32,
}

impl Default for VoicingRequest {
    fn default() -> Self {
        Self {
            tuning: STANDARD_TUNING.to_vec(),
            chord_pcs: vec![0, 4, 7],
            max_fret: 15,
            max_span: 4,
            min_strings: 4,
            max_results: 12,
        }
    }
}

pub fn search_voicings(req: &VoicingRequest) -> Result<Vec<Voicing>, NativeError> {
    ensure_abi()?;
    if !(2..=MAX_STRINGS).contains(&req.tuning.len()) {
        return Err(NativeError::Invalid(format!(
            "tuning must have 2..={MAX_STRINGS} strings"
        )));
    }
    if req.chord_pcs.is_empty() || req.chord_pcs.len() > 12 {
        return Err(NativeError::Invalid("chord needs 1..=12 pitch classes".into()));
    }
    if req.chord_pcs.iter().any(|p| !(0..12).contains(p)) {
        return Err(NativeError::Invalid("pitch classes must be 0..12".into()));
    }
    if !(0..=24).contains(&req.max_fret) {
        return Err(NativeError::Invalid("max_fret must be 0..=24".into()));
    }
    if !(0..=12).contains(&req.max_span) {
        return Err(NativeError::Invalid("max_span must be 0..=12".into()));
    }
    if req.min_strings < 1 || req.min_strings as usize > req.tuning.len() {
        return Err(NativeError::Invalid(
            "min_strings must be within the tuning".into(),
        ));
    }

    let cap = req.max_results.clamp(1, MAX_RESULTS as i32) as usize;
    let mut buffer = vec![NativeVoicing::default(); cap];
    let mut count: i32 = 0;
    // SAFETY: all pointers refer to live, correctly sized buffers; C++ writes
    // at most `cap` elements and never retains the pointers.
    let status = unsafe {
        ffi::temper_voicing_search(
            req.tuning.as_ptr(),
            req.tuning.len() as i32,
            req.chord_pcs.as_ptr(),
            req.chord_pcs.len() as i32,
            req.max_fret,
            req.max_span,
            req.min_strings,
            cap as i32,
            buffer.as_mut_ptr(),
            cap as i32,
            &mut count,
        )
    };
    if status != 0 {
        return Err(NativeError::Status {
            code: status,
            detail: ffi::status_text(status).to_string(),
        });
    }
    let count = count.clamp(0, cap as i32) as usize;

    let voicings = buffer[..count]
        .iter()
        .map(|v| {
            let frets = v.frets[..req.tuning.len()].to_vec();
            let mut midis = Vec::with_capacity(req.tuning.len());
            let mut keys = Vec::with_capacity(req.tuning.len());
            for (i, fret) in frets.iter().enumerate() {
                if *fret >= 0 {
                    midis.push(req.tuning[i] + *fret);
                    let string_no = req.tuning.len() - i;
                    keys.push(format!("{string_no}:{fret}"));
                }
            }
            Voicing {
                frets,
                string_count: v.string_count,
                lo_string: v.lo_string,
                hi_string: v.hi_string,
                span: v.span,
                max_fret: v.max_fret,
                bass_pc: v.bass_pc,
                note_count: v.note_count,
                score: v.score,
                midis,
                keys,
            }
        })
        .collect();
    Ok(voicings)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcsAnalysis {
    pub cardinality: i32,
    pub interval_vector: Vec<i32>,
    pub transpositional_symmetry: bool,
    pub tension: i32,
    pub span_semitones: i32,
}

pub fn analyze_pcs(pcs: &[i32]) -> Result<PcsAnalysis, NativeError> {
    ensure_abi()?;
    if pcs.is_empty() || pcs.len() > 12 {
        return Err(NativeError::Invalid("need 1..=12 pitch classes".into()));
    }
    if pcs.iter().any(|p| !(0..12).contains(p)) {
        return Err(NativeError::Invalid("pitch classes must be 0..12".into()));
    }
    let mut out = NativeAnalysis::default();
    // SAFETY: `pcs` is a live slice; `out` is a local, correctly aligned struct.
    let status = unsafe { ffi::temper_analyze_pcs(pcs.as_ptr(), pcs.len() as i32, &mut out) };
    if status != 0 {
        return Err(NativeError::Status {
            code: status,
            detail: ffi::status_text(status).to_string(),
        });
    }
    Ok(PcsAnalysis {
        cardinality: out.cardinality,
        interval_vector: out.interval_vector.to_vec(),
        transpositional_symmetry: out.transpositional_symmetry == 1,
        tension: out.tension,
        span_semitones: out.span_semitones,
    })
}

/// Runs the C++ self-test through the linked library.
pub fn selftest() -> Result<(), NativeError> {
    ensure_abi()?;
    // SAFETY: no arguments, no shared state.
    let status = unsafe { ffi::temper_native_selftest() };
    if status == 0 {
        Ok(())
    } else {
        Err(NativeError::Status {
            code: status,
            detail: format!("native selftest failed at check {status}"),
        })
    }
}

pub fn abi_version() -> i32 {
    ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_and_selftest() {
        assert_eq!(abi_version(), 1);
        selftest().expect("C++ core self-test");
    }

    #[test]
    fn cmaj7_voicings_are_playable() {
        let voicings = search_voicings(&VoicingRequest {
            chord_pcs: vec![0, 4, 7, 11],
            ..Default::default()
        })
        .expect("search");
        assert!(!voicings.is_empty());
        assert!(voicings.len() <= 12);
        for v in &voicings {
            assert!(v.note_count >= 4);
            assert!(v.span <= 4);
            assert!(v.max_fret <= 15);
            assert_eq!(v.keys.len(), v.midis.len());
            assert!(v.frets.iter().any(|f| *f >= 0));
        }
        // Sorted by score, best first.
        assert!(voicings.windows(2).all(|w| w[0].score >= w[1].score));
        assert!(voicings.iter().any(|v| v.bass_pc == 0));
    }

    #[test]
    fn honors_result_cap() {
        let voicings = search_voicings(&VoicingRequest {
            chord_pcs: vec![0, 4, 7],
            max_results: 3,
            ..Default::default()
        })
        .expect("search");
        assert_eq!(voicings.len(), 3);
    }

    #[test]
    fn rejects_bad_input() {
        let err = search_voicings(&VoicingRequest {
            chord_pcs: vec![12],
            ..Default::default()
        });
        assert!(matches!(err, Err(NativeError::Invalid(_))));
        assert!(analyze_pcs(&[]).is_err());
    }

    #[test]
    fn analysis_matches_known_sets() {
        let major = analyze_pcs(&[0, 4, 7]).unwrap();
        assert_eq!(major.cardinality, 3);
        assert_eq!(major.interval_vector, vec![0, 0, 1, 1, 1, 0]);
        assert!(!major.transpositional_symmetry);

        let dim7 = analyze_pcs(&[0, 3, 6, 9]).unwrap();
        assert!(dim7.transpositional_symmetry);
        assert_eq!(dim7.interval_vector, vec![0, 0, 4, 0, 0, 2]);
    }

    #[test]
    fn concurrent_searches_are_stable() {
        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(std::thread::spawn(|| {
                let mut counts = Vec::new();
                for _ in 0..40 {
                    let found = search_voicings(&VoicingRequest {
                        chord_pcs: vec![0, 4, 7, 11],
                        ..Default::default()
                    })
                    .expect("search");
                    counts.push(found.len());
                }
                counts
            }));
        }
        let all: Vec<Vec<usize>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let first = &all[0];
        assert!(all.iter().all(|c| c == first));
    }
}

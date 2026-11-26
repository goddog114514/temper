//! TEMPER core: music theory in Rust, guitar voicing search in C++.
//!
//! The two halves meet at a narrow C ABI (`native/temper_native.h`). Rust is
//! responsible for validation, memory and serde; C++ owns the combinatorial
//! search and pitch-class arithmetic that benefit from a tight loop.

pub mod ffi;
pub mod native;
pub mod theory;

use serde::Serialize;

pub use native::{analyze_pcs, search_voicings, selftest, PcsAnalysis, Voicing, VoicingRequest};
pub use theory::{
    chord_tones, identify_chords, key_signature, note_name, scale_tones, CHORDS, INTERVALS, METERS,
    MODES, PRESETS, SCALES, STANDARD_TUNING,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyRef {
    pub pc: u8,
    pub name: String,
    pub accidentals: i8,
    pub relative_minor: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub version: &'static str,
    pub tuning: [i32; 6],
    pub chords: &'static [theory::ChordQuality],
    pub scales: &'static [theory::Scale],
    pub modes: &'static [theory::Mode],
    pub intervals: &'static [theory::Interval],
    pub presets: &'static [theory::Preset],
    pub meters: &'static [theory::Meter],
    pub keys: Vec<KeyRef>,
}

pub fn catalog() -> Catalog {
    let keys = (0..12u8)
        .map(|pc| {
            let sig = theory::key_signature(pc);
            KeyRef {
                pc,
                name: sig.key_name,
                accidentals: sig.accidentals,
                relative_minor: sig.relative_minor,
            }
        })
        .collect();
    Catalog {
        version: VERSION,
        tuning: STANDARD_TUNING,
        chords: CHORDS,
        scales: SCALES,
        modes: MODES,
        intervals: INTERVALS,
        presets: PRESETS,
        meters: METERS,
        keys,
    }
}

#[cfg(test)]
mod smoke {
    #[test]
    fn catalog_is_complete() {
        let c = super::catalog();
        assert_eq!(c.keys.len(), 12);
        assert_eq!(c.chords.len(), 11);
        assert_eq!(c.scales.len(), 5);
        assert_eq!(c.modes.len(), 7);
        assert_eq!(c.presets.len(), 7);
    }
}

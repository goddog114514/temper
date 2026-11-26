//! The only module that knows the C ABI. Keep `unsafe` here and nowhere else.

use std::ffi::c_int;

pub const ABI_VERSION: i32 = 1;
pub const MAX_STRINGS: usize = 8;
pub const MAX_RESULTS: usize = 512;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NativeVoicing {
    pub frets: [i32; MAX_STRINGS],
    pub string_count: i32,
    pub lo_string: i32,
    pub hi_string: i32,
    pub span: i32,
    pub max_fret: i32,
    pub bass_pc: i32,
    pub note_count: i32,
    pub score: i32,
}

impl Default for NativeVoicing {
    fn default() -> Self {
        Self {
            frets: [-1; MAX_STRINGS],
            string_count: 0,
            lo_string: 0,
            hi_string: 0,
            span: 0,
            max_fret: 0,
            bass_pc: 0,
            note_count: 0,
            score: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeAnalysis {
    pub cardinality: i32,
    pub interval_vector: [i32; 6],
    pub transpositional_symmetry: i32,
    pub tension: i32,
    pub span_semitones: i32,
}

extern "C" {
    pub fn temper_abi_version() -> c_int;
    pub fn temper_native_selftest() -> c_int;
    pub fn temper_voicing_search(
        tuning: *const i32,
        string_count: c_int,
        chord_pcs: *const i32,
        pc_count: c_int,
        max_fret: c_int,
        max_span: c_int,
        min_strings: c_int,
        max_results: c_int,
        out: *mut NativeVoicing,
        out_cap: c_int,
        out_count: *mut c_int,
    ) -> c_int;
    pub fn temper_analyze_pcs(
        pcs: *const i32,
        count: c_int,
        out: *mut NativeAnalysis,
    ) -> c_int;
}

/// Maps the integer status code used across the boundary to a stable string.
pub fn status_text(code: i32) -> &'static str {
    match code {
        0 => "ok",
        1 => "invalid argument",
        2 => "argument out of range",
        3 => "output buffer too small",
        4 => "native internal error",
        _ => "unknown native status",
    }
}

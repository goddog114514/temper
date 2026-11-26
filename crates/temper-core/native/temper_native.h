/* TEMPER native core — C ABI shared with Rust.
 *
 * Design contract
 * ---------------
 * - Plain C types only. No Rust types, no C++ types, no ownership transfer.
 * - Every buffer is allocated by the caller (Rust). The callee writes at most
 *   `out_cap` elements and reports the real count through `out_count`.
 * - All functions are reentrant and thread-safe: they hold no mutable global
 *   state, do no dynamic allocation, and keep per-thread scratch space only.
 * - Errors are integer codes; nothing crosses the boundary as an exception.
 */
#ifndef TEMPER_NATIVE_H
#define TEMPER_NATIVE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define TEMPER_ABI_VERSION 1
#define TEMPER_MAX_STRINGS 8
#define TEMPER_MAX_RESULTS 512

enum TemperStatus {
  TEMPER_OK = 0,
  TEMPER_ERR_ARG = 1,
  TEMPER_ERR_RANGE = 2,
  TEMPER_ERR_NO_SPACE = 3,
  TEMPER_ERR_INTERNAL = 4
};

typedef struct TemperVoicing {
  int32_t frets[TEMPER_MAX_STRINGS]; /* -1 = muted, index 0 = lowest string */
  int32_t string_count;              /* tuned strings in this tuning      */
  int32_t lo_string;                 /* first sounding string index       */
  int32_t hi_string;                 /* last sounding string index        */
  int32_t span;                      /* max fret - min pressed fret       */
  int32_t max_fret;
  int32_t bass_pc;                   /* pitch class of lowest sounding    */
  int32_t note_count;                /* sounding strings                  */
  int32_t score;                     /* higher is better                  */
} TemperVoicing;

typedef struct TemperAnalysis {
  int32_t cardinality;      /* distinct pitch classes              */
  int32_t interval_vector[6]; /* ic1..ic6 counts                   */
  int32_t transpositional_symmetry; /* 1 when some t maps set to itself */
  int32_t tension;          /* weighted dissonance, 0..100         */
  int32_t span_semitones;   /* highest - lowest pitch class        */
} TemperAnalysis;

int32_t temper_abi_version(void);

/* Returns 0 when the compiled core agrees with its own invariants. */
int32_t temper_native_selftest(void);

/* Enumerates playable guitar voicings for a pitch-class set.
 * tuning: MIDI note per string, low to high.
 * Returns TEMPER_OK and sets *out_count (<= out_cap). */
int32_t temper_voicing_search(const int32_t *tuning, int32_t string_count,
                              const int32_t *chord_pcs, int32_t pc_count,
                              int32_t max_fret, int32_t max_span,
                              int32_t min_strings, int32_t max_results,
                              TemperVoicing *out, int32_t out_cap,
                              int32_t *out_count);

/* Interval vector / symmetry / tension for a pitch-class set. */
int32_t temper_analyze_pcs(const int32_t *pcs, int32_t count,
                           TemperAnalysis *out);

#ifdef __cplusplus
}
#endif

#endif /* TEMPER_NATIVE_H */

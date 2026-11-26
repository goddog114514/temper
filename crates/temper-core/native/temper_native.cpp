/* TEMPER native core — implementation.
 *
 * Everything here is allocation-free and reentrant. Scratch space lives on the
 * stack, so a search costs one call frame and no heap traffic; the Rust side
 * decides how many results it wants and owns the output buffer.
 */
#include "temper_native.h"

#include <algorithm>
#include <array>
#include <cstring>

namespace {

constexpr int kMaxStrings = TEMPER_MAX_STRINGS;
constexpr int kMaxFret = 24;
constexpr int kMaxResults = TEMPER_MAX_RESULTS;
constexpr int kEnumBudget = 4'000'000; /* hard latency guard per call */

struct SearchState {
  const int32_t *tuning;
  int32_t string_count;
  bool in_set[12];
  int32_t pc_count;
  int32_t root_pc;
  int32_t max_fret;
  int32_t max_span;
  int32_t min_strings;
  int32_t limit;
  TemperVoicing *out;
  int32_t count;
  int32_t budget;
  bool truncated;
};

inline int32_t pc_of(int32_t midi) {
  int32_t pc = midi % 12;
  return pc < 0 ? pc + 12 : pc;
}

inline int32_t clamp_int(int32_t v, int32_t lo, int32_t hi) {
  return v < lo ? lo : (v > hi ? hi : v);
}

int compare_voicing(const TemperVoicing &a, const TemperVoicing &b) {
  if (a.score != b.score) return a.score > b.score ? -1 : 1;
  if (a.span != b.span) return a.span < b.span ? -1 : 1;
  if (a.max_fret != b.max_fret) return a.max_fret < b.max_fret ? -1 : 1;
  for (int i = 0; i < kMaxStrings; ++i) {
    if (a.frets[i] != b.frets[i]) return a.frets[i] < b.frets[i] ? -1 : 1;
  }
  return 0;
}

bool same_shape(const TemperVoicing &a, const TemperVoicing &b) {
  if (a.lo_string != b.lo_string || a.hi_string != b.hi_string) return false;
  for (int i = 0; i < kMaxStrings; ++i) {
    if (a.frets[i] != b.frets[i]) return false;
  }
  return true;
}

void consider(SearchState &s, const int32_t *frets, int32_t lo, int32_t hi) {
  TemperVoicing v{};
  v.string_count = s.string_count;
  v.lo_string = lo;
  v.hi_string = hi;
  int32_t note_count = 0;
  int32_t open = 0;
  int32_t min_pressed = 99;
  int32_t max_pressed = -1;
  bool covered[12] = {false};
  int32_t lowest_midi = 1 << 30;
  int32_t bass_pc = 0;

  for (int i = 0; i < kMaxStrings; ++i) v.frets[i] = -1;
  for (int i = lo; i <= hi; ++i) {
    const int32_t f = frets[i];
    v.frets[i] = f;
    const int32_t midi = s.tuning[i] + f;
    covered[pc_of(midi)] = true;
    ++note_count;
    if (f == 0) ++open;
    if (f > 0) {
      min_pressed = std::min(min_pressed, f);
      max_pressed = std::max(max_pressed, f);
    }
    if (midi < lowest_midi) {
      lowest_midi = midi;
      bass_pc = pc_of(midi);
    }
  }

  if (note_count < s.min_strings) return;
  for (int32_t p = 0; p < 12; ++p) {
    if (s.in_set[p] && !covered[p]) return; /* must contain the whole chord */
  }

  const int32_t span = max_pressed >= 0 ? max_pressed - min_pressed : 0;
  if (span > s.max_span) return;
  /* Open strings plus a high position is not a shape a hand can hold. */
  if (open > 0 && max_pressed > 5) return;

  v.span = span;
  v.max_fret = max_pressed;
  v.bass_pc = bass_pc;
  v.note_count = note_count;
  v.score = 40 + note_count * 14 + (bass_pc == s.root_pc ? 32 : 0) - span * 5 -
            std::max(max_pressed, 0) * 2 + open * 3;

  if (s.count < s.limit) {
    s.out[s.count++] = v;
    return;
  }
  /* Keep the best `limit` shapes with a single replacement pass. */
  int worst = 0;
  for (int i = 1; i < s.count; ++i) {
    if (compare_voicing(s.out[i], s.out[worst]) > 0) worst = i;
  }
  if (compare_voicing(v, s.out[worst]) < 0) s.out[worst] = v;
}

void walk(SearchState &s, int32_t string, int32_t lo, int32_t hi,
          const std::array<std::array<int32_t, kMaxFret + 1>, kMaxStrings>
              &cands,
          const std::array<int32_t, kMaxStrings> &cand_count,
          int32_t *frets, int32_t window_lo, int32_t window_hi,
          int32_t min_pressed, int32_t max_pressed) {
  if (--s.budget <= 0) {
    s.truncated = true;
    return;
  }
  if (string > hi) {
    consider(s, frets, lo, hi);
    return;
  }
  for (int32_t k = 0; k < cand_count[string]; ++k) {
    const int32_t f = cands[string][k];
    if (f != 0 && (f < window_lo || f > window_hi)) continue;
    const int32_t next_min = f > 0 ? std::min(min_pressed, f) : min_pressed;
    const int32_t next_max = f > 0 ? std::max(max_pressed, f) : max_pressed;
    if (next_max >= 0 && next_min <= 99 && next_max - next_min > s.max_span) {
      continue;
    }
    frets[string] = f;
    walk(s, string + 1, lo, hi, cands, cand_count, frets, window_lo, window_hi,
         next_min, next_max);
    if (s.truncated) return;
  }
}

} // namespace

extern "C" {

int32_t temper_abi_version(void) { return TEMPER_ABI_VERSION; }

int32_t temper_voicing_search(const int32_t *tuning, int32_t string_count,
                              const int32_t *chord_pcs, int32_t pc_count,
                              int32_t max_fret, int32_t max_span,
                              int32_t min_strings, int32_t max_results,
                              TemperVoicing *out, int32_t out_cap,
                              int32_t *out_count) {
  if (!tuning || !chord_pcs || !out || !out_count) return TEMPER_ERR_ARG;
  if (string_count < 2 || string_count > kMaxStrings) return TEMPER_ERR_RANGE;
  if (pc_count < 1 || pc_count > 12) return TEMPER_ERR_RANGE;
  if (out_cap < 1) return TEMPER_ERR_RANGE;
  if (max_fret < 0 || max_fret > kMaxFret) return TEMPER_ERR_RANGE;
  if (max_span < 0 || max_span > 12) return TEMPER_ERR_RANGE;
  if (min_strings < 1 || min_strings > string_count) return TEMPER_ERR_RANGE;

  const int32_t limit = clamp_int(max_results, 1, std::min(out_cap, kMaxResults));
  SearchState s{};
  s.tuning = tuning;
  s.string_count = string_count;
  s.pc_count = 0;
  s.max_fret = max_fret;
  s.max_span = max_span;
  s.min_strings = min_strings;
  s.limit = limit;
  s.out = out;
  s.count = 0;
  s.budget = kEnumBudget;
  s.truncated = false;

  for (int32_t i = 0; i < pc_count; ++i) {
    const int32_t pc = chord_pcs[i] % 12;
    if (pc < 0) return TEMPER_ERR_ARG;
    if (!s.in_set[pc]) {
      s.in_set[pc] = true;
      ++s.pc_count;
    }
  }
  s.root_pc = chord_pcs[0] % 12;

  /* Candidate frets per string, ascending, limited to chord tones. */
  std::array<std::array<int32_t, kMaxFret + 1>, kMaxStrings> cands{};
  std::array<int32_t, kMaxStrings> cand_count{};
  for (int i = 0; i < string_count; ++i) {
    int32_t n = 0;
    for (int32_t f = 0; f <= max_fret; ++f) {
      if (s.in_set[pc_of(tuning[i] + f)]) cands[i][n++] = f;
    }
    cand_count[i] = n;
  }

  int32_t frets[kMaxStrings];
  for (int i = 0; i < kMaxStrings; ++i) frets[i] = -1;

  for (int32_t lo = 0; lo + min_strings <= string_count; ++lo) {
    for (int32_t hi = lo + min_strings - 1; hi < string_count; ++hi) {
      bool complete = true;
      for (int32_t i = lo; i <= hi; ++i) {
        if (cand_count[i] == 0) {
          complete = false;
          break;
        }
      }
      if (!complete) continue;

      for (int32_t k = 0; k < cand_count[lo]; ++k) {
        const int32_t anchor = cands[lo][k];
        const int32_t wlo = anchor == 0 ? 0 : anchor;
        const int32_t whi = anchor == 0 ? std::min(max_fret, max_span + 1)
                                        : std::min(max_fret, anchor + max_span);
        frets[lo] = anchor;
        walk(s, lo + 1, lo, hi, cands, cand_count, frets, wlo, whi,
             anchor > 0 ? anchor : 99, anchor > 0 ? anchor : -1);
        if (s.truncated) break;
      }
      if (s.truncated) break;
    }
    if (s.truncated) break;
  }

  /* Stable insertion sort: small N (<= 512), no allocation. */
  for (int32_t i = 1; i < s.count; ++i) {
    TemperVoicing key = s.out[i];
    int32_t j = i - 1;
    while (j >= 0 && compare_voicing(key, s.out[j]) < 0) {
      s.out[j + 1] = s.out[j];
      --j;
    }
    s.out[j + 1] = key;
  }

  /* Drop duplicate shapes, keeping the first (best) occurrence. */
  int32_t unique = 0;
  for (int32_t i = 0; i < s.count; ++i) {
    bool dup = false;
    for (int32_t j = 0; j < unique; ++j) {
      if (same_shape(s.out[i], s.out[j])) {
        dup = true;
        break;
      }
    }
    if (!dup) s.out[unique++] = s.out[i];
  }

  *out_count = unique;
  return TEMPER_OK;
}

int32_t temper_analyze_pcs(const int32_t *pcs, int32_t count,
                           TemperAnalysis *out) {
  if (!pcs || !out) return TEMPER_ERR_ARG;
  if (count < 1 || count > 12) return TEMPER_ERR_RANGE;

  bool set[12] = {false};
  int32_t cardinality = 0;
  for (int32_t i = 0; i < count; ++i) {
    const int32_t pc = ((pcs[i] % 12) + 12) % 12;
    if (!set[pc]) {
      set[pc] = true;
      ++cardinality;
    }
  }

  TemperAnalysis a{};
  a.cardinality = cardinality;
  for (int32_t i = 0; i < 12; ++i) {
    if (!set[i]) continue;
    for (int32_t j = i + 1; j < 12; ++j) {
      if (!set[j]) continue;
      int32_t d = j - i;
      if (d > 6) d = 12 - d;
      if (d >= 1 && d <= 6) ++a.interval_vector[d - 1];
    }
  }

  for (int32_t t = 1; t < 12; ++t) {
    bool same = true;
    for (int32_t i = 0; i < 12 && same; ++i) {
      if (set[i] != set[(i + t) % 12]) same = false;
    }
    if (same) {
      a.transpositional_symmetry = 1;
      break;
    }
  }

  /* Smallest arc covering the set: 12 minus the largest circular gap. */
  int32_t largest_gap = 0;
  if (cardinality > 1) {
    int32_t prev = -1;
    int32_t first = -1;
    for (int32_t i = 0; i < 12; ++i) {
      if (!set[i]) continue;
      if (first < 0) first = i;
      if (prev >= 0) largest_gap = std::max(largest_gap, i - prev);
      prev = i;
    }
    largest_gap = std::max(largest_gap, first + 12 - prev);
    a.span_semitones = 12 - largest_gap;
  } else {
    a.span_semitones = 0;
  }

  static const int32_t weight[6] = {5, 2, 1, 1, 1, 3};
  int32_t weighted = 0;
  int32_t pairs = 0;
  for (int32_t i = 0; i < 6; ++i) {
    weighted += a.interval_vector[i] * weight[i];
    pairs += a.interval_vector[i];
  }
  a.tension = pairs > 0 ? clamp_int(weighted * 100 / (pairs * 5), 0, 100) : 0;

  *out = a;
  return TEMPER_OK;
}

int32_t temper_native_selftest(void) {
  if (temper_abi_version() != TEMPER_ABI_VERSION) return 1;

  const int32_t triad[3] = {0, 4, 7};
  TemperAnalysis a{};
  if (temper_analyze_pcs(triad, 3, &a) != TEMPER_OK) return 2;
  if (a.cardinality != 3) return 3;
  if (a.interval_vector[3] != 1) return 4; /* perfect fourth/fifth */
  if (a.interval_vector[4] != 1) return 5; /* major third/sixth */
  if (a.interval_vector[2] != 1) return 6; /* minor third/sixth */

  const int32_t tuning[6] = {40, 45, 50, 55, 59, 64};
  TemperVoicing out[16];
  int32_t count = 0;
  const int32_t status = temper_voicing_search(tuning, 6, triad, 3, 12, 4, 4,
                                               16, out, 16, &count);
  if (status != TEMPER_OK) return 7;
  if (count < 1) return 8;
  bool root_bass = false;
  for (int32_t i = 0; i < count; ++i) {
    if (out[i].bass_pc == 0) root_bass = true;
    if (out[i].span > 4) return 10;
    if (out[i].note_count < 4) return 11;
  }
  if (!root_bass) return 9;

  /* Extended range must not explode nor lose the bass contract. */
  const int32_t seventh[4] = {0, 4, 7, 11};
  count = 0;
  if (temper_voicing_search(tuning, 6, seventh, 4, 15, 4, 4, 32, out, 16,
                            &count) != TEMPER_OK)
    return 12;
  if (count < 1 || count > 16) return 13;

  /* Invalid input must be reported, never crash. */
  if (temper_voicing_search(tuning, 0, triad, 3, 12, 4, 4, 8, out, 16,
                            &count) != TEMPER_ERR_RANGE)
    return 14;
  if (temper_analyze_pcs(nullptr, 3, &a) != TEMPER_ERR_ARG) return 15;

  return 0;
}

} // extern "C"

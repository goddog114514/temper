/* Pure theory helpers shared by the practice pages.
   No DOM access — covered by node --test tests/frontend/theory.test.mjs. */

export const PC_NAMES = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"];
export const ROOT_NAMES = ["C", "D♭", "D", "E♭", "E", "F", "G♭", "G", "A♭", "A", "B♭", "B"];
export const LETTERS = ["C", "D", "E", "F", "G", "A", "B"];
export const NATURAL_PC = [0, 2, 4, 5, 7, 9, 11];
export const ROOT_LETTER = [0, 1, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6];

export const CHORDS = [
  { id: "maj", label: "大三和弦", symbol: "", intervals: [0, 4, 7], degrees: ["1", "3", "5"] },
  { id: "min", label: "小三和弦", symbol: "m", intervals: [0, 3, 7], degrees: ["1", "♭3", "5"] },
  { id: "dim", label: "减三和弦", symbol: "dim", intervals: [0, 3, 6], degrees: ["1", "♭3", "♭5"] },
  { id: "aug", label: "增三和弦", symbol: "aug", intervals: [0, 4, 8], degrees: ["1", "3", "♯5"] },
  { id: "sus2", label: "挂二和弦", symbol: "sus2", intervals: [0, 2, 7], degrees: ["1", "2", "5"] },
  { id: "sus4", label: "挂四和弦", symbol: "sus4", intervals: [0, 5, 7], degrees: ["1", "4", "5"] },
  { id: "maj7", label: "大七和弦", symbol: "maj7", intervals: [0, 4, 7, 11], degrees: ["1", "3", "5", "7"] },
  { id: "m7", label: "小七和弦", symbol: "m7", intervals: [0, 3, 7, 10], degrees: ["1", "♭3", "5", "♭7"] },
  { id: "dom7", label: "属七和弦", symbol: "7", intervals: [0, 4, 7, 10], degrees: ["1", "3", "5", "♭7"] },
  { id: "m7b5", label: "半减七和弦", symbol: "m7♭5", intervals: [0, 3, 6, 10], degrees: ["1", "♭3", "♭5", "♭7"] },
  { id: "dim7", label: "减七和弦", symbol: "dim7", intervals: [0, 3, 6, 9], degrees: ["1", "♭3", "♭5", "♭♭7"] },
];

export const SCALES = [
  { id: "major", name: "自然大调", steps: [2, 2, 1, 2, 2, 2, 1], degrees: ["1", "2", "3", "4", "5", "6", "7"] },
  { id: "minor", name: "自然小调", steps: [2, 1, 2, 2, 1, 2, 2], degrees: ["1", "2", "♭3", "4", "5", "♭6", "♭7"] },
  { id: "harmonic", name: "和声小调", steps: [2, 1, 2, 2, 1, 3, 1], degrees: ["1", "2", "♭3", "4", "5", "♭6", "7"] },
  { id: "penta", name: "大调五声", steps: [2, 2, 3, 2, 3], degrees: ["1", "2", "3", "5", "6"] },
  { id: "blues", name: "布鲁斯", steps: [3, 2, 1, 1, 3, 2], degrees: ["1", "♭3", "4", "♯4", "5", "♭7"] },
];

export const PRESETS = [
  { id: "canon", label: "卡农进行", degrees: [0, 4, 5, 2, 3, 0, 3, 4], roman: "I–V–vi–iii–IV–I–IV–V" },
  { id: "p4536251", label: "4536251", degrees: [3, 4, 2, 5, 1, 4, 0], roman: "IV–V–iii–vi–ii–V–I" },
  { id: "p1645", label: "1645", degrees: [0, 5, 3, 4], roman: "I–vi–IV–V" },
  { id: "p1564", label: "1564", degrees: [0, 4, 5, 3], roman: "I–V–vi–IV" },
  { id: "p4536", label: "4536", degrees: [3, 4, 2, 5], roman: "IV–V–iii–vi" },
  { id: "cad", label: "ii–V–I", degrees: [1, 4, 0, 0], roman: "ii–V–I–I" },
  { id: "desc", label: "I–iii–IV–V", degrees: [0, 2, 3, 4], roman: "I–iii–IV–V" },
];

export const ROMAN = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];
const TRIAD_QUALITY = ["maj", "min", "min", "maj", "maj", "min", "dim"];
const SEVENTH_QUALITY = ["maj7", "m7", "m7", "maj7", "dom7", "m7", "m7b5"];

export const pcOf = (midi) => ((midi % 12) + 12) % 12;
export const midiToFreq = (midi) => 440 * Math.pow(2, (midi - 69) / 12);
export const noteName = (midi) => `${PC_NAMES[pcOf(midi)]}${Math.floor(midi / 12) - 1}`;
export const rootName = (pc) => ROOT_NAMES[((pc % 12) + 12) % 12];

export function spell(pc, letter) {
  const nat = NATURAL_PC[((letter % 7) + 7) % 7];
  let diff = ((pc - nat) % 12 + 12) % 12;
  if (diff > 6) diff -= 12;
  const base = LETTERS[((letter % 7) + 7) % 7];
  return base + (diff > 0 ? "♯".repeat(diff) : diff < 0 ? "♭".repeat(-diff) : "");
}

export const chordQuality = (id) => CHORDS.find((q) => q.id === id);

export function chordTones(rootPc, qualityId, inversion = 0) {
  const q = chordQuality(qualityId);
  if (!q) return [];
  const rootLetter = ROOT_LETTER[((rootPc % 12) + 12) % 12];
  const tones = q.intervals.map((semi, i) => {
    const pc = (rootPc + semi) % 12;
    const letter = (rootLetter + i * 2) % 7;
    return {
      midi: 60 + rootPc + semi,
      pc,
      name: spell(pc, letter),
      letter,
      degree: q.degrees[i] || "",
    };
  });
  const inv = Math.min(inversion, tones.length - 1);
  for (let i = 0; i < inv; i++) tones[i].midi += 12;
  tones.sort((a, b) => a.midi - b.midi);
  return tones;
}

export function chordSymbol(rootPc, qualityId, inversion = 0) {
  const q = chordQuality(qualityId);
  if (!q) return "";
  const tones = chordTones(rootPc, qualityId, inversion);
  const root = tones.find((t) => t.degree === "1")?.name || rootName(rootPc);
  const bass = tones[0]?.name || root;
  return root + q.symbol + (inversion > 0 && bass !== root ? `/${bass}` : "");
}

export function scaleTones(tonicPc, scaleId) {
  const s = SCALES.find((x) => x.id === scaleId);
  if (!s) return [];
  const semis = [0];
  s.steps.forEach((step) => semis.push(semis[semis.length - 1] + step));
  const rootLetter = ROOT_LETTER[((tonicPc % 12) + 12) % 12];
  return semis.map((semi, i) => {
    const midi = 60 + tonicPc + semi;
    const pc = pcOf(midi);
    const letter = (rootLetter + i) % 7;
    return { midi, pc, name: spell(pc, letter), degree: s.degrees[i] || "8" };
  });
}

/** Degree (0–6) of a major key, rendered as a diatonic chord. */
export function progressionChord(keyPc, degree, { seventh = false, inversion = 0 } = {}) {
  const table = seventh ? SEVENTH_QUALITY : TRIAD_QUALITY;
  const qualityId = table[degree] || "maj";
  const rootPc = (keyPc + [0, 2, 4, 5, 7, 9, 11][degree]) % 12;
  const tones = chordTones(rootPc, qualityId, inversion);
  const q = chordQuality(qualityId);
  const root = tones.find((t) => t.degree === "1")?.name || rootName(rootPc);
  const bass = tones[0]?.name || root;
  return {
    degree,
    quality: qualityId,
    rootPc,
    roman: ROMAN[degree],
    symbol: root + q.symbol + (inversion > 0 && bass !== root ? `/${bass}` : ""),
    midis: tones.map((t) => t.midi),
    tones,
  };
}

export function progression(keyPc, degrees, options = {}) {
  return degrees.map((d) => progressionChord(keyPc, d, options));
}

export function notesForSight(midi) {
  const pc = pcOf(midi);
  const letter = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6][pc];
  const name = spell(pc, letter);
  const octave = Math.floor(midi / 12) - 1 + (pc === 0 && name.startsWith("B") ? 1 : 0);
  const diatonic = octave * 7 + letter;
  return { midi, name, letter, octave, diatonic, pc };
}

export function formatClock(ms) {
  const total = Math.max(0, Math.round(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function shareCode(pathname) {
  const match = /\/s\/([a-z0-9]+)/i.exec(pathname || "");
  return match ? match[1].toLowerCase() : "";
}

export function pct(part, whole) {
  if (!whole) return 0;
  return Math.round((part / whole) * 1000) / 10;
}

export function clamp(value, lo, hi) {
  return Math.min(hi, Math.max(lo, value));
}

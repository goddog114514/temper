import test from "node:test";
import assert from "node:assert/strict";

import {
  CHORDS,
  PRESETS,
  chordSymbol,
  chordTones,
  formatClock,
  midiToFreq,
  noteName,
  notesForSight,
  pct,
  progression,
  rootName,
  scaleTones,
  shareCode,
  spell,
} from "../../web/js/theory.js";

test("pitch class names and frequencies", () => {
  assert.equal(noteName(60), "C4");
  assert.equal(noteName(69), "A4");
  assert.equal(rootName(10), "B♭");
  assert.ok(Math.abs(midiToFreq(69) - 440) < 1e-9);
  assert.ok(Math.abs(midiToFreq(57) - 220) < 1e-9);
});

test("spelling follows the requested letter", () => {
  assert.equal(spell(6, 3), "F♯");
  assert.equal(spell(6, 4), "G♭");
  assert.equal(spell(10, 6), "B♭");
  assert.equal(spell(1, 1), "D♭");
});

test("chord tones and inversions", () => {
  const tones = chordTones(0, "maj7");
  assert.deepEqual(tones.map((t) => t.name), ["C", "E", "G", "B"]);
  const inv = chordTones(0, "maj7", 1);
  assert.equal(inv[0].name, "E");
  assert.equal(inv[0].midi, 64);
  assert.equal(inv[3].midi, 72);
  assert.equal(chordSymbol(5, "maj7", 1), "Fmaj7/A");
  assert.equal(CHORDS.length, 11);
});

test("flat keys spell correctly", () => {
  assert.deepEqual(chordTones(10, "maj").map((t) => t.name), ["B♭", "D", "F"]);
  assert.equal(chordSymbol(3, "min"), "E♭m");
});

test("scales produce eight tones with degrees", () => {
  const major = scaleTones(0, "major");
  assert.equal(major.length, 8);
  assert.deepEqual(major.map((t) => t.name), ["C", "D", "E", "F", "G", "A", "B", "C"]);
  assert.equal(major[0].degree, "1");
});

test("diatonic progressions match the handbook presets", () => {
  assert.equal(PRESETS.length, 7);
  assert.equal(PRESETS[0].id, "canon");
  const chords = progression(0, [0, 5, 3, 4]);
  assert.deepEqual(chords.map((c) => c.symbol), ["C", "Am", "F", "G"]);
  const d = progression(2, [0, 4, 5, 3]);
  assert.deepEqual(d.map((c) => c.symbol), ["D", "A", "Bm", "G"]);
  const seventh = progression(0, [1, 4, 0], { seventh: true });
  assert.deepEqual(seventh.map((c) => c.symbol), ["Dm7", "G7", "Cmaj7"]);
});

test("sight-reading helper returns a diatonic position", () => {
  const c4 = notesForSight(60);
  assert.equal(c4.name, "C");
  assert.equal(c4.octave, 4);
  assert.equal(c4.diatonic, 4 * 7);
});

test("utilities", () => {
  assert.equal(shareCode("/s/abc234"), "abc234");
  assert.equal(shareCode("/studio"), "");
  assert.equal(formatClock(65_000), "1:05");
  assert.equal(pct(3, 4), 75);
  assert.equal(pct(1, 0), 0);
});

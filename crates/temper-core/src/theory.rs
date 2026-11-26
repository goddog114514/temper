//! Pure music theory. Everything here is deterministic, allocation-light and
//! unit-tested; the HTTP layer only shapes input/output.

use serde::Serialize;

pub const LETTERS: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];
/// Letter used when a pitch class has to be spelled without context.
pub const LETTER_OF_PC: [usize; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
/// Flat-preferring spelling used for chord roots, matching `ROOT` in the UI:
/// C D♭ D E♭ E F G♭ G A♭ A B♭ B.
pub const ROOT_LETTER: [usize; 12] = [0, 1, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6];
pub const SHARP_NAMES: [&str; 12] = [
    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordQuality {
    pub id: &'static str,
    pub label: &'static str,
    pub symbol: &'static str,
    pub intervals: &'static [u8],
    pub degrees: &'static [&'static str],
}

pub const CHORDS: &[ChordQuality] = &[
    ChordQuality { id: "maj", label: "大三和弦", symbol: "", intervals: &[0, 4, 7], degrees: &["1", "3", "5"] },
    ChordQuality { id: "min", label: "小三和弦", symbol: "m", intervals: &[0, 3, 7], degrees: &["1", "♭3", "5"] },
    ChordQuality { id: "dim", label: "减三和弦", symbol: "dim", intervals: &[0, 3, 6], degrees: &["1", "♭3", "♭5"] },
    ChordQuality { id: "aug", label: "增三和弦", symbol: "aug", intervals: &[0, 4, 8], degrees: &["1", "3", "♯5"] },
    ChordQuality { id: "sus2", label: "挂二和弦", symbol: "sus2", intervals: &[0, 2, 7], degrees: &["1", "2", "5"] },
    ChordQuality { id: "sus4", label: "挂四和弦", symbol: "sus4", intervals: &[0, 5, 7], degrees: &["1", "4", "5"] },
    ChordQuality { id: "maj7", label: "大七和弦", symbol: "maj7", intervals: &[0, 4, 7, 11], degrees: &["1", "3", "5", "7"] },
    ChordQuality { id: "m7", label: "小七和弦", symbol: "m7", intervals: &[0, 3, 7, 10], degrees: &["1", "♭3", "5", "♭7"] },
    ChordQuality { id: "dom7", label: "属七和弦", symbol: "7", intervals: &[0, 4, 7, 10], degrees: &["1", "3", "5", "♭7"] },
    ChordQuality { id: "m7b5", label: "半减七和弦", symbol: "m7♭5", intervals: &[0, 3, 6, 10], degrees: &["1", "♭3", "♭5", "♭7"] },
    ChordQuality { id: "dim7", label: "减七和弦", symbol: "dim7", intervals: &[0, 3, 6, 9], degrees: &["1", "♭3", "♭5", "♭♭7"] },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scale {
    pub id: &'static str,
    pub name: &'static str,
    pub steps: &'static [u8],
    pub letters: &'static [u8],
    pub degrees: &'static [&'static str],
    pub desc: &'static str,
}

pub const SCALES: &[Scale] = &[
    Scale {
        id: "major", name: "自然大调",
        steps: &[2, 2, 1, 2, 2, 2, 1], letters: &[0, 1, 2, 3, 4, 5, 6, 7],
        degrees: &["1", "2", "3", "4", "5", "6", "7"],
        desc: "从主音出发，按“全全半全全全半”排列。第三、四音与第七、八音之间是半音，它们决定了明亮的色彩。",
    },
    Scale {
        id: "minor", name: "自然小调",
        steps: &[2, 1, 2, 2, 1, 2, 2], letters: &[0, 1, 2, 3, 4, 5, 6, 7],
        degrees: &["1", "2", "♭3", "4", "5", "♭6", "♭7"],
        desc: "把大调的第三、六、七级降低半音。主音上方的小三度，是它忧郁颜色的来源。",
    },
    Scale {
        id: "harmonic", name: "和声小调",
        steps: &[2, 1, 2, 2, 1, 3, 1], letters: &[0, 1, 2, 3, 4, 5, 6, 7],
        degrees: &["1", "2", "♭3", "4", "5", "♭6", "7"],
        desc: "自然小调升高第七级，制造出导音与主音之间的半音牵引。增二度出现在第六、七级之间。",
    },
    Scale {
        id: "penta", name: "大调五声",
        steps: &[2, 2, 3, 2, 3], letters: &[0, 1, 2, 4, 5, 6],
        degrees: &["1", "2", "3", "5", "6"],
        desc: "去掉大调里的第四、第七级，就没有了半音与导音。五声音阶听起来开阔、没有强烈的回归压力。",
    },
    Scale {
        id: "blues", name: "布鲁斯",
        steps: &[3, 2, 1, 1, 3, 2], letters: &[0, 2, 3, 3, 4, 6, 7],
        degrees: &["1", "♭3", "4", "♯4", "5", "♭7"],
        desc: "小调五声加上降五级的“蓝音”。三个相邻的半音让旋律可以滑进滑出，是布鲁斯与摇滚的语法。",
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub id: &'static str,
    pub name: &'static str,
    pub cn: &'static str,
    pub degree: u8,
    pub intervals: &'static [u8],
    pub characteristic: &'static str,
    pub mood: &'static str,
}

pub const MODES: &[Mode] = &[
    Mode { id: "ionian", name: "Ionian", cn: "伊奥尼亚", degree: 1, intervals: &[0, 2, 4, 5, 7, 9, 11], characteristic: "大七度", mood: "明亮" },
    Mode { id: "dorian", name: "Dorian", cn: "多利亚", degree: 2, intervals: &[0, 2, 3, 5, 7, 9, 10], characteristic: "大六度 ♮6", mood: "沉着" },
    Mode { id: "phrygian", name: "Phrygian", cn: "弗里吉亚", degree: 3, intervals: &[0, 1, 3, 5, 7, 8, 10], characteristic: "小二度 ♭2", mood: "异域" },
    Mode { id: "lydian", name: "Lydian", cn: "利底亚", degree: 4, intervals: &[0, 2, 4, 6, 7, 9, 11], characteristic: "增四度 ♯4", mood: "漂浮" },
    Mode { id: "mixolydian", name: "Mixolydian", cn: "混合利底亚", degree: 5, intervals: &[0, 2, 4, 5, 7, 9, 10], characteristic: "小七度 ♭7", mood: "松弛" },
    Mode { id: "aeolian", name: "Aeolian", cn: "爱奥利亚", degree: 6, intervals: &[0, 2, 3, 5, 7, 8, 10], characteristic: "小六度 ♭6", mood: "忧郁" },
    Mode { id: "locrian", name: "Locrian", cn: "洛克里亚", degree: 7, intervals: &[0, 1, 3, 5, 6, 8, 10], characteristic: "减五度 ♭5", mood: "不安" },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interval {
    pub semitones: u8,
    pub id: &'static str,
    pub short: &'static str,
    pub name: &'static str,
    pub quality: &'static str,
}

pub const INTERVALS: &[Interval] = &[
    Interval { semitones: 0, id: "P1", short: "纯一度", name: "纯一度", quality: "perfect" },
    Interval { semitones: 1, id: "m2", short: "小二度", name: "小二度", quality: "minor" },
    Interval { semitones: 2, id: "M2", short: "大二度", name: "大二度", quality: "major" },
    Interval { semitones: 3, id: "m3", short: "小三度", name: "小三度", quality: "minor" },
    Interval { semitones: 4, id: "M3", short: "大三度", name: "大三度", quality: "major" },
    Interval { semitones: 5, id: "P4", short: "纯四度", name: "纯四度", quality: "perfect" },
    Interval { semitones: 6, id: "TT", short: "三全音", name: "增四度 / 减五度", quality: "augmented" },
    Interval { semitones: 7, id: "P5", short: "纯五度", name: "纯五度", quality: "perfect" },
    Interval { semitones: 8, id: "m6", short: "小六度", name: "小六度", quality: "minor" },
    Interval { semitones: 9, id: "M6", short: "大六度", name: "大六度", quality: "major" },
    Interval { semitones: 10, id: "m7", short: "小七度", name: "小七度", quality: "minor" },
    Interval { semitones: 11, id: "M7", short: "大七度", name: "大七度", quality: "major" },
    Interval { semitones: 12, id: "P8", short: "纯八度", name: "纯八度", quality: "perfect" },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub degrees: &'static [u8],
    pub roman: &'static str,
    pub note: &'static str,
}

/// Ids match `PRESETS` in the handbook so the two clients stay interchangeable.
pub const PRESETS: &[Preset] = &[
    Preset { id: "canon", label: "卡农进行", degrees: &[0, 4, 5, 2, 3, 0, 3, 4], roman: "I–V–vi–iii–IV–I–IV–V", note: "八小节，低音下行四度循环" },
    Preset { id: "p4536251", label: "4536251", degrees: &[3, 4, 2, 5, 1, 4, 0], roman: "IV–V–iii–vi–ii–V–I", note: "七小节，华语流行最常见的收束" },
    Preset { id: "p1645", label: "1645", degrees: &[0, 5, 3, 4], roman: "I–vi–IV–V", note: "四小节，最直接的循环" },
    Preset { id: "p1564", label: "1564", degrees: &[0, 4, 5, 3], roman: "I–V–vi–IV", note: "四小节，副歌常用" },
    Preset { id: "p4536", label: "4536", degrees: &[3, 4, 2, 5], roman: "IV–V–iii–vi", note: "四小节，停在六级留白" },
    Preset { id: "cad", label: "ii–V–I 进行", degrees: &[1, 4, 0, 0], roman: "ii–V–I–I", note: "四小节，爵士最基本的终止式" },
    Preset { id: "desc", label: "I–iii–IV–V", degrees: &[0, 2, 3, 4], roman: "I–iii–IV–V", note: "四小节，级进上行" },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meter {
    pub id: &'static str,
    pub beats: u8,
    pub hint: &'static str,
    pub accents: &'static [u8],
}

pub const METERS: &[Meter] = &[
    Meter { id: "4/4", beats: 4, hint: "强、弱、次强、弱", accents: &[0, 2] },
    Meter { id: "3/4", beats: 3, hint: "一个重心，两次旋转", accents: &[0] },
    Meter { id: "6/8", beats: 6, hint: "两组三连音", accents: &[0, 3] },
    Meter { id: "5/4", beats: 5, hint: "3 + 2 的不对称", accents: &[0, 2] },
    Meter { id: "7/8", beats: 7, hint: "2 + 2 + 3", accents: &[0, 2, 4] },
];

/// Standard tuning, low E to high E.
pub const STANDARD_TUNING: [i32; 6] = [40, 45, 50, 55, 59, 64];

pub fn pc(midi: i32) -> u8 {
    (((midi % 12) + 12) % 12) as u8
}

pub fn note_name(midi: i32) -> String {
    let pc = pc(midi) as usize;
    format!("{}{}", SHARP_NAMES[pc], midi / 12 - 1)
}

pub fn pc_name(pc: u8) -> &'static str {
    SHARP_NAMES[(pc % 12) as usize]
}

pub fn midi_to_freq(midi: f64) -> f64 {
    440.0 * 2f64.powf((midi - 69.0) / 12.0)
}

pub fn freq_to_midi(freq: f64) -> f64 {
    69.0 + 12.0 * (freq / 440.0).log2()
}

pub fn cents_between(freq: f64, reference: f64) -> f64 {
    1200.0 * (freq / reference).log2()
}

/// Spells `pc` with a forced letter, e.g. (6, F) -> "F♯", (1, D) -> "D♭".
pub fn spell(pc: u8, letter: usize) -> (String, i8) {
    let natural = [0u8, 2, 4, 5, 7, 9, 11][letter % 7] as i8;
    let mut diff = (pc as i8 - natural).rem_euclid(12);
    if diff > 6 {
        diff -= 12;
    }
    let mut name = String::from(LETTERS[letter % 7]);
    match diff {
        d if d > 0 => name.push_str(&"♯".repeat(d as usize)),
        d if d < 0 => name.push_str(&"♭".repeat((-d) as usize)),
        _ => {}
    }
    (name, diff)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tone {
    pub midi: i32,
    pub pc: u8,
    pub name: String,
    pub degree: &'static str,
    pub letter: usize,
    pub accidental: i8,
}

pub fn chord_quality(id: &str) -> Option<&'static ChordQuality> {
    CHORDS.iter().find(|q| q.id == id)
}

pub fn chord_tones(root_pc: u8, quality_id: &str, inversion: u8) -> Option<Vec<Tone>> {
    let quality = chord_quality(quality_id)?;
    let root_letter = ROOT_LETTER[(root_pc % 12) as usize];
    let mut tones: Vec<Tone> = quality
        .intervals
        .iter()
        .enumerate()
        .map(|(i, semi)| {
            let tone_pc = (root_pc + semi) % 12;
            let letter = (root_letter + i * 2) % 7;
            let (name, accidental) = spell(tone_pc, letter);
            Tone {
                midi: 60 + root_pc as i32 + *semi as i32,
                pc: tone_pc,
                name,
                degree: quality.degrees.get(i).copied().unwrap_or(""),
                letter,
                accidental,
            }
        })
        .collect();
    let inv = (inversion as usize).min(tones.len().saturating_sub(1));
    for tone in tones.iter_mut().take(inv) {
        tone.midi += 12;
    }
    tones.sort_by_key(|t| t.midi);
    Some(tones)
}

pub fn chord_symbol(root_pc: u8, quality_id: &str, bass: Option<&str>) -> String {
    let q = chord_quality(quality_id).map(|q| q.symbol).unwrap_or("");
    let mut s = format!("{}{}", pc_name(root_pc), q);
    if let Some(bass) = bass {
        s.push('/');
        s.push_str(bass);
    }
    s
}

pub fn scale(id: &str) -> Option<&'static Scale> {
    SCALES.iter().find(|s| s.id == id)
}

pub fn scale_semitones(id: &str) -> Option<Vec<u8>> {
    let s = scale(id)?;
    let mut out = vec![0u8];
    let mut acc = 0u8;
    for step in s.steps {
        acc = acc.saturating_add(*step);
        out.push(acc);
    }
    Some(out)
}

pub fn scale_pcs(tonic_pc: u8, id: &str) -> Vec<u8> {
    scale_semitones(id)
        .map(|steps| steps.iter().map(|s| (tonic_pc + s) % 12).collect())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleTone {
    pub midi: i32,
    pub pc: u8,
    pub name: String,
    pub degree: &'static str,
    pub accidental: i8,
}

pub fn scale_tones(tonic_pc: u8, id: &str) -> Option<Vec<ScaleTone>> {
    let s = scale(id)?;
    let semis = scale_semitones(id)?;
    let root_letter = ROOT_LETTER[(tonic_pc % 12) as usize];
    Some(
        semis
            .iter()
            .enumerate()
            .map(|(i, semi)| {
                let midi = 60 + tonic_pc as i32 + *semi as i32;
                let letter = (root_letter + *s.letters.get(i).unwrap_or(&(i as u8)) as usize) % 7;
                let (name, accidental) = spell((midi % 12) as u8, letter);
                ScaleTone { midi, pc: (midi % 12) as u8, name, degree: s.degrees.get(i).copied().unwrap_or("8"), accidental }
            })
            .collect(),
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySignature {
    pub key_pc: u8,
    pub key_name: String,
    pub accidentals: i8,
    pub accidental_kind: &'static str,
    pub count: u8,
    pub relative_minor_pc: u8,
    pub relative_minor: String,
    pub diatonic: Vec<DiatonicChord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiatonicChord {
    pub degree: u8,
    pub roman: &'static str,
    pub root_pc: u8,
    pub quality: &'static str,
    pub symbol: String,
}

/// Fewest-accidental spelling for each major tonic.
const MAJOR_SIGNATURE: [(u8, i8); 12] = [
    (0, 0), (1, -5), (2, 2), (3, -3), (4, 4), (5, -1),
    (6, 6), (7, 1), (8, -4), (9, 3), (10, -2), (11, 5),
];

pub fn key_signature(key_pc: u8) -> KeySignature {
    let key_pc = key_pc % 12;
    let (_, accidentals) = MAJOR_SIGNATURE[key_pc as usize];
    let use_flats = accidentals < 0;
    // Spell the tonic with the letter that matches the key signature.
    let tonic_letter = if use_flats {
        // Flat keys: C, D♭, D, E♭, E, F, G♭, G, A♭, A, B♭, B
        [0usize, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6][key_pc as usize]
    } else {
        LETTER_OF_PC[key_pc as usize]
    };
    let (key_name, _) = spell(key_pc, tonic_letter);
    let minor_pc = (key_pc + 9) % 12;
    let minor_letter = (tonic_letter + 5) % 7;
    let (minor_name, _) = spell(minor_pc, minor_letter);

    const QUAL: [&str; 7] = ["maj", "min", "min", "maj", "maj", "min", "dim"];
    const ROMAN: [&str; 7] = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];
    let degrees = [0u8, 2, 4, 5, 7, 9, 11];
    let diatonic = degrees
        .iter()
        .enumerate()
        .map(|(i, semi)| {
            let root = (key_pc + semi) % 12;
            DiatonicChord {
                degree: (i + 1) as u8,
                roman: ROMAN[i],
                root_pc: root,
                quality: QUAL[i],
                symbol: chord_symbol(root, QUAL[i], None),
            }
        })
        .collect();

    KeySignature {
        key_pc,
        key_name,
        accidentals,
        accidental_kind: if accidentals < 0 { "flat" } else { "sharp" },
        count: accidentals.unsigned_abs(),
        relative_minor_pc: minor_pc,
        relative_minor: format!("{} 小调", minor_name),
        diatonic,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordMatch {
    pub root_pc: u8,
    pub quality: &'static str,
    pub symbol: String,
    pub label: &'static str,
    pub bass_pc: u8,
    pub inversion: u8,
    pub score: i32,
}

/// Exact pitch-class matches, plus a tolerant pass that allows a missing fifth.
pub fn identify_chords(midis: &[i32]) -> Vec<ChordMatch> {
    if midis.is_empty() {
        return Vec::new();
    }
    let mut pcs: Vec<u8> = midis.iter().map(|m| pc(*m)).collect();
    pcs.sort_unstable();
    pcs.dedup();
    let bass_pc = pc(*midis.iter().min().unwrap());
    let mut matches: Vec<ChordMatch> = Vec::new();

    for root in 0..12u8 {
        for quality in CHORDS {
            let template: Vec<u8> = quality.intervals.iter().map(|i| (root + i) % 12).collect();
            let exact = template.len() == pcs.len() && template.iter().all(|p| pcs.contains(p));
            let missing_fifth = quality.intervals.contains(&7)
                && template.len() == pcs.len() + 1
                && template.iter().filter(|p| **p != (root + 7) % 12).all(|p| pcs.contains(p));
            if !exact && !missing_fifth {
                continue;
            }
            let bass_index = midis.iter().copied().min().map(pc).unwrap_or(bass_pc);
            let inversion = template.iter().position(|p| *p == bass_index).unwrap_or(0) as u8;
            let mut score = if exact { 100 } else { 82 };
            if root == bass_pc {
                score += 12;
            }
            if quality.intervals.len() == pcs.len() {
                score += 4;
            }
            matches.push(ChordMatch {
                root_pc: root,
                quality: quality.id,
                symbol: chord_symbol(root, quality.id, if inversion > 0 { Some(pc_name(bass_index)) } else { None }),
                label: quality.label,
                bass_pc,
                inversion,
                score,
            });
        }
    }
    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.root_pc.cmp(&b.root_pc)));
    matches.truncate(8);
    matches
}

pub fn interval_by_semitones(semitones: i32) -> Option<&'static Interval> {
    INTERVALS.get(semitones.rem_euclid(12) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_and_frequency() {
        assert_eq!(pc(-1), 11);
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(69), "A4");
        assert!((midi_to_freq(69.0) - 440.0).abs() < 1e-9);
        assert!((midi_to_freq(81.0) - 880.0).abs() < 1e-9);
        assert!((cents_between(466.16, 440.0) - 100.0).abs() < 0.5);
    }

    #[test]
    fn spelling_respects_letter_context() {
        assert_eq!(spell(6, 3).0, "F♯");
        assert_eq!(spell(6, 4).0, "G♭");
        assert_eq!(spell(10, 6).0, "B♭");
        assert_eq!(spell(1, 1).0, "D♭");
        assert_eq!(spell(0, 0).0, "C");
    }

    #[test]
    fn cmaj7_tones_and_inversion() {
        let tones = chord_tones(0, "maj7", 0).unwrap();
        assert_eq!(tones.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["C", "E", "G", "B"]);
        let inv = chord_tones(0, "maj7", 1).unwrap();
        assert_eq!(inv[0].name, "E");
        assert_eq!(inv[0].midi, 64);
        assert_eq!(inv[3].midi, 72); /* C5 on top */
    }

    #[test]
    fn flat_key_spelling() {
        let tones = chord_tones(10, "maj", 0).unwrap();
        assert_eq!(tones.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["B♭", "D", "F"]);
    }

    #[test]
    fn scales_cover_expected_pitch_classes() {
        assert_eq!(scale_pcs(0, "major"), vec![0, 2, 4, 5, 7, 9, 11, 0]);
        assert_eq!(scale_pcs(9, "minor"), vec![9, 11, 0, 2, 4, 5, 7, 9]);
        assert!(scale("blues").is_some());
        assert!(scale("nope").is_none());
    }

    #[test]
    fn mode_table_is_consistent() {
        assert_eq!(MODES.len(), 7);
        for (i, mode) in MODES.iter().enumerate() {
            assert_eq!(mode.degree as usize, i + 1);
            assert_eq!(mode.intervals.len(), 7);
            assert_eq!(mode.intervals[0], 0);
        }
    }

    #[test]
    fn key_signatures_and_relative_minor() {
        assert_eq!(key_signature(2).count, 2); /* D major */
        assert_eq!(key_signature(2).accidental_kind, "sharp");
        assert_eq!(key_signature(5).count, 1);
        assert_eq!(key_signature(5).accidental_kind, "flat");
        assert_eq!(key_signature(0).relative_minor_pc, 9);
        assert_eq!(key_signature(0).diatonic.len(), 7);
        assert_eq!(key_signature(0).diatonic[6].quality, "dim");
    }

    #[test]
    fn chord_identification() {
        let matches = identify_chords(&[60, 64, 67, 71]);
        assert_eq!(matches[0].symbol, "Cmaj7");
        assert_eq!(matches[0].root_pc, 0);
        let inv = identify_chords(&[64, 67, 71, 72]);
        assert!(inv.iter().any(|m| m.symbol == "Cmaj7/E"));
        let triad = identify_chords(&[60, 64, 67]);
        assert_eq!(triad[0].symbol, "C");
    }

    #[test]
    fn presets_match_handbook() {
        assert_eq!(PRESETS.len(), 7);
        assert_eq!(PRESETS[0].id, "canon");
        assert_eq!(PRESETS[0].degrees.len(), 8);
        assert_eq!(PRESETS[1].id, "p4536251");
    }
}

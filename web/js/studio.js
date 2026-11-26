import {
  $, $$, api, copyText, fretSVG, mountChrome, renderKeyboard, staffSVG, toast, Sound,
} from "./app.js";
import { CHORDS, PRESETS, ROOT_NAMES, ROMAN, progression } from "./theory.js";

mountChrome();

const state = {
  root: 0,
  quality: "maj7",
  inversion: 0,
  keyPc: 0,
  tempo: 96,
  degrees: [0, 4, 5, 3],
  presetId: "p1564",
  seventh: false,
  voicing: null,
  catalog: null,
};

const QUALITY_BY_ID = new Map(CHORDS.map((q) => [q.id, q]));

function param(name, fallback = null) {
  return new URLSearchParams(location.search).get(name) ?? fallback;
}

function readUrl() {
  const key = param("key");
  const root = param("root");
  const quality = param("quality");
  const inv = param("inv");
  const tempo = param("tempo");
  const degrees = param("degrees");
  if (key != null) state.keyPc = Math.max(0, Math.min(11, Number(key) || 0));
  if (root != null) state.root = Math.max(0, Math.min(11, Number(root) || 0));
  if (quality && QUALITY_BY_ID.has(quality)) state.quality = quality;
  if (inv != null) state.inversion = Math.max(0, Math.min(3, Number(inv) || 0));
  if (tempo != null) state.tempo = Math.max(40, Math.min(240, Number(tempo) || 96));
  if (degrees) {
    const parsed = degrees.split(".").map(Number).filter((d) => Number.isInteger(d) && d >= 0 && d <= 6);
    if (parsed.length >= 1 && parsed.length <= 16) {
      state.degrees = parsed;
      state.presetId = null;
    }
  }
}

function writeUrl() {
  const query = new URLSearchParams({
    key: String(state.keyPc),
    root: String(state.root),
    quality: state.quality,
    inv: String(state.inversion),
    tempo: String(state.tempo),
    degrees: state.degrees.join("."),
  });
  history.replaceState(null, "", `${location.pathname}?${query}`);
}

/* ---------------------------------------------------------------- controls */

function buildControls() {
  const rootSel = $("#rootSel");
  const qualitySel = $("#qualitySel");
  rootSel.innerHTML = ROOT_NAMES.map((name, pc) => `<option value="${pc}">${name}</option>`).join("");
  qualitySel.innerHTML = CHORDS.map((q) => `<option value="${q.id}">${q.label} · ${q.symbol || "maj"}</option>`).join("");
  rootSel.value = String(state.root);
  qualitySel.value = state.quality;
  rootSel.addEventListener("change", () => {
    state.root = Number(rootSel.value);
    loadChord();
  });
  qualitySel.addEventListener("change", () => {
    state.quality = qualitySel.value;
    state.inversion = 0;
    loadChord();
  });

  $("#progKey").innerHTML = ROOT_NAMES.map((name, pc) => `<option value="${pc}">${name} 大调</option>`).join("");
  $("#progKey").value = String(state.keyPc);
  $("#progKey").addEventListener("change", () => {
    state.keyPc = Number($("#progKey").value);
    state.presetId = null;
    renderProgression();
    persist();
  });

  const tempo = $("#progTempo");
  tempo.value = String(state.tempo);
  $("#progTempoOut").textContent = `${state.tempo} BPM`;
  tempo.addEventListener("input", () => {
    state.tempo = Number(tempo.value);
    $("#progTempoOut").textContent = `${state.tempo} BPM`;
    persist();
  });

  $$("#progVoicing button").forEach((button) => {
    button.addEventListener("click", () => {
      state.seventh = button.dataset.seventh === "true";
      $$("#progVoicing button").forEach((b) => b.setAttribute("aria-pressed", String(b === button)));
      renderProgression();
      persist();
    });
  });
}

function renderInversionSeg() {
  const seg = $("#invSeg");
  const quality = QUALITY_BY_ID.get(state.quality);
  const max = (quality?.intervals.length || 3) - 1;
  if (state.inversion > max) state.inversion = max;
  const labels = ["根位", "第一转位", "第二转位", "第三转位"];
  seg.innerHTML = labels
    .map((label, i) => `<button type="button" data-inv="${i}" ${i > max ? "disabled" : ""} aria-pressed="${i === state.inversion}">${label}</button>`)
    .join("");
  $$("button", seg).forEach((button) => {
    button.addEventListener("click", () => {
      state.inversion = Number(button.dataset.inv);
      renderInversionSeg();
      loadChord();
    });
  });
}

/* ---------------------------------------------------------------- chord */

let lastChord = null;

async function loadChord() {
  renderInversionSeg();
  const query = `root=${state.root}&quality=${state.quality}&inversion=${state.inversion}`;
  try {
    const [chord, voicings] = await Promise.all([
      api(`/api/theory/chord?${query}`),
      api(`/api/theory/voicings?${query}&limit=6&minStrings=4`),
    ]);
    lastChord = chord;
    renderChord(chord, voicings);
    writeUrl();
  } catch (error) {
    toast(error.message || "和弦加载失败", "error");
  }
}

function renderChord(chord, voicingData) {
  $("#chordSymbol").textContent = chord.symbol;
  $("#chordMeta").textContent = `${chord.tones.length} 音 · ${chord.quality.label}`;
  $("#chordBass").textContent = `${chord.quality.label} · 低音 ${chord.bass} · ${chord.tones.length} 音`;
  $("#chordStaff").innerHTML = staffSVG(chord.tones, { chord: true, width: 720, height: 168, labels: false });
  renderKeyboard($("#chordKeys"), {
    start: 60,
    end: 84,
    active: chord.midis,
    labels: [60, 72],
    onKey: (midi) => Sound.tone(midi, { dur: 0.9 }),
  });
  $("#chordTones").innerHTML = chord.tones
    .map((tone, i) => `<li><b>${tone.name}</b><span>${tone.degree}${i === 0 ? " · 低音" : ""}</span></li>`)
    .join("");
  $("#chordEngine").textContent = `C++ · ${(voicingData.tookUs / 1000).toFixed(2)} ms`;

  const grid = $("#voicingGrid");
  const list = voicingData.voicings || [];
  $("#voicingCount").textContent = list.length ? `${list.length} 个可按形状` : "无解";
  if (!list.length) {
    grid.innerHTML = '<p class="voicing__empty">这个转位在 15 品内没有满足条件的按法。换一个转位，或把跨品范围放宽。</p>';
    $("#voicingStage").innerHTML = "";
    $("#playVoicing").disabled = true;
    state.voicing = null;
    return;
  }
  grid.innerHTML = "";
  list.forEach((voicing, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "voicing";
    button.setAttribute("aria-pressed", String(index === 0));
    button.innerHTML = `${fretSVG(voicing, { width: 132, height: 156 })}<span class="voicing__foot"><span>${voicing.span} 品跨度</span><span>${voicing.maxFret} 品</span></span>`;
    button.addEventListener("click", () => selectVoicing(voicing, button));
    grid.append(button);
  });
  selectVoicing(list[0], grid.firstElementChild, false);
}

function selectVoicing(voicing, button, play = true) {
  state.voicing = voicing;
  $$("#voicingGrid .voicing").forEach((node) => node.setAttribute("aria-pressed", String(node === button)));
  $("#voicingStage").innerHTML = fretSVG(voicing, { width: 300, height: 240 });
  $("#voicingMeta").textContent = `第 ${voicing.maxFret} 品 · ${voicing.noteCount} 音 · 得分 ${voicing.score}`;
  $("#playVoicing").disabled = false;
  if (play) Sound.chord(voicing.midis, { dur: 1.5, gain: 0.075, spread: 0.02 });
}

/* ---------------------------------------------------------------- progression */

let raf = 0;
let playStart = 0;
let playing = false;

function renderProgression() {
  const chords = progression(state.keyPc, state.degrees, { seventh: state.seventh });
  const slots = $("#progSlots");
  slots.innerHTML = "";
  const grid = document.createElement("div");
  grid.className = "slots";
  grid.style.setProperty("--n", String(chords.length));
  chords.forEach((chord, index) => {
    const slot = document.createElement("div");
    slot.className = "slot";
    slot.dataset.index = String(index);
    slot.innerHTML = `
      <span class="rn">${chord.roman}</span>
      <span class="sym">${chord.symbol}</span>
      <select aria-label="第 ${index + 1} 小节的和弦">${ROMAN.map((r, d) => `<option value="${d}" ${d === state.degrees[index] ? "selected" : ""}>${r}</option>`).join("")}</select>`;
    slot.querySelector("select").addEventListener("change", (event) => {
      state.degrees[index] = Number(event.target.value);
      state.presetId = null;
      renderProgression();
      persist();
    });
    grid.append(slot);
  });
  slots.append(grid);
  $("#progHint").textContent = `${state.degrees.length} 小节 · 每小节 4 拍 · ${state.seventh ? "七和弦" : "三和弦"}`;

  $("#progTable").innerHTML = chords
    .map((chord, i) => `<tr><td>${i + 1}</td><td>${chord.roman}</td><td>${chord.symbol}</td><td>${chord.tones.map((t) => t.name).join(" ")}</td></tr>`)
    .join("");

  $$("#progPresets .chip").forEach((chip) => {
    chip.setAttribute("aria-pressed", String(chip.dataset.preset === state.presetId));
  });
}

function buildPresets() {
  const host = $("#progPresets");
  host.innerHTML = "";
  PRESETS.forEach((preset) => {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = "chip";
    chip.dataset.preset = preset.id;
    chip.textContent = preset.label;
    chip.title = preset.roman;
    chip.setAttribute("aria-pressed", "false");
    chip.addEventListener("click", () => {
      state.degrees = preset.degrees.slice();
      state.presetId = preset.id;
      renderProgression();
      persist();
    });
    host.append(chip);
  });
}

function stopPlayback() {
  cancelAnimationFrame(raf);
  playing = false;
  Sound.stopAll();
  $$("#progSlots .slot").forEach((slot) => slot.classList.remove("is-now"));
  $("#progPlay").textContent = "播放";
  $("#progStop").disabled = true;
}

function playProgression() {
  if (playing) {
    stopPlayback();
    return;
  }
  Sound.resume();
  if (!Sound.ctx) {
    toast("浏览器没有可用的音频上下文", "error");
    return;
  }
  const bpm = state.tempo;
  const beat = 60 / bpm;
  const chords = progression(state.keyPc, state.degrees, { seventh: state.seventh });
  const total = chords.length * 4 * beat;
  const start = Sound.now() + 0.08;
  playStart = start;
  playing = true;
  $("#progPlay").textContent = "停止";
  $("#progStop").disabled = false;

  chords.forEach((chord, index) => {
    Sound.chord(chord.midis, { when: index * 4 * beat, dur: Math.min(3.4 * beat, 2.4), gain: 0.07, spread: 0.018 });
    for (let b = 0; b < 4; b++) {
      if (b === 0) Sound.click(true, index * 4 * beat + b * beat);
    }
  });

  const tick = () => {
    if (!playing) return;
    const elapsed = Sound.now() - playStart;
    const index = Math.floor(elapsed / (4 * beat));
    $$("#progSlots .slot").forEach((slot) => {
      slot.classList.toggle("is-now", Number(slot.dataset.index) === index);
    });
    if (elapsed >= total) {
      setTimeout(stopPlayback, 120);
      return;
    }
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
}

/* ---------------------------------------------------------------- persistence */

let persistTimer = 0;
function persist() {
  writeUrl();
  clearTimeout(persistTimer);
  persistTimer = setTimeout(async () => {
    try {
      await api("/api/state", {
        method: "PUT",
        body: {
          keyPc: state.keyPc,
          tempo: state.tempo,
          presetId: state.presetId || "custom",
          degrees: state.degrees,
        },
      });
    } catch {
      /* offline keeps working */
    }
  }, 450);
}

async function loadServerState() {
  if (param("degrees") || param("key")) return;
  try {
    const saved = await api("/api/state");
    if (Number.isInteger(saved.keyPc)) state.keyPc = saved.keyPc;
    if (Number.isInteger(saved.tempo)) state.tempo = saved.tempo;
    if (Array.isArray(saved.degrees) && saved.degrees.length) state.degrees = saved.degrees;
    state.presetId = saved.presetId || null;
    $("#progKey").value = String(state.keyPc);
    $("#progTempo").value = String(state.tempo);
    $("#progTempoOut").textContent = `${state.tempo} BPM`;
  } catch {
    /* default progression */
  }
}

async function loadCustomPresets() {
  const host = $("#customPresets");
  try {
    const data = await api("/api/presets");
    if (!data.custom.length) {
      host.innerHTML = '<p class="small muted" style="padding:10px 0">还没有保存的进行。调整上面的级数，然后点“保存”。</p>';
      return;
    }
    host.innerHTML = "";
    data.custom.forEach((preset) => {
      const row = document.createElement("div");
      row.className = "rowitem";
      row.innerHTML = `<div class="rowitem__main"><b>${escapeHtml(preset.name)}</b><span>${ROOT_NAMES[preset.keyPc]} 大调 · ${preset.tempo} BPM · ${preset.degrees.length} 小节</span></div>`;
      const side = document.createElement("div");
      side.className = "rowitem__side";
      const load = document.createElement("button");
      load.type = "button";
      load.className = "chip";
      load.textContent = "载入";
      load.addEventListener("click", () => {
        state.keyPc = preset.keyPc;
        state.tempo = preset.tempo;
        state.degrees = preset.degrees.slice();
        state.presetId = preset.id;
        $("#progKey").value = String(state.keyPc);
        $("#progTempo").value = String(state.tempo);
        $("#progTempoOut").textContent = `${state.tempo} BPM`;
        renderProgression();
        persist();
      });
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "chip";
      remove.textContent = "删除";
      remove.addEventListener("click", async () => {
        try {
          await api(`/api/presets/${preset.id}`, { method: "DELETE" });
          loadCustomPresets();
          toast("已删除");
        } catch (error) {
          toast(error.message, "error");
        }
      });
      side.append(load, remove);
      row.append(side);
      host.append(row);
    });
  } catch {
    host.innerHTML = '<p class="small muted" style="padding:10px 0">无法读取已保存的进行。</p>';
  }
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[char]));
}

async function savePreset() {
  const input = $("#presetName");
  const name = input.value.trim() || `${ROOT_NAMES[state.keyPc]} 大调 · ${state.degrees.length} 小节`;
  try {
    await api("/api/presets", {
      method: "POST",
      body: { name, degrees: state.degrees, keyPc: state.keyPc, tempo: state.tempo },
    });
    input.value = "";
    await loadCustomPresets();
    toast("进行已保存到档案");
  } catch (error) {
    toast(error.message, "error");
  }
}

async function share() {
  try {
    const data = await api("/api/shares", {
      method: "POST",
      body: {
        keyPc: state.keyPc,
        tempo: state.tempo,
        presetId: state.presetId,
        degrees: state.degrees,
        instrument: "piano",
      },
    });
    const url = `${location.origin}${data.url}`;
    $("#shareOut").hidden = false;
    $("#shareUrl").textContent = url;
    $("#openShare").href = url;
    notifyShare(url);
  } catch (error) {
    toast(error.message, "error");
  }
}

function notifyShare(url) {
  copyText(url).then((ok) => toast(ok ? "链接已复制" : "已生成链接，可手动复制"));
}

/* ---------------------------------------------------------------- boot */

async function boot() {
  readUrl();
  buildControls();
  buildPresets();
  await loadServerState();
  renderProgression();
  await loadChord();
  await loadCustomPresets();

  $("#playChord").addEventListener("click", () => {
    if (!lastChord) return;
    Sound.chord(lastChord.midis, { dur: 1.9, gain: 0.075 });
  });
  $("#playArp").addEventListener("click", () => {
    if (!lastChord) return;
    Sound.arp([...lastChord.midis].sort((a, b) => a - b), { step: 0.13, dur: 1.1 });
  });
  $("#playVoicing").addEventListener("click", () => {
    if (state.voicing) Sound.chord(state.voicing.midis, { dur: 1.6, gain: 0.078, spread: 0.02 });
  });
  $("#progPlay").addEventListener("click", playProgression);
  $("#progStop").addEventListener("click", stopPlayback);
  $("#savePreset").addEventListener("click", savePreset);
  $("#shareBtn").addEventListener("click", share);
  $("#copyShare").addEventListener("click", () => copyText($("#shareUrl").textContent).then((ok) => toast(ok ? "已复制" : "复制失败，请手动选择")));
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && playing) stopPlayback();
  });
}

boot();

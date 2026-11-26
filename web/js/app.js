/* Shared browser runtime: API client, audio, staff/fretboard renderers, chrome. */
import { PC_NAMES, chordTones, midiToFreq, noteName, pcOf } from "./theory.js";

export const $ = (selector, root = document) => root.querySelector(selector);
export const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

export function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value == null || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "html") node.innerHTML = value;
    else if (key.startsWith("on")) node.addEventListener(key.slice(2).toLowerCase(), value);
    else if (key === "dataset") Object.assign(node.dataset, value);
    else node.setAttribute(key, value === true ? "" : String(value));
  }
  for (const child of children.flat()) {
    if (child == null) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return node;
}

/* ---------------------------------------------------------------- identity */

export function deviceId() {
  const key = "temper-device";
  try {
    let id = localStorage.getItem(key);
    if (!id || !/^[A-Za-z0-9_-]{6,64}$/.test(id)) {
      id = (crypto.randomUUID?.() || `dev-${Date.now()}-${Math.random().toString(36).slice(2)}`).replace(/[^A-Za-z0-9_-]/g, "");
      localStorage.setItem(key, id);
    }
    return id;
  } catch {
    return "anonymous-device";
  }
}

export class ApiError extends Error {
  constructor(status, code, message, details) {
    super(message);
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

export async function api(path, { method = "GET", body, signal } = {}) {
  const headers = { "x-temper-device": deviceId() };
  let payload;
  if (body !== undefined) {
    headers["content-type"] = "application/json";
    payload = JSON.stringify(body);
  }
  const response = await fetch(path, { method, headers, body: payload, signal });
  const text = await response.text();
  let data = null;
  if (text) {
    try { data = JSON.parse(text); } catch { data = { raw: text }; }
  }
  if (!response.ok) {
    const error = data?.error || {};
    throw new ApiError(response.status, error.code || "http_error", error.message || `请求失败（${response.status}）`, error.details);
  }
  return data;
}

/* ---------------------------------------------------------------- toast */

let toastNode = null;
let toastTimer = 0;
export function toast(message, kind = "info") {
  if (!toastNode) {
    toastNode = el("div", { class: "toast", role: "status", "aria-live": "polite" });
    document.body.append(toastNode);
  }
  toastNode.textContent = message;
  toastNode.dataset.kind = kind;
  toastNode.classList.add("is-on");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastNode.classList.remove("is-on"), 2600);
}

export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = el("textarea", { style: "position:fixed;left:-999px;top:0" });
    area.value = text;
    document.body.append(area);
    area.select();
    const ok = document.execCommand?.("copy");
    area.remove();
    return Boolean(ok);
  }
}

/* ---------------------------------------------------------------- audio */

/* ---------------------------------------------------------------- piano samples
   Salamander grand piano (CC BY 3.0): one recording per major third, C2–C7, in
   assets/piano-samples.js. Decoded through an OfflineAudioContext at startup, so no audio
   context opens before a gesture and the notes are normally ready before the first click.
   Play-back corrects each recording's measured cents, so every key lands on equal temperament. */
const PIANO_SAMPLE_TUNE = { 36: -11.1, 42: -5.4, 45: -5.6, 48: -5.8, 54: -6.1, 57: 0.7, 60: -1.5, 66: -2.8, 69: 0.9, 72: 4.5, 78: 4.5, 81: 7.1, 84: 8.7, 90: 12.0, 93: 14.8, 96: 23.5 };
const PianoSamples = {
  buffers: new Map(),
  ready: false,
  loading: null,
  failed: false,

  load() {
    if (this.loading) return this.loading;
    const AC = window.OfflineAudioContext || window.webkitOfflineAudioContext;
    if (!AC) {
      this.failed = true;
      return Promise.resolve();
    }
    this.loading = ensurePianoSamples()
      .then(() => {
        const data = window.PIANO_SAMPLE_DATA;
        if (!data) throw new Error("missing piano sample data");
        const ctx = new AC(1, 1, 44100);
        const decode64 = (uri) => {
          const bin = atob(uri.slice(uri.indexOf(",") + 1));
          const bytes = new Uint8Array(bin.length);
          for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
          return bytes.buffer;
        };
        return Promise.all(
          Object.entries(data).map(async ([root, uri]) => {
            try {
              this.buffers.set(Number(root), await ctx.decodeAudioData(decode64(uri)));
            } catch {}
          })
        );
      })
      .then(() => {
        this.ready = this.buffers.size >= 8;
        this.failed = !this.ready;
      })
      .catch(() => {
        this.failed = true;
      })
      .finally(() => {
        this.loading = null;
      });
    return this.loading;
  },

  pick(midi) {
    if (!this.ready) return null;
    let best = null;
    let dist = Infinity;
    this.buffers.forEach((buf, root) => {
      const d = Math.abs(root - midi);
      if (d < dist) {
        dist = d;
        best = { root, buf, tune: PIANO_SAMPLE_TUNE[root] || 0 };
      }
    });
    return best;
  },
};

function ensurePianoSamples() {
  if (window.PIANO_SAMPLE_DATA) return Promise.resolve();
  if (window.__pianoSamplesPromise) return window.__pianoSamplesPromise;
  window.__pianoSamplesPromise = new Promise((resolve, reject) => {
    const s = document.createElement("script");
    s.src = new URL("../assets/piano-samples.js", import.meta.url).href;
    s.async = true;
    const timer = setTimeout(() => reject(new Error("sample script timed out")), 10000);
    s.onload = () => { clearTimeout(timer); resolve(); };
    s.onerror = () => { clearTimeout(timer); reject(new Error("sample script failed")); };
    document.head.appendChild(s);
  });
  return window.__pianoSamplesPromise;
}
PianoSamples.load();

export const Sound = {
  ctx: null,
  master: null,
  on: true,
  timbre: "piano",
  voices: new Set(),
  pianoVoices: new Set(),
  maxVoices: 40,

  boot() {
    if (this.ctx) return;
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    this.ctx = new AC();
    this.master = this.ctx.createGain();
    this.master.gain.value = this.on ? 0.8 : 0;
    const comp = this.ctx.createDynamicsCompressor();
    comp.threshold.value = -16;
    comp.knee.value = 24;
    comp.ratio.value = 3.2;
    comp.attack.value = 0.004;
    comp.release.value = 0.24;
    this.master.connect(comp).connect(this.ctx.destination);
  },

  resume() {
    this.boot();
    if (this.ctx?.state === "suspended") this.ctx.resume();
  },

  setEnabled(on) {
    this.on = on;
    if (this.master) this.master.gain.value = on ? 0.8 : 0;
  },

  setTimbre(mode) {
    this.timbre = mode === "guitar" ? "guitar" : "piano";
  },

  now() {
    return this.ctx ? this.ctx.currentTime : 0;
  },

  /* Piano voice: a sampled grand piano (soft synth stand-in until the samples are ready). */
  tone(midi, { when = 0, dur = 1.1, gain = 0.075, timbre = this.timbre } = {}) {
    this.resume();
    if (!this.ctx || !this.on) return null;
    if (this.voices.size >= this.maxVoices) this.stopAll();
    const start = this.ctx.currentTime + Math.max(0, when);
    const f = midiToFreq(midi);

    const pick = PianoSamples.pick(midi);
    if (timbre === "guitar") {
      const env = this.ctx.createGain();
      env.gain.setValueAtTime(0, start);
      env.gain.linearRampToValueAtTime(gain, start + 0.012);
      env.gain.exponentialRampToValueAtTime(Math.max(0.0001, gain * 0.55), start + Math.min(0.3, dur * 0.35));
      env.gain.exponentialRampToValueAtTime(0.0001, start + dur);
      const a = this.ctx.createOscillator();
      const b = this.ctx.createOscillator();
      a.type = "triangle";
      b.type = "sawtooth";
      a.frequency.value = f;
      b.frequency.value = f * 2.002;
      const bGain = this.ctx.createGain();
      bGain.gain.value = 0.18;
      const filter = this.ctx.createBiquadFilter();
      filter.type = "lowpass";
      filter.frequency.value = 2600;
      filter.Q.value = 0.6;
      a.connect(env);
      b.connect(bGain).connect(env);
      env.connect(filter).connect(this.master);
      a.start(start);
      b.start(start);
      a.stop(start + dur + 0.05);
      b.stop(start + dur + 0.05);
      this.voices.add(a);
      this.voices.add(b);
      a.onended = () => { this.voices.delete(a); this.voices.delete(b); };
      return start;
    }

    /* Sampled grand piano: the recording carries its own attack and decay, `dur` is the moment
       the damper falls (bass strings ring a moment longer). Polyphony is capped separately. */
    if (pick) {
      const rate = 2 ** ((midi - pick.root) / 12 - pick.tune / 1200);
      const amp = gain * 2.4;
      if (this.pianoVoices.size >= 48) {
        const oldest = this.pianoVoices.values().next().value;
        try { oldest.stop(); } catch {}
        this.pianoVoices.delete(oldest);
      }
      const src = this.ctx.createBufferSource();
      src.buffer = pick.buf;
      src.playbackRate.value = rate;
      const g = this.ctx.createGain();
      g.gain.value = amp;
      src.connect(g).connect(this.master);
      const tau = Math.max(0.035, Math.min(0.14, 0.035 + (76 - midi) * 0.004));
      const end = start + Math.max(0.03, Math.min(dur, pick.buf.duration / rate - 0.02));
      g.gain.setValueAtTime(amp, end);
      g.gain.setTargetAtTime(0.0001, end, tau);
      src.start(start);
      src.stop(end + tau * 7 + 0.05);
      this.pianoVoices.add(src);
      src.onended = () => this.pianoVoices.delete(src);
      return start;
    }

    /* Stand-in while the samples load, or if they are unavailable: a sine fundamental with a
       quiet octave — deliberately rounder than a triangle lead, which reads as shrill. */
    const g = this.ctx.createGain();
    g.gain.setValueAtTime(0.0001, start);
    g.gain.linearRampToValueAtTime(gain, start + 0.012);
    g.gain.exponentialRampToValueAtTime(gain * 0.6, start + 0.25);
    g.gain.exponentialRampToValueAtTime(0.0001, start + dur);
    const lp = this.ctx.createBiquadFilter();
    lp.type = "lowpass";
    lp.frequency.value = Math.min(5200, f * 8);
    lp.Q.value = 0.5;
    const o1 = this.ctx.createOscillator();
    o1.type = "sine";
    o1.frequency.value = f;
    const o2 = this.ctx.createOscillator();
    o2.type = "triangle";
    o2.frequency.value = f * 2.002;
    const g2 = this.ctx.createGain();
    g2.gain.value = 0.09;
    o1.connect(lp);
    o2.connect(g2).connect(lp);
    lp.connect(g).connect(this.master);
    [o1, o2].forEach((o) => { o.start(start); o.stop(start + dur + 0.03); });
    this.voices.add(o1);
    this.voices.add(o2);
    o1.onended = () => { this.voices.delete(o1); this.voices.delete(o2); };
    return start;
  },

  chord(midis, opts = {}) {
    const list = [...new Set(midis)].sort((x, y) => x - y);
    list.forEach((midi, i) => {
      this.tone(midi, { ...opts, when: (opts.when || 0) + i * (opts.spread ?? 0.012) });
    });
  },

  arp(midis, { step = 0.13, dur = 0.9, gain = 0.08, when = 0 } = {}) {
    midis.forEach((midi, i) => this.tone(midi, { when: when + i * step, dur, gain }));
  },

  click(accent = false, when = 0) {
    this.resume();
    if (!this.ctx || !this.on) return;
    const start = this.ctx.currentTime + when;
    const osc = this.ctx.createOscillator();
    const env = this.ctx.createGain();
    osc.type = "square";
    osc.frequency.value = accent ? 1640 : 1180;
    env.gain.setValueAtTime(accent ? 0.06 : 0.035, start);
    env.gain.exponentialRampToValueAtTime(0.0001, start + 0.05);
    osc.connect(env).connect(this.master);
    osc.start(start);
    osc.stop(start + 0.06);
  },

  stopAll() {
    for (const node of this.voices) {
      try { node.stop(); } catch {}
    }
    for (const node of this.pianoVoices) {
      try { node.stop(); } catch {}
    }
    this.voices.clear();
    this.pianoVoices.clear();
  },
};

/* ---------------------------------------------------------------- staff */

const TREBLE_BASE = 4 * 7 + 2; /* E4 bottom line */
const BASS_BASE = 2 * 7 + 4;   /* G2 bottom line */

export function diatonicOf(note) {
  const pc = pcOf(note.midi);
  const letter = note.letter ?? [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6][pc];
  return (Math.floor(note.midi / 12) - 1) * 7 + letter;
}

export function staffSVG(notes, opts = {}) {
  const width = opts.width || 720;
  const height = opts.height || 172;
  const clef = opts.clef || "treble";
  const chord = Boolean(opts.chord);
  const labels = Boolean(opts.labels);
  const base = clef === "bass" ? BASS_BASE : TREBLE_BASE;
  const topY = 46;
  const bottomY = 94;
  const x0 = 96;
  const x1 = width - 40;
  const list = notes.length ? notes : [{ midi: 60 }];
  const spread = list.length > 1 && !chord;
  const stepX = spread ? (x1 - x0) / (list.length - 1) : (x0 + x1) / 2;
  const out = [];

  for (let i = 0; i < 5; i++) {
    const y = topY + i * 12;
    out.push(`<line class="sline" x1="${clef ? 58 : 24}" y1="${y}" x2="${width - 22}" y2="${y}"/>`);
  }
  if (clef !== "none") {
    out.push(`<text class="clef" x="12" y="94" font-family="Segoe UI Symbol, Apple Symbols, Noto Music, serif">${clef === "bass" ? "𝄢" : "𝄞"}</text>`);
  }

  const ys = list.map((n) => bottomY - (diatonicOf(n) - base) * 6);
  const upward = ys.reduce((a, b) => a + b, 0) / ys.length >= 74;
  list.forEach((note, i) => {
    const x = chord ? (x0 + x1) / 2 : spread ? x0 + i * stepX : stepX;
    const y = bottomY - (diatonicOf(note) - base) * 6;
    const ledgers = [];
    if (y > bottomY) for (let ly = bottomY + 12; ly <= y; ly += 12) ledgers.push(ly);
    if (y < topY) for (let ly = topY - 12; ly >= y; ly -= 12) ledgers.push(ly);
    const ledger = ledgers.map((ly) => `<line class="ledger" x1="${x - 11}" y1="${ly}" x2="${x + 11}" y2="${ly}"/>`).join("");
    const name = note.name || noteName(note.midi);
    const accidental = name.includes("♯") ? "♯".repeat((name.match(/♯/g) || []).length)
      : name.includes("♭") ? "♭".repeat((name.match(/♭/g) || []).length) : "";
    const acc = accidental ? `<text class="acc" x="${x - 24}" y="${y + 4}" text-anchor="middle">${accidental}</text>` : "";
    out.push(
      `<g class="note-hit" data-midi="${note.midi}">${ledger}${acc}` +
      `<ellipse class="nhead" cx="${x}" cy="${y}" rx="7" ry="5" transform="rotate(-18 ${x} ${y})"/>` +
      `<line class="nstem" x1="${x + (upward ? 6.4 : -6.4)}" y1="${y}" x2="${x + (upward ? 6.4 : -6.4)}" y2="${upward ? y - 34 : y + 34}"/></g>`
    );
    if (labels) {
      out.push(`<text class="ntext" x="${x}" y="${height - 16}" text-anchor="middle">${name}${Math.floor(note.midi / 12) - 1}</text>`);
    }
  });
  return `<svg viewBox="0 0 ${width} ${height}" role="img" aria-label="五线谱：${list.map((n) => n.name || noteName(n.midi)).join("、")}">${out.join("")}</svg>`;
}

/* ---------------------------------------------------------------- fretboard */

export function fretSVG(voicing, opts = {}) {
  const frets = (voicing?.frets || [-1, -1, -1, -1, -1, -1]).slice(0, 6);
  const strings = 6;
  const width = opts.width || 168;
  const height = opts.height || 190;
  const left = 26;
  const right = width - 14;
  const top = 30;
  const gap = (height - top - 24) / (strings - 1);
  const positive = frets.filter((f) => f > 0);
  const min = positive.length ? Math.min(...positive) : 1;
  const base = min <= 1 ? 1 : min;
  const windowSize = 5;
  const xs = (f) => left + ((f - base + 0.5) / windowSize) * (right - left);
  const stringY = (index) => top + (strings - 1 - index) * gap; /* index 0 = low E at bottom */
  const out = [];

  for (let f = 0; f <= windowSize; f++) {
    const x = left + (f / windowSize) * (right - left);
    out.push(`<line class="${f === 0 ? "fb-nut" : "fb-fret"}" x1="${x}" y1="${top - 8}" x2="${x}" y2="${height - 22}"/>`);
  }
  for (let s = 0; s < strings; s++) {
    out.push(`<line class="fb-string" x1="${left}" y1="${stringY(s)}" x2="${right}" y2="${stringY(s)}"/>`);
  }
  for (let s = 0; s < strings; s++) {
    const fret = frets[s];
    const y = stringY(s);
    const label = `第 ${strings - s} 弦`;
    if (fret < 0) {
      out.push(`<text class="fb-num" x="${left - 12}" y="${y + 3}" text-anchor="middle">×</text>`);
    } else if (fret === 0) {
      out.push(`<circle class="fb-open" cx="${left - 12}" cy="${y}" r="4.4"><title>${label} 空弦</title></circle>`);
    } else {
      const x = xs(fret);
      out.push(`<circle class="fb-dot" cx="${x}" cy="${y}" r="7"><title>${label} 第 ${fret} 品</title></circle>`);
      out.push(`<text x="${x}" y="${y + 3.4}" text-anchor="middle" font-family="IBM Plex Mono,monospace" font-size="8" fill="var(--bg)">${fret}</text>`);
    }
  }
  out.push(`<text class="fb-num" x="${left}" y="${height - 6}">${base === 1 ? "1" : `${base}`}品</text>`);
  out.push(`<text class="fb-num" x="${right}" y="${height - 6}" text-anchor="end">${base === 1 ? "5" : `${base + windowSize - 1}`}品</text>`);
  return `<svg viewBox="0 0 ${width} ${height}" role="img" aria-label="吉他指法图">${out.join("")}</svg>`;
}

/* ---------------------------------------------------------------- keyboard */

export function renderKeyboard(host, { start = 60, end = 84, active = [], onKey, labels = [60, 72] } = {}) {
  const whites = [];
  const blacks = [];
  for (let midi = start; midi <= end; midi++) {
    ([1, 3, 6, 8, 10].includes(pcOf(midi)) ? blacks : whites).push(midi);
  }
  host.innerHTML = "";
  const box = el("div", { class: "keyboard" });
  const whiteWidth = 100 / whites.length;
  whites.forEach((midi) => {
    const key = el("button", { type: "button", "data-midi": midi, "aria-label": noteName(midi), "aria-pressed": active.includes(midi) },
      el("span", {}, labels.includes(midi) || pcOf(midi) === 0 ? noteName(midi) : ""));
    key.addEventListener("click", () => onKey?.(midi));
    box.append(key);
  });
  blacks.forEach((midi) => {
    const index = whites.filter((w) => w < midi).length;
    const key = el("button", {
      type: "button",
      class: "is-black",
      "data-midi": midi,
      "aria-label": noteName(midi),
      "aria-pressed": active.includes(midi),
      style: `left:${index * whiteWidth}%`,
    });
    key.addEventListener("click", () => onKey?.(midi));
    box.append(key);
  });
  host.append(box);
  return box;
}

/* ---------------------------------------------------------------- chrome */

export function mountChrome() {
  const path = location.pathname.replace(/\/$/, "") || "/";
  $$(".top nav a").forEach((link) => {
    const href = link.getAttribute("href");
    if (href === path || (path.startsWith("/s/") && href === "/studio")) {
      link.setAttribute("aria-current", "page");
    }
  });
  $$("[data-device]").forEach((node) => { node.textContent = deviceId().slice(0, 12); });
  $$("[data-health]").forEach(async (node) => {
    node.dataset.state = "loading";
    try {
      const health = await api("/api/health");
      node.dataset.state = health.status === "ok" ? "ok" : "error";
      node.title = `API ${health.status} · native ${health.native?.ok ? "ok" : "off"} · DB ${health.db?.ok ? "ok" : "down"}`;
    } catch {
      node.dataset.state = "error";
      node.title = "练习服务未连接";
    }
  });
  Sound.setEnabled(localStorage.getItem("temper-sound") !== "0");
}

export function downloadJSON(name, value) {
  const blob = new Blob([JSON.stringify(value, null, 2)], { type: "application/json" });
  const link = el("a", { href: URL.createObjectURL(blob), download: name });
  document.body.append(link);
  link.click();
  setTimeout(() => { URL.revokeObjectURL(link.href); link.remove(); }, 400);
}

export { chordTones, midiToFreq, noteName, pcOf };

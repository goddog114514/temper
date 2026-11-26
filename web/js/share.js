import { $, $$, api, mountChrome, toast, Sound } from "./app.js";
import { ROOT_NAMES, progression, shareCode } from "./theory.js";

mountChrome();

const code = shareCode(location.pathname || "") || new URLSearchParams(location.search).get("c");
const state = { payload: null, playing: false, timer: 0 };

function fail(message) {
  $("#sharePanel").hidden = true;
  $("#shareError").hidden = false;
  $("#shareTitle").textContent = "这条链接不在了";
  $("#shareMeta").textContent = message;
}

async function load() {
  if (!code) return fail("地址里没有分享码。");
  $("#shareCode").textContent = `CODE ${code.toUpperCase()}`;
  try {
    const data = await api(`/api/shares/${encodeURIComponent(code)}`);
    state.payload = data.payload;
    render(data);
  } catch (error) {
    fail(error.message || "无法读取这条分享。");
  }
}

function render(data) {
  const payload = data.payload;
  const chords = progression(payload.keyPc, payload.degrees, {});
  $("#shareTitle").textContent = data.title;
  $("#shareMeta").textContent = `${payload.keyName} 大调 · ${payload.degrees.length} 小节 · ${payload.tempo} BPM · ${payload.degrees.length * 4} 拍循环`;
  $("#shareTempo").textContent = `${payload.tempo} BPM · ${payload.instrument === "guitar" ? "吉他" : "钢琴"}`;
  $("#shareFoot").textContent = `浏览 ${data.views + 1} 次 · 生成于 ${data.createdAt.slice(0, 10)}`;
  $("#sharePanel").hidden = false;
  $("#shareError").hidden = true;

  const slots = document.createElement("div");
  slots.className = "slots";
  slots.style.setProperty("--n", String(chords.length));
  slots.innerHTML = chords
    .map((chord, i) => `<div class="slot" data-i="${i}"><span class="rn">${chord.roman}</span><span class="sym">${chord.symbol}</span><span class="rn">${i + 1}</span></div>`)
    .join("");
  $("#shareSlots").innerHTML = "";
  $("#shareSlots").append(slots);

  $("#shareTable").innerHTML = chords
    .map((chord, i) => `<tr><td>${i + 1}</td><td>${chord.roman}</td><td>${chord.symbol}</td><td>${chord.tones.map((t) => t.name).join(" ")}</td></tr>`)
    .join("");

  const params = new URLSearchParams({
    key: String(payload.keyPc),
    tempo: String(payload.tempo),
    degrees: payload.degrees.join("."),
  });
  $("#shareImport").href = `/studio?${params}`;
}

function stop() {
  cancelAnimationFrame(state.timer);
  state.playing = false;
  Sound.stopAll();
  $$("#shareSlots .slot").forEach((slot) => slot.classList.remove("is-now"));
  $("#sharePlay").textContent = "播放";
  $("#shareStop").disabled = true;
}

function play() {
  if (state.playing) return stop();
  const payload = state.payload;
  if (!payload) return;
  Sound.resume();
  if (!Sound.ctx) return toast("浏览器没有可用的音频上下文", "error");
  const chords = progression(payload.keyPc, payload.degrees, {});
  const beat = 60 / payload.tempo;
  const start = Sound.now() + 0.08;
  state.playing = true;
  $("#sharePlay").textContent = "停止";
  $("#shareStop").disabled = false;
  chords.forEach((chord, i) => {
    Sound.chord(chord.midis, { when: i * 4 * beat, dur: Math.min(3.4 * beat, 2.4), gain: 0.07 });
  });
  const total = chords.length * 4 * beat;
  const tick = () => {
    if (!state.playing) return;
    const elapsed = Sound.now() - start;
    const index = Math.floor(elapsed / (4 * beat));
    $$("#shareSlots .slot").forEach((slot) => slot.classList.toggle("is-now", Number(slot.dataset.i) === index));
    if (elapsed >= total) return stop();
    state.timer = requestAnimationFrame(tick);
  };
  state.timer = requestAnimationFrame(tick);
}

$("#sharePlay").addEventListener("click", play);
$("#shareStop").addEventListener("click", stop);
document.addEventListener("keydown", (event) => {
  if (event.code === "Space" && !event.target.closest("input,select,textarea")) {
    event.preventDefault();
    play();
  }
  if (event.key === "Escape") stop();
});

load();

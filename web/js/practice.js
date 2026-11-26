import { $, $$, api, mountChrome, staffSVG, toast, Sound } from "./app.js";
import { formatClock, notesForSight, rootName } from "./theory.js";

mountChrome();

const MODES = [
  { id: "interval", name: "音程辨认", en: "INTERVAL", hint: "两个音的距离。先听低音，再判断中间隔了多少个半音。", length: 8 },
  { id: "chord", name: "和弦性质", en: "CHORD", hint: "三音还是四音？先听最低音与三音，再看七音。", length: 8 },
  { id: "scale", name: "音阶辨认", en: "SCALE", hint: "先找主音，再听第三、第六、第七级。", length: 8 },
  { id: "sight", name: "读谱", en: "SIGHT", hint: "只看谱，不弹。先认谱号，再数线与间。", length: 10 },
  { id: "rhythm", name: "节奏跟练", en: "RHYTHM", hint: "跟着节拍点击。系统按八分音符网格比较你的拍点。", length: 6 },
];

const state = {
  mode: "interval",
  difficulty: 2,
  length: 8,
  session: null,
  questions: [],
  index: 0,
  answered: 0,
  correct: 0,
  streak: 0,
  locked: false,
  shownAt: 0,
  startedAt: 0,
  current: null,
  rhythm: null,
  timer: 0,
};

function setMode(modeId) {
  state.mode = modeId;
  const meta = MODES.find((m) => m.id === modeId);
  state.length = meta.length;
  $$("#modeCards .mode-card").forEach((card) => card.setAttribute("aria-pressed", String(card.dataset.mode === modeId)));
  $("#modeHint").textContent = meta.hint;
  $("#lengthSel").value = String(state.length);
  $("#liveMode").textContent = meta.en;
}

function buildModes() {
  const host = $("#modeCards");
  host.innerHTML = "";
  MODES.forEach((mode) => {
    const card = document.createElement("button");
    card.type = "button";
    card.className = "mode-card";
    card.dataset.mode = mode.id;
    card.setAttribute("aria-pressed", String(mode.id === state.mode));
    card.innerHTML = `<span>${mode.en}</span><b>${mode.name}</b><span>${mode.length} 题起</span>`;
    card.addEventListener("click", () => setMode(mode.id));
    host.append(card);
  });

  $$("#difficultySeg button").forEach((button) => {
    button.addEventListener("click", () => {
      state.difficulty = Number(button.dataset.diff);
      $$("#difficultySeg button").forEach((b) => b.setAttribute("aria-pressed", String(b === button)));
    });
  });
  $("#lengthSel").addEventListener("change", () => { state.length = Number($("#lengthSel").value); });
  setMode(state.mode);
}

function updateLive() {
  $("#liveCorrect").textContent = String(state.correct);
  $("#liveStreak").textContent = String(state.streak);
  const total = state.session?.total || state.length;
  const done = state.answered;
  $("#liveBar").style.width = `${Math.min(100, (done / total) * 100)}%`;
  $("#liveNote").textContent = state.session ? `已完成 ${done} / ${total}，答对 ${state.correct}。` : "开始后会显示进度。";
}

async function startSession() {
  const button = $("#startBtn");
  button.disabled = true;
  try {
    const data = await api("/api/practice/sessions", {
      method: "POST",
      body: { mode: state.mode, difficulty: state.difficulty, length: state.length },
    });
    state.session = data.session;
    state.questions = data.questions;
    state.index = 0;
    state.answered = 0;
    state.correct = 0;
    state.streak = 0;
    state.locked = false;
    state.startedAt = performance.now();
    $("#qTotal").textContent = String(state.session.total);
    $("#setup").hidden = true;
    $("#summary").hidden = true;
    $("#runner").hidden = false;
    updateLive();
    renderQuestion();
  } catch (error) {
    toast(error.message || "无法开始练习", "error");
  } finally {
    button.disabled = false;
  }
}

function renderQuestion() {
  const q = state.questions[state.index];
  if (!q) return finishSession();
  state.current = q;
  state.locked = false;
  state.shownAt = performance.now();
  state.rhythm = null;
  clearInterval(state.timer);

  $("#qIndex").textContent = String(state.index + 1);
  $("#qKind").textContent = MODES.find((m) => m.id === q.kind)?.en || q.kind.toUpperCase();
  $("#qBar").style.width = `${(state.index / state.questions.length) * 100}%`;
  $("#qHint").textContent = q.prompt.hint || "";
  $("#feedback").hidden = true;
  $("#feedback").innerHTML = "";
  $("#nextBtn").disabled = true;
  $("#nextBtn").textContent = state.index + 1 >= state.questions.length ? "看结果" : "下一题";
  $("#choices").innerHTML = "";
  $("#promptStage").hidden = true;
  $("#promptStage").innerHTML = "";
  $("#rhythmArea").hidden = true;
  $("#playPrompt").hidden = false;
  $("#replay").hidden = true;
  $("#rhythmStatus").textContent = "";
  $("#tapPad").textContent = "点击这里开始 · 跟随节拍敲击";

  if (q.kind === "sight") {
    $("#promptLabel").textContent = q.prompt.clef === "bass" ? "低音谱表 · 这个音是？" : "高音谱表 · 这个音是？";
    $("#promptBig").textContent = "看谱";
    $("#playPrompt").hidden = true;
    const info = notesForSight(q.prompt.midi);
    $("#promptStage").hidden = false;
    $("#promptStage").innerHTML = staffSVG([{ midi: q.prompt.midi, letter: info.letter, name: info.name }], {
      clef: q.prompt.clef,
      width: 620,
      height: 172,
    });
  } else if (q.kind === "rhythm") {
    $("#promptLabel").textContent = `${q.prompt.meter} · ${q.prompt.bpm} BPM`;
    $("#promptBig").textContent = "跟着拍点敲击";
    $("#playPrompt").hidden = true;
    $("#rhythmArea").hidden = false;
    $("#choices").innerHTML = "";
    renderBeatGrid(q);
  } else if (q.kind === "chord") {
    $("#promptLabel").textContent = "听和弦";
    $("#promptBig").textContent = `根音 ${rootName(q.prompt.rootPc)}`;
    $("#promptStage").hidden = false;
    $("#promptStage").innerHTML = staffSVG(
      q.prompt.midis.map((midi, i) => ({ midi, name: null, letter: [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6][((midi % 12) + 12) % 12] })),
      { chord: true, width: 620, height: 172 }
    );
  } else if (q.kind === "scale") {
    $("#promptLabel").textContent = "听音阶";
    $("#promptBig").textContent = `主音 ${rootName(q.prompt.tonicPc)}`;
    $("#promptStage").hidden = false;
    $("#promptStage").innerHTML = staffSVG(
      q.prompt.midis.slice(0, 8).map((midi) => ({ midi })),
      { width: 700, height: 176 }
    );
  } else {
    $("#promptLabel").textContent = q.prompt.melodic ? "旋律音程" : "和声音程";
    $("#promptBig").textContent = "听两个音";
  }

  if (q.kind !== "rhythm") buildChoices(q);
  setTimeout(() => playPrompt(), 180);
}

function buildChoices(q) {
  const host = $("#choices");
  host.innerHTML = "";
  q.prompt.choices.forEach((choice, i) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "choice";
    button.dataset.id = choice.id;
    button.innerHTML = `<kbd>${i + 1}</kbd><span>${choice.label}</span>`;
    button.addEventListener("click", () => submitChoice(choice.id, button));
    host.append(button);
  });
}

function playPrompt() {
  const q = state.current;
  if (!q) return;
  Sound.resume();
  if (q.kind === "interval") {
    if (q.prompt.melodic) Sound.arp([q.prompt.root, q.prompt.top], { step: 0.52, dur: 1.1, gain: 0.085 });
    else Sound.chord([q.prompt.root, q.prompt.top], { dur: 1.9, gain: 0.075, spread: 0.01 });
  } else if (q.kind === "chord") {
    Sound.chord(q.prompt.midis, { dur: 1.9, gain: 0.07 });
  } else if (q.kind === "scale") {
    Sound.arp(q.prompt.midis, { step: 0.16, dur: 0.9, gain: 0.08 });
  } else if (q.kind === "rhythm") {
    // The reference pattern is the exercise itself; the tap pad drives it.
  }
}

function markChoices(chosenId, expectedId) {
  $$("#choices .choice").forEach((button) => {
    button.disabled = true;
    if (button.dataset.id === expectedId) button.dataset.state = "correct";
    else if (button.dataset.id === chosenId) button.dataset.state = "wrong";
  });
}

async function submitChoice(choiceId, button) {
  if (state.locked) return;
  state.locked = true;
  $$("#choices .choice").forEach((node) => { node.disabled = true; });
  const elapsed = Math.round(performance.now() - state.shownAt);
  try {
    const result = await api(`/api/practice/sessions/${state.session.id}/answers`, {
      method: "POST",
      body: { questionId: state.current.id, answer: choiceId, elapsedMs: elapsed },
    });
    settle(result, choiceId);
  } catch (error) {
    state.locked = false;
    toast(error.message || "提交失败", "error");
    $$("#choices .choice").forEach((node) => { node.disabled = false; });
  }
}

function settle(result, chosenId) {
  state.answered = result.session.answered;
  state.correct = result.session.correct;
  state.streak = result.streak;
  const expectedId = result.expected?.id || result.expected?.name;
  if (chosenId != null) markChoices(chosenId, expectedId);

  const feedback = $("#feedback");
  feedback.hidden = false;
  feedback.dataset.state = result.correct ? "ok" : "bad";
  feedback.innerHTML = `
    <b>${result.correct ? "答对了" : "再听一遍"}</b>
    <p>${result.explanation}${result.correct ? "" : " · 正确答案已标出"}</p>`;
  $("#nextBtn").disabled = false;
  $("#replay").hidden = false;
  $("#qBar").style.width = `${(state.answered / state.questions.length) * 100}%`;
  updateLive();
  if (result.correct) Sound.tone(88, { dur: 0.35, gain: 0.05 });
}

function replay() {
  const q = state.current;
  if (!q) return;
  if (q.kind === "rhythm") {
    const expectedMs = q.expected?.expectedMs || [];
    expectedMs.forEach((ms) => Sound.click(true, ms / 1000));
    return;
  }
  playPrompt();
}

function next() {
  if (!state.locked && $("#nextBtn").disabled) return;
  $("#replay").hidden = true;
  if (state.index + 1 >= state.questions.length) {
    finishSession();
    return;
  }
  state.index += 1;
  renderQuestion();
}

async function finishSession() {
  if (!state.session) return;
  try {
    const data = await api(`/api/practice/sessions/${state.session.id}/finish`, { method: "POST" });
    const summary = data.summary;
    $("#runner").hidden = true;
    $("#summary").hidden = false;

    const accuracy = summary.accuracy || 0;
    $("#sumAccuracy").innerHTML = `${accuracy}<small>%</small>`;
    $("#sumRing").style.setProperty("--v", String(accuracy));
    $("#sumCorrect").textContent = `${summary.correct}/${summary.answered}`;
    $("#sumScore").textContent = String(summary.score);
    $("#sumTime").textContent = formatClock(performance.now() - (state.startedAt || performance.now()));
    $("#sumMode").textContent = MODES.find((m) => m.id === state.session.mode)?.en || state.session.mode;

    const tags = summary.byTag || [];
    $("#sumTags").innerHTML = tags.length
      ? tags.map((tag) => `<div class="rowitem"><div class="rowitem__main"><b>${tagLabel(tag.tag)}</b><span>${tag.attempts} 题</span></div><div class="rowitem__side">${tag.accuracy}%</div></div>`).join("")
      : '<p class="small muted">这一组没有可汇总的薄弱项。</p>';
    updateLive();
  } catch (error) {
    toast(error.message || "无法结束练习", "error");
  }
}

function tagLabel(tag) {
  const [kind, value] = String(tag).split(":");
  const prefix = MODES.find((m) => m.id === kind)?.name || kind;
  return value ? `${prefix} · ${value}` : prefix;
}

/* ---------------------------------------------------------------- rhythm */

function renderBeatGrid(q) {
  const host = $("#beatGrid");
  host.innerHTML = (q.prompt.pattern || []).map((on, i) => `<i data-i="${i}" class="${on ? "on" : ""}"></i>`).join("");
}

function startRhythm() {
  const q = state.current;
  if (!q || state.locked) return;
  const bpm = q.prompt.bpm;
  const beatMs = 60_000 / bpm;
  const eighthMs = beatMs / 2;
  const countIn = 4 * beatMs;
  const grid = q.prompt.grid;
  const startAt = Sound.now() + 0.15;

  Sound.resume();
  for (let b = 0; b < 4; b++) Sound.click(b === 0, b * (beatMs / 1000));
  for (let i = 0; i < grid; i++) {
    Sound.click(q.prompt.pattern[i] === 1, (countIn + i * eighthMs) / 1000);
  }

  const tapOrigin = performance.now() + 150 + countIn;
  const taps = [];
  const totalMs = countIn + grid * eighthMs + beatMs;
  let active = true;
  let countdown = 4;

  $("#rhythmStart").disabled = true;
  $("#tapPad").textContent = "准备… 4";
  state.rhythm = { taps, tapOrigin, active: true };

  const tap = () => {
    if (!active) return;
    const offset = performance.now() - tapOrigin;
    if (offset >= -120) taps.push(Math.round(offset));
    $("#tapPad").classList.add("is-hit");
    setTimeout(() => $("#tapPad").classList.remove("is-hit"), 90);
  };
  const onKey = (event) => {
    if (event.code === "Space") {
      event.preventDefault();
      tap();
    }
  };
  $("#tapPad").addEventListener("pointerdown", tap);
  document.addEventListener("keydown", onKey);

  const startedAt = performance.now();
  state.timer = setInterval(() => {
    const elapsed = performance.now() - startedAt;
    const inCountIn = elapsed < countIn;
    const remain = Math.max(0, Math.ceil((countIn - elapsed) / beatMs));
    if (inCountIn) {
      $("#tapPad").textContent = `准备… ${remain}`;
      $("#rhythmStatus").textContent = `数入 ${remain} 拍`;
    } else {
      $("#tapPad").textContent = "敲击";
      const slot = Math.min(grid - 1, Math.floor((elapsed - countIn) / eighthMs));
      $$("#beatGrid i").forEach((node, i) => node.classList.toggle("now", i === slot));
      $("#rhythmStatus").textContent = `第 ${slot + 1} / ${grid} 格`;
    }
    if (elapsed >= totalMs) {
      active = false;
      clearInterval(state.timer);
      $("#tapPad").removeEventListener("pointerdown", tap);
      document.removeEventListener("keydown", onKey);
      $("#rhythmStart").disabled = false;
      $("#rhythmStatus").textContent = `收到 ${taps.length} 个拍点，正在判分…`;
      submitRhythm(taps).finally(() => {
        state.rhythm = null;
      });
    }
  }, 40);
}

async function submitRhythm(taps) {
  if (state.locked) return;
  state.locked = true;
  try {
    const result = await api(`/api/practice/sessions/${state.session.id}/answers`, {
      method: "POST",
      body: { questionId: state.current.id, tapsMs: taps, elapsedMs: Math.round(performance.now() - state.shownAt) },
    });
    settle(result, null);
  } catch (error) {
    state.locked = false;
    toast(error.message || "提交失败", "error");
  }
}

/* ---------------------------------------------------------------- boot */

buildModes();
$("#startBtn").addEventListener("click", startSession);
$("#nextBtn").addEventListener("click", next);
$("#replay").addEventListener("click", replay);
$("#playPrompt").addEventListener("click", playPrompt);
$("#rhythmStart").addEventListener("click", startRhythm);
$("#finishEarly").addEventListener("click", finishSession);
$("#againBtn").addEventListener("click", () => {
  $("#summary").hidden = true;
  $("#setup").hidden = false;
  $("#runner").hidden = true;
  $("#setup").scrollIntoView({ behavior: "smooth", block: "start" });
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") return;
  if (/^[1-9]$/.test(event.key) && !$("#runner").hidden && !$("#choices").hidden) {
    const index = Number(event.key) - 1;
    const button = $$("#choices .choice")[index];
    if (button && !button.disabled) button.click();
  }
  if (event.key === "Enter" && !$("#nextBtn").disabled && !$("#runner").hidden) {
    event.preventDefault();
    next();
  }
});

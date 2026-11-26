import { $, $$, api, deviceId, downloadJSON, mountChrome, toast } from "./app.js";
import { ROOT_NAMES } from "./theory.js";

mountChrome();

const CHECKLIST = [
  { id: "c4-a4", label: "在琴键或指板上找到 C4 与 A4" },
  { id: "fifth-tritone", label: "听出纯五度与三全音的区别" },
  { id: "sing-scale", label: "唱一遍 C 大调音阶" },
  { id: "key-signatures", label: "说出 C、G、D 三个调的调号" },
  { id: "cfgc", label: "弹 C–F–G–C 四个和弦" },
  { id: "count-4-4", label: "用 4/4 拍数完八小节" },
  { id: "staff-first-line", label: "不看琴键说出五线谱第一线的音" },
  { id: "play-widget", label: "把本页任意一个组件玩三遍" },
];

const MODE_LABEL = {
  interval: "音程辨认",
  chord: "和弦性质",
  scale: "音阶辨认",
  sight: "读谱",
  rhythm: "节奏跟练",
};

let profile = null;

function fmtDate(iso) {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function render(profileData) {
  profile = profileData;
  const stats = profileData.stats;
  $("#statSessions").textContent = String(stats.sessions);
  $("#statAccuracy").innerHTML = `${stats.accuracy}<small>%</small>`;
  $("#statStreak").innerHTML = `${stats.streakDays}<small>天</small>`;
  $("#statMinutes").innerHTML = `${stats.minutes}<small>分钟</small>`;
  $("#deviceLabel").textContent = `DEVICE ${profileData.user.id.slice(0, 10)}`;

  $("#modeRows").innerHTML = stats.byMode.length
    ? stats.byMode
        .map(
          (mode) => `<div class="rowitem">
            <div class="rowitem__main"><b>${MODE_LABEL[mode.mode] || mode.mode}</b><span>${mode.sessions} 组 · ${mode.correct}/${mode.answered}</span></div>
            <div class="rowitem__side">${mode.accuracy}%</div>
          </div>
          <div class="bar bar--green" style="margin:0 0 10px"><i style="width:${Math.min(100, mode.accuracy)}%"></i></div>`
        )
        .join("")
    : '<p class="small muted">还没有完成的练习组。</p>';

  $("#weakRows").innerHTML = profileData.weakTags.length
    ? profileData.weakTags
        .map((tag) => {
          const [kind, value] = String(tag.tag).split(":");
          return `<div class="rowitem">
            <div class="rowitem__main"><b>${MODE_LABEL[kind] || kind}${value ? ` · ${value}` : ""}</b><span>${tag.correct}/${tag.attempts} 次答对</span></div>
            <div class="rowitem__side"><span class="tag ${tag.accuracy < 60 ? "tag--bad" : "tag--ok"}">${tag.accuracy}%</span></div>
          </div>`;
        })
        .join("")
    : '<p class="small muted">再练几组，这里会指出最容易错的音程与和弦。</p>';

  $("#recentRows").innerHTML = profileData.recent.length
    ? profileData.recent
        .map(
          (row) => `<tr>
            <td>${fmtDate(row.startedAt)}</td>
            <td>${MODE_LABEL[row.mode] || row.mode}</td>
            <td>${row.difficulty}</td>
            <td>${row.correct}/${row.answered}</td>
            <td>${row.answered ? Math.round((row.correct / row.answered) * 1000) / 10 : 0}%</td>
            <td>${row.score}</td>
          </tr>`
        )
        .join("")
    : '<tr><td colspan="6" class="muted">还没有记录</td></tr>';

  renderProgress(profileData.progress || []);
}

function renderProgress(items) {
  const done = new Map(items.map((item) => [item.id, item.done]));
  const known = new Set(CHECKLIST.map((item) => item.id));
  const extra = items.filter((item) => !known.has(item.id));
  const all = [...CHECKLIST, ...extra.map((item) => ({ id: item.id, label: item.id }))];

  $("#progressRows").innerHTML = all
    .map(
      (item) => `<label class="rowitem" style="grid-template-columns:auto 1fr;align-items:center;cursor:pointer">
        <input type="checkbox" data-id="${item.id}" ${done.get(item.id) ? "checked" : ""} style="accent-color:var(--accent);width:16px;height:16px">
        <span class="rowitem__main"><b style="font-weight:400">${item.label}</b></span>
      </label>`
    )
    .join("");

  const count = $$("#progressRows input:checked").length;
  $("#checkCount").textContent = `${count} / ${all.length}`;
  $("#progressBar").style.width = `${(count / Math.max(1, all.length)) * 100}%`;

  $$("#progressRows input").forEach((box) => {
    box.addEventListener("change", async () => {
      const payload = $$("#progressRows input").map((input) => ({ id: input.dataset.id, done: input.checked }));
      const checked = payload.filter((item) => item.done).length;
      $("#checkCount").textContent = `${checked} / ${payload.length}`;
      $("#progressBar").style.width = `${(checked / Math.max(1, payload.length)) * 100}%`;
      try {
        await api("/api/progress", { method: "PUT", body: { items: payload } });
      } catch (error) {
        toast(error.message, "error");
      }
    });
  });
}

async function loadPresets() {
  try {
    const data = await api("/api/presets");
    $("#presetCount").textContent = String(data.custom.length);
    $("#presetRows").innerHTML = data.custom.length
      ? data.custom
          .map(
            (preset) => `<div class="rowitem">
              <div class="rowitem__main"><b>${escapeHtml(preset.name)}</b><span>${ROOT_NAMES[preset.keyPc]} 大调 · ${preset.tempo} BPM · ${preset.degrees.length} 小节</span></div>
              <div class="rowitem__side"><a class="linkish" href="/studio?key=${preset.keyPc}&tempo=${preset.tempo}&degrees=${preset.degrees.join(".")}">打开</a></div>
            </div>`
          )
          .join("")
      : '<p class="small muted">去工坊保存第一条进行。</p>';
  } catch {
    $("#presetRows").innerHTML = '<p class="small muted">无法读取保存的进行。</p>';
  }
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[char]));
}

async function load() {
  try {
    const data = await api("/api/profile");
    render(data);
    await loadPresets();
  } catch (error) {
    toast(error.message || "无法读取档案", "error");
  }
}

$("#refreshBtn").addEventListener("click", load);
$("#exportBtn").addEventListener("click", async () => {
  try {
    const [profileData, presets] = await Promise.all([api("/api/profile"), api("/api/presets")]);
    downloadJSON(`temper-profile-${deviceId().slice(0, 8)}.json`, { exportedAt: new Date().toISOString(), profile: profileData, presets });
    toast("已导出 JSON");
  } catch (error) {
    toast(error.message, "error");
  }
});
$("#resetBtn").addEventListener("click", () => {
  if (!confirm("生成新的设备标识？旧记录不会被删除，但当前浏览器不再关联。")) return;
  localStorage.removeItem("temper-device");
  localStorage.removeItem("temper-instrument");
  localStorage.removeItem("temper-sound");
  location.reload();
});

load();

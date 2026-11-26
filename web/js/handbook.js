/* Handbook ↔ server bridge.
   The handbook still works when opened as a file; every call is best-effort. */
import { api, deviceId } from "./app.js";

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => Array.from(document.querySelectorAll(selector));

function setStatus(text, state) {
  const dot = $("#cloudDot");
  const label = $("#cloudStatus");
  if (dot) dot.dataset.state = state;
  if (label) label.textContent = text;
}

async function health() {
  try {
    const data = await api("/api/health");
    const native = data.native?.ok ? "C++ 引擎就绪" : "C++ 引擎未就绪";
    setStatus(`练习服务在线 · ${native} · SQLite ${data.db?.ok ? "正常" : "异常"}`, data.status === "ok" ? "ok" : "error");
  } catch {
    setStatus("练习服务未连接：手册仍可离线使用，练习与档案需要启动后端", "error");
  }
}

async function syncProgress() {
  const boxes = $$(".checklist input[type=checkbox]");
  if (!boxes.length) return;
  try {
    const { items } = await api("/api/progress");
    const done = new Map(items.map((item) => [item.id, item.done]));
    boxes.forEach((box) => {
      if (done.has(box.dataset.item)) box.checked = done.get(box.dataset.item);
    });
    boxes[0]?.dispatchEvent(new Event("change"));
  } catch {
    return;
  }
  let timer = 0;
  const push = () => {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      const payload = boxes.map((box) => ({ id: box.dataset.item, done: box.checked }));
      try {
        await api("/api/progress", { method: "PUT", body: { items: payload } });
      } catch {
        /* offline is fine */
      }
    }, 350);
  };
  boxes.forEach((box) => box.addEventListener("change", push));
}

async function syncState() {
  const read = () => ({
    instrument: localStorage.getItem("temper-instrument") === "guitar" ? "guitar" : "piano",
    sound: localStorage.getItem("temper-sound") !== "0",
  });
  try {
    const server = await api("/api/state");
    const local = read();
    // First visit: adopt server preference. Later visits: local wins and is pushed up.
    if (!localStorage.getItem("temper-instrument") && server.instrument) {
      localStorage.setItem("temper-instrument", server.instrument);
    }
    if (!localStorage.getItem("temper-sound") && server.sound === false) {
      localStorage.setItem("temper-sound", "0");
    }
  } catch {
    return;
  }
  let timer = 0;
  const push = () => {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        await api("/api/state", { method: "PUT", body: read() });
      } catch {}
    }, 500);
  };
  $$("button[data-inst]").forEach((button) => button.addEventListener("click", push));
  $$("button[data-sound]").forEach((button) => button.addEventListener("click", push));
}

function annotate() {
  const short = $("#deviceShort");
  if (short) short.textContent = `DEVICE ${deviceId().slice(0, 10)}`;
  $$('#cloud a[href^="/practice"], #cloud a[href^="/studio"], #cloud a[href^="/profile"]').forEach((link) => {
    link.addEventListener("click", (event) => {
      // Keep the handbook state when moving to the studio.
      if (link.getAttribute("href") === "/studio") {
        const key = localStorage.getItem("temper-key") || "0";
        link.href = `/studio?key=${key}`;
      }
      if (event.metaKey || event.ctrlKey) return;
    });
  });
}

annotate();
health();
syncProgress();
syncState();
setTimeout(() => {
  if ($("#cloudStatus")?.textContent.includes("正在连接")) {
    setStatus("练习服务未连接：手册仍可离线使用", "error");
  }
}, 6000);

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") health();
});

#!/usr/bin/env node
/* Layout + runtime audit across pages and viewports. Requires a running server
   and Edge/Chrome; prints a table and writes screenshots to .build/audit/. */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { BUILD_ROOT } from "./env.mjs";

const BASE = process.env.TEMPER_BASE || "http://127.0.0.1:8795";
const OUT = join(BUILD_ROOT, "audit");
const PORT = 9226;
mkdirSync(OUT, { recursive: true });

const EDGE = [
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
].find((path) => existsSync(path));
if (!EDGE) {
  console.error("没有找到 Edge/Chrome");
  process.exit(1);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const browser = spawn(EDGE, [
  "--headless=new", "--disable-gpu", "--no-proxy-server", "--no-first-run",
  `--remote-debugging-port=${PORT}`, `--user-data-dir=${join(OUT, "profile")}`,
  "--window-size=1440,1000", "about:blank",
], { stdio: "ignore" });

class CDP {
  constructor(url) {
    this.ws = new WebSocket(url);
    this.id = 0;
    this.pending = new Map();
    this.listeners = [];
    this.ws.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result);
      } else if (message.method) {
        this.listeners.forEach((fn) => fn(message));
      }
    });
  }
  open() {
    return new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }
  on(fn) { this.listeners.push(fn); }
  send(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++this.id;
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
}

async function devtoolsUrl() {
  for (let i = 0; i < 60; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const page = list.find((target) => target.type === "page");
      if (page) return page.webSocketDebuggerUrl;
    } catch {}
    await sleep(250);
  }
  throw new Error("DevTools 没有就绪");
}

const rows = [];
const failures = [];

async function main() {
  const cdp = new CDP(await devtoolsUrl());
  await cdp.open();
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");

  let exceptions = [];
  cdp.on((message) => {
    if (message.method === "Runtime.exceptionThrown") {
      exceptions.push(message.params.exceptionDetails?.text || "exception");
    }
  });

  const evaluate = async (expression) => {
    const result = await cdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
    return result.result.value;
  };
  const goto = async (path) => {
    exceptions = [];
    await cdp.send("Page.navigate", { url: `${BASE}${path}` });
    await sleep(2400);
  };
  const viewport = (width, height, mobile) => cdp.send("Emulation.setDeviceMetricsOverride", {
    width, height, deviceScaleFactor: mobile ? 2 : 1, mobile,
  });
  const shot = async (name) => {
    const { data } = await cdp.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true });
    writeFileSync(join(OUT, `${name}.png`), Buffer.from(data, "base64"));
  };
  const audit = async (label, path, width, height, mobile) => {
    await viewport(width, height, mobile);
    await goto(path);
    const metrics = await evaluate(`(() => {
      const de = document.documentElement;
      const body = document.body;
      return {
        overflowX: Math.max(de.scrollWidth, body.scrollWidth) - window.innerWidth,
        scrollHeight: de.scrollHeight,
        innerWidth: window.innerWidth,
        title: document.title,
        h1: document.querySelector('h1')?.textContent.trim().slice(0, 40) || '',
        broken: [...document.images].filter(i => !i.complete || i.naturalWidth === 0).length
      };
    })()`);
    rows.push({ label, width, overflowX: metrics.overflowX, height: metrics.scrollHeight, exceptions: exceptions.length, title: metrics.title });
    if (metrics.overflowX > 1) failures.push(`${label}: 横向溢出 ${metrics.overflowX}px`);
    if (exceptions.length) failures.push(`${label}: JS 异常 ${exceptions.join(" | ")}`);
    if (metrics.overflowX <= 1 && width === 1440 && ["studio", "practice", "profile", "handbook"].includes(label)) {
      await shot(`${label}-1440`);
    }
    return metrics;
  };

  await audit("handbook", "/", 1440, 1000, false);
  await audit("handbook", "/", 390, 844, true);
  await audit("studio", "/studio", 1440, 1000, false);
  await audit("studio", "/studio", 390, 844, true);
  await audit("practice", "/practice", 1440, 1000, false);
  await audit("practice", "/practice", 390, 844, true);
  await audit("profile", "/profile", 1440, 1000, false);
  await audit("profile", "/profile", 390, 844, true);

  // Handbook: switch to guitar, ensure lazy samples load and nothing throws.
  await viewport(1440, 1000, false);
  await goto("/");
  exceptions = [];
  await evaluate("document.querySelector('button[data-inst=\"guitar\"]').click()");
  await sleep(2200);
  const guitar = await evaluate(`(() => ({
    mode: document.documentElement.dataset.inst,
    status: document.querySelector('#sampleState')?.textContent || '',
    pressed: document.querySelector('button[data-inst="guitar"]')?.getAttribute('aria-pressed')
  }))()`);
  rows.push({ label: "handbook guitar", width: 1440, overflowX: "-", height: "-", exceptions: exceptions.length, title: guitar.status });
  if (guitar.mode !== "guitar") failures.push("手册：未能切到吉他");
  if (exceptions.length) failures.push(`手册吉他：JS 异常 ${exceptions.join(" | ")}`);
  await shot("handbook-guitar");

  // Practice: sight-reading should draw a staff; rhythm should show a tap pad.
  await viewport(1440, 1000, false);
  await goto("/practice");
  await evaluate("document.querySelector('[data-mode=\"sight\"]').click()");
  await evaluate("document.querySelector('#startBtn').click()");
  await sleep(1600);
  const sight = await evaluate(`(() => ({
    staff: Boolean(document.querySelector('#promptStage svg')),
    choices: document.querySelectorAll('#choices .choice').length,
    prompt: document.querySelector('#promptBig').textContent
  }))()`);
  if (!sight.staff || sight.choices < 3) failures.push("练习：读谱题没有谱表或选项");
  await shot("practice-sight");

  await goto("/practice");
  await evaluate("document.querySelector('[data-mode=\"rhythm\"]').click()");
  await evaluate("document.querySelector('#startBtn').click()");
  await sleep(1500);
  const rhythm = await evaluate(`(() => ({
    visible: !document.querySelector('#rhythmArea').hidden,
    pad: document.querySelector('#tapPad').textContent.trim(),
    beats: document.querySelectorAll('#beatGrid i').length
  }))()`);
  if (!rhythm.visible || rhythm.beats < 4) failures.push("练习：节奏题没有节拍垫或网格");
  await evaluate("document.querySelector('#rhythmStart').click()");
  await sleep(700);
  const tapping = await evaluate("document.querySelector('#tapPad').textContent.trim()");
  rows.push({ label: "rhythm tap", width: 1440, overflowX: "-", height: "-", exceptions: 0, title: `${rhythm.beats} grid · ${tapping}` });
  await shot("practice-rhythm");

  // Studio: change quality, verify the API response reaches the DOM.
  await goto("/studio");
  await evaluate(`(() => {
    const select = document.querySelector('#qualitySel');
    select.value = 'm7b5';
    select.dispatchEvent(new Event('change'));
  })()`);
  await sleep(1400);
  const chord = await evaluate(`(() => ({
    symbol: document.querySelector('#chordSymbol').textContent,
    voicings: document.querySelectorAll('#voicingGrid .voicing').length,
    engine: document.querySelector('#chordEngine').textContent
  }))()`);
  if (!/m7♭5/.test(chord.symbol)) failures.push(`工坊：切到半减七后符号异常（${chord.symbol}）`);
  rows.push({ label: "studio m7b5", width: 1440, overflowX: "-", height: "-", exceptions: 0, title: `${chord.symbol} · ${chord.voicings} shapes · ${chord.engine}` });
  await shot("studio-m7b5");

  console.log("label".padEnd(18), "width".padStart(6), "overflow".padStart(9), "height".padStart(7), "errors".padStart(7), "title");
  for (const row of rows) {
    console.log(String(row.label).padEnd(18), String(row.width).padStart(6), String(row.overflowX).padStart(9), String(row.height).padStart(7), String(row.exceptions).padStart(7), row.title.slice(0, 46));
  }
  if (failures.length) {
    console.error(`\n✗ ${failures.length} 个问题`);
    failures.forEach((failure) => console.error("  - " + failure));
    browser.kill();
    process.exit(1);
  }
  console.log(`\n✓ 通过 · 截图 ${OUT}`);
  browser.kill();
}

main().catch((error) => {
  console.error(error);
  browser.kill();
  process.exit(1);
});

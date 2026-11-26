#!/usr/bin/env node
/* Visual/interaction check through the DevTools protocol.
   Boots Edge headless, drives the practice runner and the studio, and writes
   screenshots to .build/visual/. Not part of `npm run smoke` (needs a running
   server and a Chromium-based browser). */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { BUILD_ROOT } from "./env.mjs";

const BASE = process.env.TEMPER_BASE || "http://127.0.0.1:8795";
const OUT = join(BUILD_ROOT, "visual");
const PORT = 9223;
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
  "--headless=new",
  "--disable-gpu",
  "--no-proxy-server",
  "--no-first-run",
  `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${join(OUT, "profile")}`,
  "--window-size=1440,1000",
  "about:blank",
], { stdio: "ignore" });

class CDP {
  constructor(url) {
    this.ws = new WebSocket(url);
    this.id = 0;
    this.pending = new Map();
    this.ws.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result);
      }
    });
  }
  open() {
    return new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }
  send(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++this.id;
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
}

async function waitForDevtools() {
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

async function main() {
  const wsUrl = await waitForDevtools();
  const cdp = new CDP(wsUrl);
  await cdp.open();
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });

  const evaluate = async (expression) => {
    const result = await cdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
    return result.result.value;
  };
  const goto = async (path) => {
    await cdp.send("Page.navigate", { url: `${BASE}${path}` });
    await sleep(2200);
  };
  const shot = async (name) => {
    const { data } = await cdp.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true });
    writeFileSync(join(OUT, `${name}.png`), Buffer.from(data, "base64"));
  };

  await goto("/practice");
  const modes = await evaluate("document.querySelectorAll('#modeCards .mode-card').length");
  console.log("practice modes:", modes);
  await evaluate("document.querySelector('#startBtn').click()");
  await sleep(1800);
  const running = await evaluate(`(() => {
    const runner = document.querySelector('#runner');
    return {
      visible: !runner.hidden,
      choices: document.querySelectorAll('#choices .choice').length,
      prompt: document.querySelector('#promptBig').textContent,
      index: document.querySelector('#qIndex').textContent
    };
  })()`);
  console.log("runner:", running);
  await shot("practice-runner");

  await evaluate("document.querySelector('#choices .choice').click()");
  await sleep(900);
  const feedback = await evaluate(`(() => {
    const node = document.querySelector('#feedback');
    return { visible: !node.hidden, state: node.dataset.state, text: node.textContent.trim().slice(0, 80) };
  })()`);
  console.log("feedback:", feedback);
  await shot("practice-feedback");

  await goto("/studio");
  const studio = await evaluate(`(() => ({
    symbol: document.querySelector('#chordSymbol').textContent,
    meta: document.querySelector('#chordMeta').textContent,
    voicings: document.querySelectorAll('#voicingGrid .voicing').length,
    engine: document.querySelector('#chordEngine').textContent
  }))()`);
  console.log("studio:", studio);
  await shot("studio-desktop");

  await evaluate("document.querySelector('#voicingGrid .voicing:nth-child(2)')?.click()");
  await sleep(400);
  await shot("studio-voicing");

  await evaluate("document.querySelector('#progPresets .chip')?.click()");
  await sleep(500);
  const progression = await evaluate("document.querySelectorAll('#progSlots .slot').length");
  console.log("progression slots:", progression);
  await shot("studio-progression");

  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
  await goto("/studio");
  await sleep(800);
  await shot("studio-mobile");

  browser.kill();
  console.log(`\n✓ 截图写入 ${OUT}`);
}

main().catch((error) => {
  console.error(error);
  browser.kill();
  process.exit(1);
});

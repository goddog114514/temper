#!/usr/bin/env node
/* End-to-end smoke test: boot the real binary, hit the API, render pages in
   headless Edge, and fail loudly if anything is missing. */
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { BUILD_ROOT, DIST_DIR, ROOT, WEB_DIR, binaryPath, toolchainEnv } from "./env.mjs";

const env = toolchainEnv();
const PORT = Number(process.env.TEMPER_SMOKE_PORT || 8791);
const BASE = `http://127.0.0.1:${PORT}`;
const WORK = join(BUILD_ROOT, "smoke");
const DB = join(WORK, "temper-smoke.db");
const DEVICE = "smoke-device-001";
const STATIC = existsSync(join(DIST_DIR, "index.html")) ? DIST_DIR : WEB_DIR;

rmSync(WORK, { recursive: true, force: true });
mkdirSync(WORK, { recursive: true });

const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok, detail });
  console.log(`${ok ? "✓" : "✗"} ${name}${detail ? `  ${detail}` : ""}`);
}

function buildIfNeeded() {
  if (existsSync(binaryPath())) return;
  console.log("→ 未找到二进制，先构建");
  const build = spawnSync("cargo", ["build", "--release", "-p", "temper-server"], {
    stdio: "inherit",
    env,
    cwd: ROOT,
  });
  if (build.status !== 0) {
    console.error("构建失败");
    process.exit(1);
  }
}

async function request(path, { method = "GET", body, device = DEVICE } = {}) {
  const headers = {};
  if (device) headers["x-temper-device"] = device;
  if (body) headers["content-type"] = "application/json";
  const response = await fetch(`${BASE}${path}`, { method, headers, body: body ? JSON.stringify(body) : undefined });
  const text = await response.text();
  let data = null;
  try { data = JSON.parse(text); } catch { data = text; }
  return { status: response.status, data };
}

async function waitForServer(proc) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    if (proc.exitCode != null) throw new Error(`服务提前退出（code ${proc.exitCode}）`);
    try {
      const response = await fetch(`${BASE}/api/health`, { signal: AbortSignal.timeout(900) });
      if (response.ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("服务在 15s 内没有就绪");
}

function findEdge() {
  const candidates = [
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
  ];
  return candidates.find((path) => existsSync(path)) || "";
}

function dumpDOM(url, tag) {
  const edge = findEdge();
  if (!edge) return { ok: true, detail: "跳过：未安装 Edge/Chrome", skipped: true };
  const profile = join(WORK, `edge-${tag}`);
  const args = [
    "--headless=new",
    "--disable-gpu",
    "--no-first-run",
    "--no-default-browser-check",
    "--no-proxy-server",
    "--disable-extensions",
    `--user-data-dir=${profile}`,
    "--virtual-time-budget=6000",
    "--dump-dom",
    url,
  ];
  const result = spawnSync(edge, args, { encoding: "utf8", timeout: 40_000, maxBuffer: 32 * 1024 * 1024 });
  const html = result.stdout || "";
  return { ok: html.length > 1000, html, stderr: result.stderr || "" };
}

async function main() {
  buildIfNeeded();
  const logPath = join(WORK, "server.log");
  const log = [];
  const proc = spawn(binaryPath(), [], {
    env: { ...env, TEMPER_ADDR: `127.0.0.1:${PORT}`, TEMPER_DB: DB, TEMPER_STATIC: STATIC, TEMPER_LOG: "temper_server=warn" },
    cwd: ROOT,
    windowsHide: true,
  });
  proc.stdout.on("data", (chunk) => log.push(chunk.toString()));
  proc.stderr.on("data", (chunk) => log.push(chunk.toString()));
  const shutdown = () => {
    try { proc.kill(); } catch {}
    writeFileSync(logPath, log.join(""));
  };
  process.on("exit", shutdown);

  try {
    await waitForServer(proc);
    check("server boots", true, `${BASE} · ${STATIC === DIST_DIR ? "dist" : "web"}`);

    const health = await request("/api/health");
    check("health is ok", health.status === 200 && health.data.status === "ok", JSON.stringify(health.data?.native));
    check("native core reports ok", health.data?.native?.ok === true);

    const catalog = await request("/api/theory/catalog");
    check("catalog", catalog.status === 200 && catalog.data.chords.length === 11 && catalog.data.presets.length === 7);

    const voicings = await request("/api/theory/voicings?root=0&quality=maj7&limit=6");
    check("C++ voicings", voicings.status === 200 && voicings.data.voicings.length > 0, `${voicings.data.voicings?.length || 0} shapes · ${voicings.data.tookUs}µs`);

    const analyze = await request("/api/theory/analyze", { method: "POST", body: { notes: [60, 64, 67, 71] } });
    check("chord analysis", analyze.status === 200 && analyze.data.matched[0]?.symbol === "Cmaj7", analyze.data.matched?.[0]?.symbol);

    const session = await request("/api/practice/sessions", { method: "POST", body: { mode: "interval", difficulty: 2, length: 4 } });
    check("session starts", session.status === 200 && session.data.questions.length === 4);
    let last = null;
    for (const question of session.data.questions) {
      const answer = await request(`/api/practice/sessions/${session.data.session.id}/answers`, {
        method: "POST",
        body: { questionId: question.id, answer: question.prompt.choices[0].id, elapsedMs: 1200 },
      });
      last = answer;
    }
    check("answers are graded", last?.status === 200 && typeof last.data.correct === "boolean", last?.data?.explanation);
    const finish = await request(`/api/practice/sessions/${session.data.session.id}/finish`, { method: "POST" });
    check("session finishes", finish.status === 200 && finish.data.summary.answered === 4, `${finish.data.summary?.accuracy}%`);

    const share = await request("/api/shares", {
      method: "POST",
      body: { keyPc: 2, tempo: 104, degrees: [0, 4, 5, 2, 3, 0, 3, 4], presetId: "canon" },
    });
    check("share created", share.status === 200 && share.data.code.length >= 5, share.data.code);

    const profile = await request("/api/profile");
    check("profile aggregates", profile.status === 200 && profile.data.stats.sessions === 1);

    for (const [path, marker] of [
      ["/", "把声音"],
      ["/studio", "和弦"],
      ["/practice", "选一种练法"],
      ["/profile", "练过的"],
      [`/s/${share.data.code}`, "和弦进行"],
    ]) {
      const page = await fetch(`${BASE}${path}`);
      const html = await page.text();
      check(`page ${path}`, page.status === 200 && html.includes(marker) && html.includes("/css/") === (path !== "/"));
    }

    for (const [path, marker] of [
      ["/", "练习服务在线"],
      ["/studio", "C++ ·"],
      ["/practice", "音程辨认"],
      ["/profile", "还没有完成"],
      [`/s/${share.data.code}`, "浏览"],
    ]) {
      const dump = dumpDOM(`${BASE}${path}`, path.replace(/\W+/g, "_"));
      if (dump.skipped) {
        check(`render ${path}`, true, dump.detail);
        continue;
      }
      const ok = dump.ok && dump.html.includes(marker) && !dump.html.includes("页面文件缺失");
      check(`render ${path}`, ok, ok ? marker : (dump.stderr || "DOM 中没有预期内容").slice(0, 160));
    }
  } finally {
    shutdown();
  }

  const failed = results.filter((result) => !result.ok);
  console.log(`\n${failed.length ? "✗" : "✓"} ${results.length - failed.length}/${results.length} 项通过 · 日志 ${logPath}`);
  if (failed.length) {
    console.error(failed.map((f) => `  - ${f.name}: ${f.detail}`).join("\n"));
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});

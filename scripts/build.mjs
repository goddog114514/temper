#!/usr/bin/env node
/* Build: Rust backend + static frontend, with an asset integrity check. */
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { DIST_DIR, ROOT, WEB_DIR, binaryPath, toolchainEnv } from "./env.mjs";

const env = toolchainEnv();
const t0 = Date.now();

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", env, cwd: ROOT, ...options });
  if (result.status !== 0) {
    console.error(`\n✗ ${command} ${args.join(" ")} 失败（exit ${result.status}）`);
    process.exit(result.status || 1);
  }
}

console.log("→ cargo build --release");
run("cargo", ["build", "--release", "-p", "temper-server"]);

if (!existsSync(binaryPath())) {
  console.error(`✗ 未找到可执行文件：${binaryPath()}`);
  process.exit(1);
}

console.log("→ 校验静态资源引用");
const htmlFiles = readdirSync(WEB_DIR).filter((name) => name.endsWith(".html"));
const missing = [];
for (const file of htmlFiles) {
  const html = readFileSync(join(WEB_DIR, file), "utf8");
  const refs = [...html.matchAll(/(?:href|src)="(\/[^"#?]+)"/g)].map((m) => m[1]);
  for (const ref of refs) {
    if (ref.startsWith("//")) continue;
    if (!/\.[a-z0-9]+$/i.test(ref)) continue; /* page routes have no extension */
    if (!existsSync(join(WEB_DIR, ref.replace(/^\//, "")))) missing.push(`${file} → ${ref}`);
  }
}
if (missing.length) {
  console.error("✗ 缺少静态资源：\n" + missing.join("\n"));
  process.exit(1);
}

console.log("→ 复制前端到 dist/");
rmSync(DIST_DIR, { recursive: true, force: true });
mkdirSync(DIST_DIR, { recursive: true });
function copyDir(from, to) {
  mkdirSync(to, { recursive: true });
  for (const entry of readdirSync(from, { withFileTypes: true })) {
    const src = join(from, entry.name);
    const dest = join(to, entry.name);
    if (entry.isDirectory()) copyDir(src, dest);
    else writeFileSync(dest, readFileSync(src));
  }
}
copyDir(WEB_DIR, DIST_DIR);

function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walk(path, out);
    else out.push(path);
  }
  return out;
}

const files = walk(DIST_DIR).map((path) => {
  const bytes = readFileSync(path);
  return {
    path: relative(DIST_DIR, path).replace(/\\/g, "/"),
    bytes: statSync(path).size,
    sha256: createHash("sha256").update(bytes).digest("hex").slice(0, 16),
  };
});
writeFileSync(join(DIST_DIR, "manifest.json"), JSON.stringify({ builtAt: new Date().toISOString(), files }, null, 2));

const total = files.reduce((sum, file) => sum + file.bytes, 0);
console.log(`\n✓ 构建完成 ${((Date.now() - t0) / 1000).toFixed(1)}s`);
console.log(`  binary  ${binaryPath()}`);
console.log(`  static  ${DIST_DIR} (${files.length} 个文件, ${(total / 1024).toFixed(0)} KB)`);

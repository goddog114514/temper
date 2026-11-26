#!/usr/bin/env node
/* Start the dev server against web/ and a local SQLite file. */
import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { ROOT, WEB_DIR, toolchainEnv } from "./env.mjs";

const release = process.argv.includes("--release");
const dbDir = join(ROOT, "data");
mkdirSync(dbDir, { recursive: true });

const addr = process.env.TEMPER_ADDR || "127.0.0.1:8787";
const env = toolchainEnv({
  TEMPER_ADDR: addr,
  TEMPER_DB: process.env.TEMPER_DB || join(dbDir, "temper.db"),
  TEMPER_STATIC: process.env.TEMPER_STATIC || WEB_DIR,
  TEMPER_LOG: process.env.TEMPER_LOG || "temper_server=info,tower_http=warn",
});

console.log(`→ cargo run ${release ? "--release " : ""}-p temper-server`);
console.log(`  http://${addr}  ·  static ${env.TEMPER_STATIC}`);
console.log(`  db ${env.TEMPER_DB}\n`);

const result = spawnSync(
  "cargo",
  ["run", ...(release ? ["--release"] : []), "-p", "temper-server"],
  { stdio: "inherit", env, cwd: ROOT }
);
process.exit(result.status ?? 1);

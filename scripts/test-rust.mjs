#!/usr/bin/env node
/* cargo test with the project-local target dir and toolchain PATH. */
import { spawnSync } from "node:child_process";
import { ROOT, toolchainEnv } from "./env.mjs";

const result = spawnSync("cargo", ["test", "--workspace", ...process.argv.slice(2)], {
  stdio: "inherit",
  env: toolchainEnv(),
  cwd: ROOT,
});
process.exit(result.status ?? 1);

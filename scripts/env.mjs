/* Shared paths and toolchain environment for the Node scripts. */
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

/** Keep all build output inside the project. ROOT must stay ASCII so MinGW/GCC can emit object files. */
export const BUILD_ROOT = process.env.TEMPER_BUILD_DIR || join(ROOT, ".build");

export const TARGET_DIR = process.env.CARGO_TARGET_DIR || join(BUILD_ROOT, "target");
export const TMP_DIR = join(BUILD_ROOT, "tmp");
export const DIST_DIR = join(ROOT, "dist");
export const WEB_DIR = join(ROOT, "web");

function existing(candidates) {
  return candidates.filter((path) => path && existsSync(path));
}

function rustBin() {
  const candidates = [
    process.env.CARGO_HOME && join(process.env.CARGO_HOME, "bin"),
    join(BUILD_ROOT, "..", "cargo", "bin"),
    "C:/temper-tc/cargo/bin",
    join(process.env.USERPROFILE || "", ".cargo", "bin"),
  ];
  return existing(candidates)[0] || "";
}

function mingwBin() {
  const candidates = [
    process.env.TEMPER_MINGW,
    "C:/temper-tc/mingw64/bin",
    "C:/msys64/mingw64/bin",
    join(process.env.USERPROFILE || "", ".toolchains", "mingw64", "bin"),
  ];
  return existing(candidates)[0] || "";
}

export function toolchainEnv(extra = {}) {
  const rust = rustBin();
  const mingw = mingwBin();
  const pathParts = [rust, mingw, process.env.PATH || ""].filter(Boolean);
  for (const dir of [TARGET_DIR, TMP_DIR]) mkdirSync(dir, { recursive: true });
  const env = {
    ...process.env,
    PATH: pathParts.join(";"),
    CARGO_TARGET_DIR: TARGET_DIR,
    TMP: TMP_DIR,
    TEMP: TMP_DIR,
    TMPDIR: TMP_DIR,
    ...extra,
  };
  if (rust) {
    env.CARGO_HOME = dirname(rust);
    env.RUSTUP_HOME = process.env.RUSTUP_HOME || join(dirname(rust), "..", "rustup");
  }
  return env;
}

export function binaryPath() {
  const name = process.platform === "win32" ? "temper-server.exe" : "temper-server";
  return join(TARGET_DIR, process.env.TEMPER_PROFILE === "debug" ? "debug" : "release", name);
}

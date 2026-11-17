#!/usr/bin/env node
/**
 * Builds the portable desktop client.
 *
 * The fleet runs a single `.exe` with no installer (PLAN.md §6), so this skips Tauri's
 * bundler entirely (`--no-bundle`) and copies the bare binary to `release/App Directory.exe`.
 * The frontend is compiled first by Tauri's `beforeBuildCommand`.
 *
 * Usage: pnpm package:desktop        (add --debug to skip LTO for a quick check)
 */

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriDir = join(root, "apps", "desktop", "src-tauri");
const debug = process.argv.includes("--debug");
const profile = debug ? "debug" : "release";

/**
 * The Tauri CLI shells out to `cargo`. On machines where Rust was installed by rustup but
 * the crate bin directory is not on the caller's PATH (common in our shells), prepend it.
 */
const cargoBin = join(process.env.CARGO_HOME ?? join(homedir(), ".cargo"), "bin");
const withCargo = `${cargoBin}${delimiter}${process.env.PATH ?? ""}`;
const childEnv = existsSync(cargoBin) ? { ...process.env, PATH: withCargo } : process.env;

const source = join(tauriDir, "target", profile, "app-directory.exe");
const destinationDir = join(root, "release");
const destination = join(destinationDir, "App Directory.exe");

const config = JSON.parse(readFileSync(join(tauriDir, "tauri.conf.json"), "utf8"));
const args = [
  "--filter",
  "@app-directory/desktop",
  "exec",
  "tauri",
  "build",
  "--no-bundle",
];
if (debug) args.push("--debug");

// pnpm is a .cmd shim on Windows, so the shell is required there. The arguments contain no
// untrusted input, and passing one command string keeps Node from warning about escaping.
const shell = process.platform === "win32";
console.log(`> ${["pnpm", ...args].join(" ")}`);
const build = spawnSync([shell ? "pnpm.cmd" : "pnpm", ...args].join(" "), {
  cwd: root,
  stdio: "inherit",
  shell,
  env: childEnv,
});

if (build.status !== 0) {
  console.error("\nBuild failed.");
  process.exit(build.status ?? 1);
}

let size = 0;
try {
  size = statSync(source).size;
} catch {
  console.error(`\nExpected the binary at ${source}`);
  process.exit(1);
}

mkdirSync(destinationDir, { recursive: true });
copyFileSync(source, destination);

const megabytes = (size / (1024 * 1024)).toFixed(1);
console.log(`\nPortable client ready: ${destination}`);
console.log(`  version ${config.version} · ${megabytes} MB`);
console.log("  Copy this single file to a target machine; it installs nothing.");

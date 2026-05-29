/**
 * Run Tauri CLI with ~/.cargo/bin on PATH (npm scripts often omit it on Windows).
 * Usage: node scripts/run-tauri.mjs [dev|build]
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const mode = process.argv[2] === "build" ? "build" : "dev";

const pathKey = process.platform === "win32" ? "Path" : "PATH";
const pathSep = process.platform === "win32" ? ";" : ":";
const cargoBin = path.join(homedir(), ".cargo", "bin");

const env = { ...process.env };
env[pathKey] = [cargoBin, env[pathKey]].filter(Boolean).join(pathSep);

function findTauriCli() {
  const binName = process.platform === "win32" ? "tauri.cmd" : "tauri";
  const candidates = [
    path.join(repoRoot, "node_modules", ".bin", binName),
    path.join(repoRoot, "frontend", "node_modules", ".bin", binName),
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  return "tauri";
}

const cargoCheck = spawnSync("cargo", ["--version"], { env, shell: process.platform === "win32" });
if (cargoCheck.status !== 0) {
  console.error(
    "cargo was not found on PATH.\n\n" +
      "Install the Rust toolchain from https://rustup.rs/ then open a new terminal.\n" +
      `Expected cargo at: ${path.join(cargoBin, process.platform === "win32" ? "cargo.exe" : "cargo")}`
  );
  process.exit(1);
}

const tauriCli = findTauriCli();
const child = spawn(tauriCli, [mode], {
  cwd: repoRoot,
  env,
  stdio: "inherit",
  shell: process.platform === "win32",
});

child.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

child.on("exit", (code) => {
  process.exit(code ?? 1);
});

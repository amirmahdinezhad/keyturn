import { spawn, spawnSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const label = "com.7skylab.keyturn.dev";
const supervisorLog = "/tmp/keyturn-dev-supervisor.log";
const mode = process.argv[2];

function job() {
  const result = spawnSync("/bin/launchctl", ["list", label], { encoding: "utf8" });
  return result.status === 0 ? Number(result.stdout.match(/"PID"\s*=\s*(\d+)/)?.[1] ?? 0) : 0;
}

/** launchd owns dev so closing Codex cannot kill Vite or the restart worker. */
function startIndependent() {
  if (job()) {
    console.log(`Keyturn dev is already running (supervisor ${job()}).`);
    return;
  }
  // A failed/exited submit job keeps its label until explicitly removed.
  spawnSync("/bin/launchctl", ["remove", label], { stdio: "ignore" });
  const env = ["PATH=" + process.env.PATH];
  for (const key of ["CODEX_HOME", "KEYTURN_OPENCODE_USERDATA"]) {
    if (process.env[key]) env.push(`${key}=${process.env[key]}`);
  }
  const result = spawnSync("/bin/launchctl", [
    "submit", "-l", label, "-o", supervisorLog, "-e", supervisorLog,
    "--", "/usr/bin/env", ...env, process.execPath, fileURLToPath(import.meta.url), "--worker",
  ], { encoding: "utf8" });
  if (result.status !== 0) throw new Error(`Cannot start independent dev: ${result.stderr.trim()}`);
  console.log(`Keyturn dev started independently of Codex on http://localhost:5174/. Logs: ${supervisorLog}`);
}

/** Keep the existing tray panel/Vite behavior; shut down both process groups together. */
async function supervise() {
  const services = [
    ["vite", ["exec", "vite", "--port", "5174", "--strictPort"], "/tmp/keyturn-vite-5174.log"],
    ["tauri", ["exec", "tauri", "dev", "--config", JSON.stringify({ build: { devUrl: "http://localhost:5174", beforeDevCommand: "" } })], "/tmp/keyturn-tauri-dev.log"],
  ];
  const children = [];
  const streams = [];
  let stopping = false;
  const stop = (code) => {
    if (stopping) return;
    stopping = true;
    for (const child of children) {
      try { process.kill(-child.pid, "SIGTERM"); } catch { /* Already exited. */ }
    }
    // Reap the whole dev group, including the cargo-watched native child.
    setTimeout(() => {
      for (const child of children) {
        try { process.kill(-child.pid, "SIGKILL"); } catch { /* Already exited. */ }
      }
      for (const stream of streams) stream.end();
      process.exit(code);
    }, 2000);
  };
  process.on("SIGTERM", () => stop(0));
  process.on("SIGINT", () => stop(0));
  for (const [name, args, log] of services) {
    const stream = createWriteStream(log, { flags: "w" });
    streams.push(stream);
    const child = spawn("pnpm", args, { cwd: root, detached: true, stdio: ["ignore", "pipe", "pipe"] });
    children.push(child);
    child.stdout.pipe(stream, { end: false });
    child.stderr.pipe(stream, { end: false });
    child.on("error", (error) => { console.error(`${name}: ${error.message}`); stop(1); });
    child.on("exit", (code) => { console.log(`${name} exited (${code}).`); stop(code ?? 1); });
    console.log(`${name} started (${child.pid}); ${log}`);
  }
}

try {
  if (mode === "--worker") await supervise();
  else if (process.platform !== "darwin") {
    // Preserve the existing foreground CLI workflow on other platforms.
    const command = process.env.npm_execpath ? process.execPath : "pnpm";
    const prefix = process.env.npm_execpath ? [process.env.npm_execpath] : [];
    const result = spawnSync(command, [...prefix, "exec", "tauri", "dev"], { cwd: root, stdio: "inherit" });
    process.exitCode = result.status ?? 1;
  }
  else if (mode === "--stop") {
    const wasRunning = !!job();
    const result = spawnSync("/bin/launchctl", ["remove", label], { encoding: "utf8" });
    if (wasRunning && result.status !== 0) throw new Error(result.stderr.trim());
  } else if (mode === "--status") {
    const pid = job();
    if (!pid) throw new Error("Independent Keyturn dev is not running.");
    const parent = spawnSync("/bin/ps", ["-o", "ppid=", "-p", String(pid)], { encoding: "utf8" });
    if (parent.status !== 0 || parent.stdout.trim() !== "1") throw new Error("Dev supervisor is not owned by launchd.");
    console.log(`Independent Keyturn dev: supervisor=${pid}, parent=launchd (1), port=5174.`);
  } else startIndependent();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}

import { spawnSync } from "node:child_process";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ownedEntries = [
  "agent-rtl.keys", "agent-rtl.settings.json", "keyturn-codex-profiles",
  "keyturn-claude-profiles", "keyturn-usage-alerts.json",
  "keyturn-login-homes", "keyturn-antigravity-profiles",
];

/** Delete an explicit allowlist, never the shared OpenCode directory or CLI homes. */
export async function resetStorage({ userHome, dataDir }) {
  for (const name of ownedEntries) await rm(path.join(dataDir, name), { recursive: true, force: true });
  for (const location of ["WebKit", "Caches"]) {
    for (const identifier of ["keyturn", "com.7skylab.keyturn"]) {
      await rm(path.join(userHome, "Library", location, identifier), { recursive: true, force: true });
    }
  }
  await mkdir(dataDir, { recursive: true });
  // Skip legacy migration so the existing CLI login appears in discovery,
  // rather than silently creating a Personal workspace on the next launch.
  await writeFile(path.join(dataDir, "agent-rtl.settings.json"), JSON.stringify({
    workspaceVersion: 1, workspaces: [], activeWorkspaceId: null,
  }, null, 2) + "\n", { mode: 0o600 });
}

function runDev(mode) {
  const result = spawnSync(process.execPath, [path.join(root, "scripts/dev.mjs"), ...mode], { cwd: root, stdio: "inherit" });
  if (result.status !== 0) throw new Error("Could not control Keyturn dev; data reset stopped.");
}

async function reset() {
  if (process.platform !== "darwin") throw new Error("reset:data currently supports the macOS dev environment.");
  // Identify only this project's executables and the installed Keyturn app.
  const executables = [
    path.join(root, "src-tauri/target/debug/keyturn"),
    path.join(root, "src-tauri/target/release/keyturn"),
    path.join(root, "src-tauri/target/release/bundle/macos/Keyturn.app/Contents/MacOS/keyturn"),
    "/Applications/Keyturn.app/Contents/MacOS/keyturn",
  ];
  const found = spawnSync("/usr/sbin/lsof", ["-t", ...executables], { encoding: "utf8" });
  if (![0, 1].includes(found.status)) throw new Error("Could not identify running Keyturn processes; data reset stopped.");
  const pids = [...new Set(found.stdout.trim().split(/\s+/).filter(Boolean).map(Number))];
  runDev(["--stop"]);
  for (const pid of pids) {
    try { process.kill(pid, "SIGTERM"); } catch (error) { if (error.code !== "ESRCH") throw error; }
  }
  const deadline = Date.now() + 10000;
  while (pids.some((pid) => {
    try { process.kill(pid, 0); return true; } catch (error) { if (error.code === "ESRCH") return false; throw error; }
  })) {
    if (Date.now() > deadline) throw new Error("Keyturn is still running; no data was deleted. Quit it and retry.");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  // Let the supervisor reap its process groups and WebKit finish shutting down.
  await new Promise((resolve) => setTimeout(resolve, 2500));
  const userHome = homedir();
  const dataDir = process.env.KEYTURN_OPENCODE_USERDATA || path.join(userHome, "Library/Application Support/ai.opencode.desktop");
  await resetStorage({ userHome, dataDir });
  console.log("Keyturn data cleared. Provider CLI logins and OpenCode data were preserved.");
  runDev([]);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  reset().catch((error) => { console.error(error.message); process.exitCode = 1; });
}

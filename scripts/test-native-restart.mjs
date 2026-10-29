import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import path from "node:path";

if (process.platform !== "darwin") throw new Error("Native lifecycle verification requires macOS.");
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const temporary = mkdtempSync(path.join(tmpdir(), "keyturn-restart-test-"));
const app = path.join(temporary, "Keyturn Restart Fixture.app");
const binary = path.join(app, "Contents/MacOS/fixture");
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const check = (command, args, options = {}) => {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit", ...options });
  if (result.status !== 0) throw new Error(`${command} failed (${result.status}).`);
};
function fixturePids() {
  const result = spawnSync("/bin/ps", ["-ax", "-o", "pid=,comm="], { encoding: "utf8" });
  return result.stdout.split("\n").flatMap((line) => {
    const match = line.trim().match(/^(\d+)\s+(.*)$/);
    try { return match && realpathSync(match[2]) === realpathSync(binary) ? [Number(match[1])] : []; }
    catch { return []; }
  });
}
try {
  mkdirSync(path.dirname(binary), { recursive: true });
  writeFileSync(path.join(app, "Contents/Info.plist"), `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>fixture</string>
<key>CFBundleIdentifier</key><string>com.7skylab.keyturn.restart-fixture</string>
<key>CFBundleName</key><string>Keyturn Restart Fixture</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSUIElement</key><true/>
</dict></plist>`);
  check("/usr/bin/xcrun", ["swiftc", path.join(root, "scripts/native-restart-fixture.swift"), "-o", binary]);
  check("/usr/bin/open", ["-a", app]);
  const deadline = Date.now() + 5000;
  while (!fixturePids().length) {
    if (Date.now() > deadline) throw new Error("Fixture did not start.");
    await pause(100);
  }
  check("cargo", ["test", "--offline", "native_restart_installs_after_exit_and_launches_the_same_app_again", "--", "--ignored", "--nocapture"], { cwd: path.join(root, "src-tauri") });
} finally {
  for (const pid of fixturePids()) {
    try { process.kill(pid, "SIGTERM"); } catch { /* Already exited. */ }
  }
  await pause(100);
  rmSync(temporary, { recursive: true, force: true });
}

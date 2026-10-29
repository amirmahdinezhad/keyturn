import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync, copyFileSync, chmodSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const directory = "/tmp/keyturn-motion";
const app = path.join(directory, "Keyturn Motion QA.app");
const binary = path.join(app, "Contents/MacOS/keyturn-motion-qa");
const run = (cmd, args) => {
  const result = spawnSync(cmd, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${cmd} exited ${result.status}`);
};
mkdirSync(path.dirname(binary), { recursive: true });
writeFileSync(path.join(directory, "tauri.qa.json"), JSON.stringify({
  identifier: "com.7skylab.keyturn.motion-qa", productName: "Keyturn Motion QA",
  build: { beforeBuildCommand: "", frontendDist: path.join(directory, "dist") },
  app: {
    windows: [{ label: "motion", title: "Keyturn Motion QA", width: 430, height: 600, minWidth: 380, minHeight: 400, decorations: false, transparent: true, alwaysOnTop: true, skipTaskbar: false, visible: true, resizable: false, focus: true, acceptFirstMouse: true, center: true }],
    security: { capabilities: [{ identifier: "motion", windows: ["motion"], permissions: ["core:default", "core:app:allow-version"] }] },
  },
  bundle: { active: false },
}));
run("pnpm", ["exec", "vite", "build", "--config", "scripts/motion/vite.config.ts"]);
run("pnpm", ["exec", "tauri", "build", "--no-bundle", "--config", path.join(directory, "tauri.qa.json"), "--", "--offline"]);
copyFileSync(path.join(root, "src-tauri/target/release/keyturn"), binary);
chmodSync(binary, 0o755);
writeFileSync(path.join(app, "Contents/Info.plist"), `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>keyturn-motion-qa</string>
<key>CFBundleIdentifier</key><string>com.7skylab.keyturn.motion-qa</string>
<key>CFBundleName</key><string>Keyturn Motion QA</string>
<key>CFBundleDisplayName</key><string>Keyturn Motion QA</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSUIElement</key><true/><key>NSHighResolutionCapable</key><true/>
<key>LSEnvironment</key><dict><key>KEYTURN_OPENCODE_USERDATA</key><string>/tmp/keyturn-motion/state</string></dict>
</dict></plist>`);
run("codesign", ["--force", "--deep", "--sign", "-", app]);
console.log(`Isolated optimized fixture: ${app}`);

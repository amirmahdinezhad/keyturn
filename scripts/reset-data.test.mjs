import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, readFile, access, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { resetStorage } from "./reset-data.mjs";

test("reset is repeatable, clears only Keyturn data and keeps onboarding empty", async () => {
  const userHome = await mkdtemp(path.join(tmpdir(), "keyturn-reset-"));
  const dataDir = path.join(userHome, "shared-opencode");
  const put = async (file) => { await mkdir(path.dirname(file), { recursive: true }); await writeFile(file, "fixture"); };
  const removed = [
    path.join(dataDir, "keyturn-codex-profiles/profile/auth.json"),
    path.join(dataDir, "keyturn-claude-profiles/profiles.json"),
    path.join(dataDir, "agent-rtl.keys"),
    path.join(dataDir, "keyturn-login-homes/draft/auth.json"),
    path.join(dataDir, "keyturn-antigravity-profiles/profile/auth.json"),
    path.join(dataDir, "keyturn-usage-alerts.json"),
    path.join(userHome, "Library/WebKit/keyturn/localstorage"),
    path.join(userHome, "Library/Caches/com.7skylab.keyturn/cache"),
  ];
  const preserved = [
    path.join(dataDir, "opencode.db"),
    path.join(userHome, ".codex/auth.json"),
    path.join(userHome, ".claude/settings.json"),
    path.join(userHome, "Library/WebKit/another-app/localstorage"),
  ];
  try {
    for (const file of [...removed, ...preserved]) await put(file);
    await resetStorage({ userHome, dataDir });
    await resetStorage({ userHome, dataDir });
    for (const file of removed) await assert.rejects(access(file), { code: "ENOENT" });
    for (const file of preserved) assert.equal(await readFile(file, "utf8"), "fixture");
    assert.deepEqual(JSON.parse(await readFile(path.join(dataDir, "agent-rtl.settings.json"), "utf8")), {
      workspaceVersion: 1, workspaces: [], activeWorkspaceId: null,
    });
  } finally { await rm(userHome, { recursive: true, force: true }); }
});

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { validateManifest, repository } from "./release.mjs";

const notes = "English: Account improvements.\nفارسی: بهبود حساب‌ها.";
const asset = { url: `https://github.com/${repository}/releases/download/v1.0.0/Keyturn_universal.app.tar.gz`, signature: Buffer.from("untrusted comment: test\nsignature\ntrusted comment: test\nglobal-signature\n").toString("base64") };
const manifest = () => ({ version: "1.0.0", notes, pub_date: "2026-10-30T00:00:00Z", platforms: { "darwin-aarch64": { ...asset }, "darwin-x86_64": { ...asset } } });

test("accepts a universal feed with reviewed release notes", () => {
  validateManifest(manifest(), "1.0.0", notes);
});
test("rejects stale versions, missing Intel support and unreviewed notes", () => {
  const missingIntel = manifest(); delete missingIntel.platforms["darwin-x86_64"];
  for (const invalid of [missingIntel, { ...manifest(), version: "0.9.0" }, { ...manifest(), notes: "placeholder" }]) {
    assert.throws(() => validateManifest(invalid, "1.0.0", notes));
  }
});
test("rejects unsigned, mutable and foreign downloads", () => {
  for (const replacement of [{ ...asset, signature: "" }, { ...asset, url: asset.url.replace("download/v1.0.0", "latest/download") }, { ...asset, url: asset.url.replace(repository, "someone/else") }]) {
    const invalid = manifest(); invalid.platforms["darwin-aarch64"] = replacement;
    assert.throws(() => validateManifest(invalid, "1.0.0", notes));
  }
});
test("publication without explicit approval fails before invoking GitHub", () => {
  assert.throws(() => execFileSync(process.execPath, ["scripts/release.mjs", "publish"], { stdio: "pipe" }), (error) => {
    assert.match(error.stderr.toString(), /explicit user approval/);
    return true;
  });
});

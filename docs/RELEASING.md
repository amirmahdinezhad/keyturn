# Releasing

The GitHub workflow is configured for signed, notarized universal macOS builds
and an updater feed. Homebrew and Nix files are packaging templates.

Create a native release build or publish a release only when explicitly asked.
Routine verification uses the dev app; do not replace the installed app.

## One-time setup

1. **Updater signing key**

   ```bash
   pnpm exec tauri signer generate -w ~/.tauri/keyturn.key
   ```

   `src-tauri/tauri.conf.json` already contains an updater public key. Reuse its
   matching private key for CI; generating a replacement requires updating the
   public key too. The current workflow expects a passwordless private key.
   Local config keeps `bundle.createUpdaterArtifacts` off; CI enables it.

2. **Apple signing + notarization**

   Create a Developer ID Application certificate and an app-specific password
   for notarization. CI expects these repository secrets:

   | Secret | Value |
   | --- | --- |
   | `APPLE_CERTIFICATE` | base64 of the exported `.p12` |
   | `APPLE_CERTIFICATE_PASSWORD` | password for the `.p12` |
   | `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
   | `APPLE_ID` | Apple account email |
   | `APPLE_PASSWORD` | app-specific password |
   | `APPLE_TEAM_ID` | 10-character team id |
   | `TAURI_SIGNING_PRIVATE_KEY` | contents of the updater private key |

3. **Update endpoint**

   `tauri.conf.json` points at
   `https://github.com/7SkyLab/keyturn/releases/latest/download/latest.json`.
   `tauri-action` publishes that file with every release.

## Cutting a release

```bash
git tag v0.2.0
git push origin v0.2.0
```

`.github/workflows/release.yml` installs dependencies with pnpm 11 and the frozen
lockfile, builds a universal macOS bundle, and uses the configured secrets for
signing/notarization. It creates a **draft** GitHub release with the native
assets and `latest.json`. Validate the draft assets before publishing; clients
check the latest published release.

The dev updater UI can show the running version and check errors. Its mocked
tests cover release notes, skip-version storage, download progress and
install/relaunch. Actual installation requires a compatible signed release
and is not part of the normal dev checklist.

## Homebrew

`packaging/homebrew/keyturn.rb` is a cask template. After a release:

1. Update `version` and `sha256` (from the release `.dmg`).
2. Copy the file into your tap (`7SkyLab/homebrew-tap`) and push.

Users then run `brew install --cask 7skylab/tap/keyturn`.

## DMG window

The installer window's background, icon positions and size live in
`bundle > macOS > dmg` in `src-tauri/tauri.conf.json`. The artwork itself is
`packaging/dmg/background.png` (660×400, the window size); its editable source is
`packaging/dmg/background.html` — open it at 660×400 and re-capture after any
change, then replace the PNG.

## Nix

`flake.nix` contains a dev shell and an unfinished package template. The package
still has `npmDepsHash = pkgs.lib.fakeHash` and an npm-based install/build phase,
while the release workflow uses pnpm. It has not been validated by the dev
checklist; resolve dependency fetching and platform inputs before using it to
distribute a release.

After completing and validating the template, its package commands are:

```bash
nix build .#default
nix run .#default
```

# Releasing

Signed and notarized macOS builds, the auto-updater feed, Homebrew and Nix.

## One-time setup

1. **Updater signing key**

   ```bash
   npm run tauri signer generate -- -w ~/.tauri/keyturn.key
   ```

   Put the public key into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
   (replacing `REPLACE_WITH_TAURI_SIGNER_PUBLIC_KEY`) and keep the private key
   for CI. Local builds keep `bundle.createUpdaterArtifacts` off so they work
   without the key; the release workflow turns it on and signs the artifacts.

2. **Apple signing + notarization**

   Create a Developer ID Application certificate and an app-specific password
   (or App Store Connect API key). CI expects these repository secrets:

   | Secret | Value |
   | --- | --- |
   | `APPLE_CERTIFICATE` | base64 of the exported `.p12` |
   | `APPLE_CERTIFICATE_PASSWORD` | password for the `.p12` |
   | `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
   | `APPLE_ID` | Apple account email |
   | `APPLE_PASSWORD` | app-specific password |
   | `APPLE_TEAM_ID` | 10-character team id |
   | `TAURI_SIGNING_PRIVATE_KEY` | contents of the updater private key |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | its password |

3. **Update endpoint**

   `tauri.conf.json` points at
   `https://github.com/7SkyLab/keyturn/releases/latest/download/latest.json`.
   `tauri-action` publishes that file with every release.

## Cutting a release

```bash
git tag v0.2.0
git push origin v0.2.0
```

`.github/workflows/release.yml` builds a universal macOS bundle, signs and
notarizes it, creates the GitHub release and attaches the `.dmg`, `.app.tar.gz`
and `latest.json` (the updater feed).

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

`flake.nix` provides a dev shell and a package build. Point `version`/`src` at
the release tag and run:

```bash
nix build .#keyturn
nix run .#keyturn
```

Because a Tauri build needs both Node and Rust dependencies, refresh the npm
hash (`nix build` prints the expected value on mismatch) whenever
`package-lock.json` changes.

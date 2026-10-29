# Releasing Keyturn

Source and releases: <https://github.com/amirmahdinezhad/keyturn>.
The release target is universal macOS 12+ (Apple Silicon and Intel).

## Prepare and obtain approval

For every feature or bug fix, finish implementation and the `AGENTS.md` local
checklist in the native **dev tray panel** first. Keep development on port 5174.
Prepare a patch version for compatible fixes, a minor version for features, or a
major version for breaking changes. Synchronize `package.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, the keyturn entry in
`src-tauri/Cargo.lock`, and Homebrew/Nix template versions. Update versioned README
download links and add English/Persian notes in `docs/releases/<version>.md`.
Run `pnpm run release:check` and `pnpm run test:release`.

Present the tested changes, proposed version and notes to the owner. Wait for
explicit approval for that release before pushing its tag or publishing it.
An explicit request to publish a named version already authorizes that version;
it does not authorize later releases. Successful local tests are not approval.

## Signing and GitHub setup

The public endpoint is
`https://github.com/amirmahdinezhad/keyturn/releases/latest/download/latest.json`.
Installed apps do not need a GitHub account or token.

The updater public key in `tauri.conf.json` is the **base64-encoded complete
Minisign public-key file**, including its comment. The matching private key is
outside the repository at `~/.tauri/keyturn-release.key`. Preserve and securely back up
this key: replacing it breaks trust for existing installations.

GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY` holds the base64-encoded
complete private-key file, not its pathname or its raw Minisign line. Set
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if password protected. Never print these
values or put them in repository files. Local `createUpdaterArtifacts` stays
false; release builds override it to true.

Current macOS builds use ad-hoc code signing (`signingIdentity: "-"`) and are
**not Apple notarized**. The updater archive has a separate cryptographic
signature verified by Tauri. Developer ID signing/notarization requires an Apple
certificate and a notarization step before it can be advertised. First-launch
macOS approval is explained in the README.

## Publish an approved release

1. Commit the reviewed app source, scripts and public docs. Keep credentials,
   account data, screenshots and QA recordings out. Preserve unrelated work.
   Push only the intended branch and tag, never checkpoint refs or a mirror.
2. Push the reviewed commit and annotated version tag, for example:

   ```bash
   git push origin main
   git tag -a v1.0.1 -m "Keyturn 1.0.1"
   git push origin v1.0.1
   ```

3. Wait for `.github/workflows/release.yml`. It checks the tag/version, frontend
   and Rust tests, builds the universal app, signs and verifies its updater,
   then creates a **draft**. Pushing main runs checks without publishing.
4. Review the draft DMG, `Keyturn_universal.app.tar.gz`, `.sig`, `latest.json`
   and `SHA256SUMS`. Feed notes and release body must match the reviewed notes.
   Both darwin-aarch64 and darwin-x86_64 use the universal archive.
   `pnpm run release:verify <asset-directory>` checks notes, immutable URLs,
   checksums, embedded app version, CPU architectures, macOS code signature,
   DMG integrity and the updater signature using Tauri's Minisign library.
   Inspect the DMG once; never overwrite assets of a published version.
5. After owner approval, with a clean tracked tree at the tagged commit, run:

   ```bash
   pnpm run release:publish --approved
   ```

   It downloads and verifies the draft, publishes it as latest, and checks the
   public feed and updater download without authentication. The flag records
   the operator's decision; it is not a substitute for owner approval.
6. Open the native dev panel → Settings → Updates → Check for updates. Confirm
   the running version and server result. Test an actual upgrade on a separate
   installed test copy when an older public version exists. Report the release
   URL, all local checks and any installation test not performed.

For an explicitly authorized local build, provide the base64 private-key file
contents in the process environment, install both Rust macOS targets, and run the
same Tauri command as CI. Stage it using
`node scripts/release.mjs assemble src-tauri/target/universal-apple-darwin/release/bundle`.
Routine verification must leave `/Applications/Keyturn.app` intact.

## Installed users

Settings → Updates → **Check for updates** → **What's new** → **Download** →
**Install & relaunch**. Manual checks reveal previously skipped releases.
Opt-in daily automatic checks respect skips, can download with the panel closed,
and wait for explicit installation. Release notes remain visible after download.
Drafts and prereleases do not become the public latest update.

Pre-1.0 development builds used the old `7SkyLab/keyturn` endpoint and need one
manual install of 1.0.0. Releases from 1.0.0 onward share the new feed and key.

## Packaging templates

`packaging/homebrew/keyturn.rb` is an unpublished cask template; set the release
DMG checksum before using it in a tap. `flake.nix` has a fake dependency hash and
is an unvalidated development/package template, not a supported distribution.
The DMG background and editable source are in `packaging/dmg/`; dimensions and
icon positions are in `src-tauri/tauri.conf.json`.

References: [Tauri updater](https://v2.tauri.app/plugin/updater/) and
[macOS distribution](https://v2.tauri.app/distribute/macos-application-bundle/).

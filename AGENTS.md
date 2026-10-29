# Keyturn — agent rules

## Release workflow (every feature and bug fix)

- Read `docs/RELEASING.md` when preparing, building or publishing a release.
- Implement the change and complete the checklist below in the local native dev
  panel. Prepare the next SemVer version and user-facing English/Persian notes
  in `docs/releases/<version>.md`; keep the app versions synchronized.
- Present the tested changes, proposed version and release notes for the user's
  approval. A successful local test is not permission to publish. Wait for
  explicit approval for that particular release before pushing its tag or
  publishing it. An explicit request to publish a named version is approval for
  that version only, not for later releases.
- After approval, push the reviewed commit and `v<version>` tag to
  `amirmahdinezhad/keyturn`. GitHub Actions builds a **draft**, never a public
  release automatically. Verify the DMG, updater archive, signature, both macOS
  architectures, checksums, release notes and `latest.json` before publishing
  with `pnpm run release:publish --approved`.
- Confirm the public feed and assets work without GitHub authentication and
  check Settings → Updates in the native panel. Report the release URL and
  checklist results. Installed users must be able to read the notes, download,
  then explicitly choose Install & relaunch.
- Reuse the existing updater signing key; keep its private half outside this
  repository and in GitHub Actions secrets. Release artifacts must come from
  the exact tagged commit. Preserve uncommitted work and keep local account
  data, credentials, screenshots and QA recordings out of public commits.

## Dev stack (do not change ports)

- Frontend: `vite --port 5174 --strictPort` → http://localhost:5174/
- Combined startup: `pnpm run run` starts Vite and Tauri on the configured port 5174.
- App: `tauri dev --config '{"build":{"devUrl":"http://localhost:5174","beforeDevCommand":""}}'`
  when Vite is already running. Both default configs use **5174**.
- The real UI surface is the **tray panel** (menu-bar icon click), not a browser
  tab. Tray clicks show the existing panel instantly; frontend updates arrive
  through Vite HMR. Keep this behavior in dev and release builds.
- Leave `custom-protocol` out of the Tauri dependency's permanent features.
  The Tauri CLI enables it for production builds. Enabling it during dev loads
  embedded `dist` assets and silently disconnects the panel from Vite.

## After EVERY task — run this checklist, no exceptions

1. **Typecheck**: `pnpm exec tsc --noEmit` (must exit 0).
2. **Tests**: `pnpm exec vitest run` (all files must pass).
3. **Live on dev** (for any change under `src/`):
   a. Served source: `curl -s "http://localhost:5174/src/path/to/File.tsx" | grep -c "<expected-string>"`
   b. HMR/reload pushed: `tail -n 3 /tmp/keyturn-vite-5174.log`
   c. Live page (preview browser on 5174) via snapshot/evaluate — e.g. no stale
      copy in `document.body.innerText`.
   d. Rust changes (`src-tauri/`): `cargo check --offline` in `src-tauri/`,
      then confirm the `tauri dev` watcher rebuilt
      (`tail /tmp/keyturn-tauri-dev.log` shows `Finished dev profile`,
      new `target/debug/keyturn` PID).
4. **Report** the checklist results (typecheck/tests/live) in the final message.

## Notes

- A plain browser has no Tauri IPC, so the app shows the
  "Couldn't load settings … (reading 'invoke')" empty state there. That is
  expected — the real provider list only renders inside the Tauri window.
  Browser/source checks alone do not prove that the native panel is current.
- `CliProviders.tsx` also exports `formatAgo`/`formatReset`, so React Fast
  Refresh bails out (`hmr invalidate … Could not Fast Refresh`) and the client
  does a full page reload instead. Do not mistake the reload for a failure.
- To prove the panel's webview receives HMR while open: move all preview tabs
  off port 5174, confirm an established WebKit connection with
  `lsof -nP -iTCP:5174`, then `touch` a frontend file and confirm a fresh
  `hmr update` line in `/tmp/keyturn-vite-5174.log`. An HMR log without a
  connected native client is insufficient.

# CodexBar account enrollment reference

Inspected 2026-10-28 at CodexBar commit
[`5f93dc3068b3bee3f8753f3772614cc2949c335d`](https://github.com/steipete/CodexBar/tree/5f93dc3068b3bee3f8753f3772614cc2949c335d).
Keyturn implements the behavior in Rust and React; it does not embed CodexBar.

| Provider | Primary source inspected | Keyturn behavior |
| --- | --- | --- |
| Codex | [CodexLoginRunner.swift](https://github.com/steipete/CodexBar/blob/5f93dc3068b3bee3f8753f3772614cc2949c335d/Sources/CodexBar/Providers/Codex/CodexLoginRunner.swift) | Official installed CLI login in an isolated `CODEX_HOME`; browser and device code. Imported auth.json is also staged and verified through CLI app-server `account/read` with refresh enabled. |
| Claude | [ClaudeLoginRunner.swift](https://github.com/steipete/CodexBar/blob/5f93dc3068b3bee3f8753f3772614cc2949c335d/Sources/CodexBar/Providers/Claude/ClaudeLoginRunner.swift) | Real PTY for `claude auth login --claudeai`, Enter forwarding and optional authorization-code forwarding. Stable, isolated configuration paths preserve the CLI's Keychain scope. Existing configurations remain references. |
| Antigravity | [AntigravityLoginRunner.swift](https://github.com/steipete/CodexBar/blob/5f93dc3068b3bee3f8753f3772614cc2949c335d/Sources/CodexBar/Providers/Antigravity/AntigravityLoginRunner.swift), [OAuth credentials store](https://github.com/steipete/CodexBar/blob/5f93dc3068b3bee3f8753f3772614cc2949c335d/Sources/CodexBarCore/Providers/Antigravity/AntigravityOAuthCredentialsStore.swift), [remote usage fetcher](https://github.com/steipete/CodexBar/blob/5f93dc3068b3bee3f8753f3772614cc2949c335d/Sources/CodexBarCore/Providers/Antigravity/AntigravityRemoteUsageFetcher.swift) | Experimental loopback Google OAuth with random state. Resolve a paired OAuth client from a compatible installed artifact or explicit environment pair; no bundled secret. Import compatible credential JSON or explicitly read the current AGY CLI session. Verify Google identity and paid tier remotely. |

All methods stage accounts before final membership save. Authentication URLs do
not indicate success; the native layer waits for completion and verifies paid
eligibility. API keys and free/unknown plans do not qualify. Quota percentages
and model availability are not used as evidence of a paid Antigravity plan.

Raw CLI transcripts stay native and bounded. Only recognized provider HTTPS
login URLs, device codes, status and safe errors cross into the panel. Account
secrets are not included in command results. File content can travel from the
file chooser to native validation. Cancel/exit stops the owned login process and
cleans app-owned draft files; ambient provider logins are not logged out.

Validation uses synthetic CLI/PTY processes, callback-state tests and mocked UI
flows. These prove orchestration and failure handling, not a completed real
provider sign-in. No actual user login was completed during this implementation.
Antigravity remains experimental because its client artifacts and remote schema
are version-dependent.

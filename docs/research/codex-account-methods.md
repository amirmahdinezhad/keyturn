# Codex account addition and switching methods

Researched on 2026-10-14 using official OpenAI documentation. This report distinguishes documented authentication mechanisms, the user's observed desktop behavior, and proposed Keyturn behavior. No real credentials were opened, changed, uploaded, or used for sign-in during this research.

## Recommended product scope

For this user's setup, prioritize named profiles with three entry points: **Import auth.json**, **Sign in with ChatGPT**, and **Save current account**. Keep adding an account separate from activating it. Support the user's existing desktop sequence: close Codex, activate the selected profile in Keyturn, then reopen Codex.

The user reports that replacing the file while Codex is closed loads the selected account on reopening. That is useful local evidence, but it is not a documented desktop switching API. Official documentation explicitly supports copying a Codex authentication cache as a fallback; it does not establish that replacing a file changes an already-running desktop session. [Authentication](https://learn.chatgpt.com/docs/auth#fallback-authenticate-locally-and-copy-your-auth-cache)

| Method | Suitable use | Recommendation for Keyturn |
| --- | --- | --- |
| Import a supplied `auth.json` | Work login supplied as a file | **First priority:** native file picker, name, validation, save without activation |
| Save the current Codex login | Personal or work account already signed in | **First priority:** one action, editable suggested name |
| Browser sign-in through Codex CLI | New personal account; each worker's own workspace login | **First priority:** let Codex handle OAuth in an isolated temporary home, then save the result |
| Device-code sign-in | Callback or browser-launch problems | **Next:** fallback with link, code, cancellation and completion state |
| Owned local app-server sign-in | Fully integrated native login UI | **Next:** structured progress instead of parsing CLI output; use an isolated process |
| Separate `CODEX_HOME` per account | Simultaneous CLI accounts and isolated state | **Optional advanced mode:** useful for launchers, not a proven desktop account switch |
| OS Keychain / keyring | Users or organizations selecting native credential storage | **Next infrastructure step:** backend support before claiming universal switching |
| Platform API key | API billing, automation, non-subscription usage | **Optional distinct account type:** disclose separate API charges |
| Enterprise Codex access token / service account | Trusted scripts and private automation | **Advanced only:** not the default personal desktop login flow |
| Sign in with ChatGPT OSS registration | Keyturn eventually runs its own AI workloads | **Separate future integration:** does not supply a documented drop-in Codex desktop cache |
| Workload identity federation | Managed enterprise automation | **Out of the current account-picker scope:** admin setup and runtime identity required |

The priorities in this table are product recommendations, not claims that all rows are currently implemented.

Implementation update: Keyturn now supports named profiles, current-account
capture, native file import/replacement, isolated CLI browser login, and terminal
device-code fallback. On macOS, file import and Set active start a cancellable
30-second graceful desktop restart. Local verification covers the installed
cache identity and a new process; account identity inside the first-party app
still requires separate validation. See the implementation verification report.

## Existing Codex cache: importing and saving

Codex caches a login in `CODEX_HOME/auth.json` or the OS credential store. CLI and IDE extension share cached credentials. The file contains secrets and must stay out of repositories, telemetry and diagnostic output. ChatGPT login uses subscription access; Platform API-key usage is billed separately at API rates. [Authentication](https://learn.chatgpt.com/docs/auth)

Credential-store selection matters:

| `cli_auth_credentials_store` | Meaning for switching |
| --- | --- |
| `file` | `auth.json` is the file-backed credential cache |
| `keyring` | Requires the OS store; replacing a JSON file does not update that store |
| `auto` | OS store when available, file fallback otherwise; a file alone is not proof of the selected account |
| `ephemeral` | Credentials remain in the current process; no persistent cache to switch |

Local requirements or MDM can enforce the selected store. Keyturn should identify unsupported storage and respect managed requirements. [Configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)

Managed `forced_login_method` and `forced_chatgpt_workspace_id` can reject credentials for another method or workspace. A profile picker cannot bypass those restrictions. [Authentication](https://learn.chatgpt.com/docs/auth#enforce-a-login-method-or-workspace)

Proposed import behavior: read the user-selected file in the Rust backend, reject invalid or unsupported content before saving, retain the complete supported credential payload, return display metadata only, and leave the current Codex cache untouched. A structurally valid file is not proof that its tokens are live. Activation should preserve the outgoing account's latest cache and replace the active file atomically with restricted permissions.

## Refreshing imported accounts

Codex manages refresh and writes renewed credentials back to its cache. The official advanced cache-copy workflow requires persisting that updated file, using a single machine or serialized stream per copy, and avoiding concurrent copies. It names token rotation by another machine, expired or revoked refresh tokens, and restoration of an older file as reasons authentication can stop working. [Maintaining Codex account auth](https://learn.chatgpt.com/docs/auth/ci-cd-auth)

**Implementation implication:** capture the active file before switching away, so a profile does not restore its original stale seed. A file handed over by a manager is a snapshot of an existing login, not an independently renewable account registration. If that login is also active elsewhere, refresh conflicts may occur. Prefer each worker's own authorized workspace sign-in when available; otherwise offer **Replace imported file** / **Reconnect** recovery without claiming indefinite validity.

## Easy addition of a new personal account

`codex login` opens the ChatGPT OAuth browser flow. `codex login --device-auth` uses device-code authentication. The CLI also supports `--with-api-key`, `--with-access-token`, and `codex login status`; secrets for the first two are read from stdin. [CLI reference](https://learn.chatgpt.com/docs/cli/reference#codex-login)

`CODEX_HOME` selects the root for configuration, auth, logs, sessions and other state; its directory must already exist. It applies to CLI, IDE extension and app-server. [Environment variables](https://learn.chatgpt.com/docs/config-file/environment-variables)

**Recommended flow:** create a protected temporary `CODEX_HOME`, invoke installed Codex with file-backed storage, wait for successful login, inspect the resulting cache, and save it as a named profile. The account currently active in the default home stays intact. Show the actual resulting account before activation, because the browser may already be signed in to another account. Clean up canceled attempts. Device-code login remains a fallback and may require enabling it in personal security or workspace settings. [Authentication](https://learn.chatgpt.com/docs/auth#preferred-device-code-authentication-beta)

For simultaneous CLI accounts, an explicit launcher can select distinct permanent homes. This also separates configuration and local history, so it changes more than credentials. Codex's `--profile` selects a named configuration layer; it is not documented as a separate account-token store. [Advanced configuration](https://learn.chatgpt.com/docs/config-file/config-advanced#profiles)

## Structured login through a controlled app-server

The app-server offers `account/login/start`, cancellation, completion notifications, account metadata and rate-limit reads. Managed `chatgpt` login handles browser OAuth, persistence and refresh; `chatgptDeviceCode` returns a verification link and code. The experimental external-token mode instead requires the host to own renewal. Current guidance allows local/open-source app-server authentication and recommends Sign in with ChatGPT; commercial or hosted services require a different supported route. [App-server authentication](https://learn.chatgpt.com/docs/app-server#auth-endpoints)

A Keyturn-owned isolated app-server could improve the login experience. Its RPC methods act on that process. They do not establish a supported mechanism for remotely changing the separate desktop app's running login. Version-gate this integration against the installed CLI.

## API and organization-specific alternatives

Platform API keys represent API access and billing, not an interchangeable ChatGPT subscription. Offer them as a clearly labeled optional account type if needed. They are the official default for many automated Codex workflows. [Non-interactive authentication](https://learn.chatgpt.com/docs/non-interactive-mode#use-api-key-auth)

Codex access tokens are available in Business and Enterprise workspaces for trusted non-interactive local workflows. They depend on workspace permissions and token lifetime; `CODEX_ACCESS_TOKEN` can supply ephemeral access, and `codex login --with-access-token` persists a credential. Service-account tokens represent the service account rather than their creator. These are automation alternatives, not a promise of personal desktop sign-in support. [Access tokens](https://learn.chatgpt.com/docs/enterprise/access-tokens), [Service accounts](https://learn.chatgpt.com/docs/enterprise/service-accounts)

Workload identity federation exchanges OIDC or SPIFFE identity for short-lived access. Codex maps to a managed ChatGPT principal; it is a beta feature requiring workspace enablement. Its environment-selected identity takes precedence over stored logins, so a file switch cannot override it. [Workload identity federation](https://developers.openai.com/api/docs/guides/workload-identity-federation)

## New Sign in with ChatGPT profiles are a different integration

Current developer guidance supports separate ChatGPT registrations and an account picker for OSS/local apps. Each registration keeps its own client ID and tokens, including when emails match. Refreshes must be serialized and replacement tokens preserved. [Accounts and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)

The official cookbook exposes `listProfiles()`, `selectProfile()` and `signIn({ newProfile: true })`. It maintains an app-owned connection record with verified identity and granted permissions. [OSS integration cookbook](https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt)

Its documented Codex integration passes a plan-authorized access token to a custom Responses provider and a child app-server. Renewal requires restarting that child with the new token and resuming the thread. This is not documented as writing those credentials into the first-party desktop `auth.json`. [Sign in with ChatGPT app-server integration](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)

This route is suitable if Keyturn later owns an AI experience. It adds an OAuth lifecycle and current Responses limitations, rather than directly solving the user's desktop account switch. [Preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)

## What can be promised now

- **File import:** a supplied supported Codex cache can become a named profile without changing the active account.
- **File activation:** Keyturn can perform the replacement the user currently performs manually, for file-backed storage.
- **Desktop next launch:** the user's close/replace/reopen setup is a target workflow; validate against the installed app before calling it universally supported.
- **Running desktop / existing CLI sessions:** no guarantee of immediate account change or cancellation of existing work. Avoid automatic force-quit; an eventual explicit restart action must check active work.
- **Account validity:** local structure and display identity cannot prove server authorization, workspace membership or token renewal.
- **No perpetual cache promise:** revoked, expired or concurrently rotated credentials may require reimport or a new sign-in.

These statements describe the feasible scope and remaining verification boundaries. They do not substitute for the implementation task's tests or native-panel verification.

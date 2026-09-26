# Naming research: a menu-bar name for the key/quotas app

Research date: 2026-09-20. Candidate set for a short, one-word macOS menu-bar
name for the tool formerly called `agent-key-switcher` (now Keyturn).

What a good name has to do here:

- stay readable next to a menu-bar icon and as a Homebrew cask token
- be one word, 4-10 characters, no spaces
- evoke keys / switching / quota / agents / AI without being generic
- survive as a GitHub repo, npm package, `.app` name and (ideally) a domain

## How the evidence was gathered

- **GitHub**: `github.com/search?q=<name>&type=repositories` (counts and top
  repos below come from the equivalent `GET /search/repositories` call), plus a
  check of `github.com/<name>` to see whether a user/org already owns the bare
  handle.
- **npm**: `https://registry.npmjs.org/<name>` — HTTP `404` means the name is
  free, `200` means it is taken.
- **Homebrew**: `https://formulae.brew.sh/api/cask/<token>.json` — `404` means
  no cask with that token exists.
- **Apple / Tauri / Homebrew rules**: developer.apple.com, v2.tauri.app,
  docs.brew.sh (see the constraints section).

Anything I could not verify is marked **unverified** rather than guessed.

## 1. Ranked shortlist

| # | Name | Chars | What it evokes | GitHub | npm | macOS app clash | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `keytide` | 7 | key + tide; quota windows ebb and flow, a key that "tides you over" until reset | bare handle **404**; 2 repos, top ★0 | free (404) | none found (near-miss `Keyty`) | **Top pick** |
| 2 | `keycycle` | 8 | cycling/rotating through keys as windows roll over | bare handle **404**; 7 repos, top ★40 (unrelated) | free (404) | none found | Strong |
| 3 | `keyslot` | 7 | numbered slots each holding a key; switch the active slot | bare handle taken (small user, 14 repos); 20 repos, top ★27 (unrelated) | free (404) | none found | Strong |
| 4 | `keyturn` | 7 | turning to the next key; an inverted "turnkey" | bare handle taken (dormant, 0 repos); 16 repos, top ★10 (unrelated) | free (404) | none found | Strong |
| 5 | `sparekey` | 8 | the spare key you fall back to — exactly the auto-switch story | bare handle taken; 13 repos, max ★11 | free (404) | none found | Good (8 chars, compound) |
| 6 | `keyvolt` | 7 | voltage/charge; battery-style quota meter | bare handle taken (dormant); 3 repos, top ★0 | free (404) | none found | Good, soft company clash |
| 7 | `keywatt` | 7 | watts/power; battery meter | bare handle taken (dormant, 0 repos); **0** repos in name | free (404) | none found | Good, soft hardware clash |
| 8 | `keyvane` | 7 | a weathervane pointing at the healthy key | bare handle taken (0 repos); **0** repos in name | free (404) | none found | OK, meaning is subtle |
| 9 | `keylode` | 7 | "lode" = a vein/rich source of keys | bare handle **404**; **0** repos in name | free (404) | none found | Clean, weak meaning |
| 10 | `keydune` | 7 | rolling dunes/waves (quota windows) | bare handle **404**; **0** repos in name | free (404) | none found | Clean, weak key meaning |
| 11 | `kwota` | 5 | Polish for "quota" — short and literal | bare handle taken; 39 repos, top ★10 | free (404) | none found (near-miss `KEW Smart`) | Quirky; pronunciation unclear |
| 12 | `keymo` | 5 | key + monitor/momentum; the shortest option | bare handle taken; 496 repos (mostly `Keymouse*`) | free (404) | **Keymo/Keymou** (Many Tricks) + **KEYMO** (BYSTAMP) | **Reject** |

## 2. Per-candidate detail

### 1. keytide — top pick

- Evokes: the tide is the quota window — it rises, falls and resets. A key that
  "tides you over" until the rolling window refills.
- GitHub: `https://github.com/keytide` → **404** (handle free). Search
  `https://github.com/search?q=keytide&type=repositories` → 2 hits, both ★0:
  `remypicciano/keytide`, `acekavi/keytide`.
- npm: `https://registry.npmjs.org/keytide` → **404** (free).
- Homebrew: `https://formulae.brew.sh/api/cask/keytide.json` → **404** (free).
- macOS app: no macOS/iOS app found. Nearest neighbour is **Keyty**
  (`keyty.app`, a keyboard/mouse visualiser, cask `keyty`) — phonetically
  similar but a different word and a different product.
- Domains: `keytide.com` is registered; `whois` reports `keytide.io` and
  `keytide.sh` as **"Domain not found"** (i.e. free); `keytide.app` / `keytide.dev`
  **unverified** (whois returned nothing usable).
- Verdict: clean on every axis checked, and the meaning maps to the actual
  feature (rolling quota windows). This is the best combination of clean +
  on-message.

### 2. keycycle

- Evokes: cycling the active key through accounts/profiles as each hits a limit.
- GitHub: `https://github.com/keycycle` → **404** (handle free). Search
  `https://github.com/search?q=keycycle&type=repositories` → 7 hits, top
  `xyTom/KeyCycler` ★40 (a keyboard chording tool, unrelated).
- npm: **404** (free). Homebrew: **404** (free).
- macOS app: none found.
- Domain: `keycycle.com` registered; `keycycle.app` **unverified**.
- Verdict: strong and descriptive; slightly longer (8).

### 3. keyslot

- Evokes: a rack of slots, each holding a key; you switch the live slot.
- GitHub: `https://github.com/keyslot` → taken (user `keyslot`, 14 repos,
  1 follower). Search `https://github.com/search?q=keyslot&type=repositories`
  → 20 hits, top `billynothingelse/XvddKeyslotUtil` ★27 (a Redis tool,
  unrelated).
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Verdict: strong meaning; the bare GitHub handle being taken is the only wart.

### 4. keyturn

- Evokes: turning a key onward to the next one; a play on "turnkey".
- GitHub: `https://github.com/keyturn` → taken but dormant (user `keyturn`,
  0 public repos, created 2012). Search → 16 hits, top
  `DanMillerDev/GearVRControllerKeyturn` ★10 (unrelated).
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Verdict: strong; the dormant handle is a minor annoyance.

### 5. sparekey

- Evokes: the spare key you reach for when the primary is locked out — a
  literal description of auto-switch-on-rate-limit.
- GitHub: `https://github.com/sparekey` → taken. Search
  `https://github.com/search?q=sparekey&type=repositories` → 13 hits, max ★11
  (`kalekundert/sparekeys`).
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Trademark: **SpareKey** is also a US non-profit mortgage-assistance programme
  — different sector, soft clash.
- Verdict: thematically excellent, but 8 characters and a common two-word
  compound make it weaker as a brand.

### 6. keyvolt

- Evokes: voltage/charge — matches the battery-style quota icon.
- GitHub: `https://github.com/keyvolt` → taken (dormant, 0 repos). Search → 3
  hits, all ★0 landing pages (`keyvolt-landing`, `keyvolt-terraform`,
  `keyvolt.com.ua`).
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Company clash: **KeyVolt** (`keyvolt.us`) sells software-licence fulfilment
  for BigCommerce; another **Keyvolt** (`keyvolt.com.ua`) is a Ukrainian solar
  firm. Different domains, so soft.
- Verdict: good name, minor unrelated-company noise.

### 7. keywatt

- Evokes: watts/power for the battery/percentage menu-bar meter.
- GitHub: `https://github.com/keywatt` → taken (dormant, 0 repos, created
  2020). Search → **0** repos with the name.
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Company clash: **KEYWATT** is an EV-charging brand by IES Synergy
  (`help.e-flux.io` manual). Hardware, different sector — soft.
- Verdict: good; cleanest of the "power" names but has an existing hardware mark.

### 8. keyvane

- Evokes: a weathervane swinging to point at whichever key is healthy.
- GitHub: `https://github.com/keyvane` → taken (user `Keyvane`, 0 repos,
  created 2025). Search → **0** repos with the name.
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Verdict: clean, but "vane" is an uncommon word and the connection is oblique.

### 9. keylode

- Evokes: a "lode" is a rich vein of ore — a motherlode of keys.
- GitHub: `https://github.com/keylode` → **404** (free); 0 repos in name.
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Verdict: technically the cleanest (free everywhere) but the meaning is weak
  for an app about quota windows.

### 10. keydune

- Evokes: rolling dunes/waves — gesturing at rolling quota windows.
- GitHub: `https://github.com/keydune` → **404** (free); 0 repos in name.
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Verdict: clean, but "dune" says "sand", not "keys" or "limits".

### 11. kwota

- Evokes: Polish for "quota" — the most literal option, and short.
- GitHub: `https://github.com/kwota` → taken. Search
  `https://github.com/search?q=kwota&type=repositories` → 39 hits, mostly
  Polish projects (top `kx1000/KwotaSlownie` ★10).
- npm: **404**. Homebrew: **404**. macOS app: none found.
- Verdict: charming but ambiguous — non-Polish speakers will not guess the
  pronunciation or the meaning.

### 12. keymo — rejected

- Evokes: key + monitor/momentum; the shortest candidate at 5 characters.
- GitHub: `https://github.com/keymo` → taken; search → 496 hits (dominated by
  `Keymouse*` projects).
- npm: **404**.
- macOS clash (hard enough to reject): **Keymo** by Many Tricks was a macOS
  menu-bar mouse-control utility; it was renamed to **Keymou** because
  "Keymo" sounded like "chemo"
  (`https://manytricks.com/keymou/releasenotes`). Separately, **KEYMO** by
  BYSTAMP is an App Store e-signature app (`https://apps.apple.com/us/app/keymo/id1533823438`).
- Verdict: reject — an existing macOS menu-bar app plus a documented
  pronunciation problem.

## 3. Hard collisions (rejected)

These names were dropped outright, with the collision that killed them.

| Name | Collision | Evidence |
| --- | --- | --- |
| `keyflux` | **keyflux.io** — "KeyFlux REMIT is our just-in-time auth and verification layer for AI agents… Replaces long-lived deploy keys". Same space (AI-agent credentials/keys). | `https://www.keyflux.io/`, `https://www.keyflux.io/blog` |
| `keysmith` | An AI-coding key tool already: `Jia-Ethan/codex-keysmith` ★4,550 and `Jia-Ethan/claude-keysmith` ★796. Plus a macOS app **Keysmith** ("Create custom keyboard shortcuts") with cask token `keysmith`. | `https://github.com/search?q=keysmith&type=repositories`, `https://formulae.brew.sh/cask/keysmith`, `https://www.keysmith.app/` |
| `switchkey` | `itsuhane/SwitchKey` ★961 is a macOS **menu-bar** utility for switching input sources — same form factor, same "switch" verb. | `https://github.com/itsuhane/SwitchKey` |
| `keyring` | The generic term for a credential store: `jaraco/keyring` ★1,512, `99designs/keyring` ★658; npm taken; also PAM keyrings / macOS Keychain territory. | `https://github.com/search?q=keyring&type=repositories`, `https://registry.npmjs.org/keyring` |
| `keymo` | See §2.12 — macOS menu-bar app + App Store app. | `https://manytricks.com/keymou/releasenotes` |

Other names checked and set aside before deep verification because their
collision profile was already too heavy: `keyring` (above), `keyline` (137
GitHub hits incl. `keyline-icons` ★91; bare `github.com/keyline` taken;
`keylines.net`), `keybar` (Keybar keyboard app), `barkeep` (a code-review
product + 246 GitHub hits), `quorum` (2,617 hits incl. Consensys Quorum ★4,764),
`kestrel` (Microsoft's Kestrel web server; npm taken), `rota` (74,839 GitHub
hits), `relay`, `ratchet`, `keydrop` (KeyDrop CS:GO skin site), `keyspan`
(historical Tripp Lite brand), `keyport` (hardware key organiser).

## 4. Naming constraints: Apple and Tauri

### Apple

- **App Store name limit is 30 characters.** Apple: "An app name can be up to
  30 characters long," and "Choose a simple, memorable name that is easy to
  spell and hints at what your app does. Be distinctive. Avoid names that use
  generic terms or are too similar to existing app names."
  Source: https://developer.apple.com/app-store/product-page/ (§App name).
- **Short names are explicitly preferred.** In the Human Interface Guidelines
  for the menu bar, under the app menu's About item: "Prefer a short name of 16
  characters or fewer. Don't include a version number." The same page says
  "Prefer short, one-word menu titles," and, for menu bar extras, "Consider
  using a symbol to represent your menu bar extra."
  Source: https://developer.apple.com/design/human-interface-guidelines/the-menu-bar
- **The bundle name that macOS reads is itself capped at 16 characters.**
  `CFBundleName` "specifies the short name of the bundle… This name should be
  less than 16 characters long." This is the key Apple-imposed length ceiling a
  menu-bar app name must respect.
  Source: https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html
  (§CFBundleName; §CFBundleDisplayName; §CFBundleIdentifier).
- **Note on "no spaces":** Apple does *not* forbid spaces in
  `CFBundleDisplayName` or App Store names (many apps have spaces). The
  no-spaces rule here is our own, chosen so the name reads cleanly beside a
  menu-bar icon and maps one-to-one onto a Homebrew cask token. Apple's real
  hard rules are the character sets: `CFBundleIdentifier` may contain "only
  alphanumeric (`A-Z`, `a-z`, `0-9`), hyphen (`-`), and period (`.`)"
  characters and should be reverse-DNS.

### Homebrew cask token

- The token is the lowercased app name with spaces/underscores/middots turned
  into hyphens and everything non-alphanumeric deleted (multiple hyphens
  collapsed, leading/trailing hyphens removed). A single lowercase word like
  `keytide` therefore yields the token `keytide` exactly.
  Source: https://docs.brew.sh/Cask-Cookbook (§Converting the simplified name
  to a token).

### Tauri v2

- **`productName`** is the app name "known by on the user's system." On macOS
  it "Names the `.app` bundle and the `.dmg`, and sets the bundle's
  `CFBundleDisplayName` and `CFBundleName`." Its allowed pattern is
  `^[^/\:*?"<>|]+$` — no slashes, colons, quotes or the Windows-reserved
  characters. So `productName: "Keytide"` produces `Keytide.app` and the DMG
  filename `Keytide_<version>_<arch>.dmg` (matching the current cask URL shape
  `keyturn_#{version}_universal.dmg`).
  Source: https://v2.tauri.app/reference/config/#productname
- **`identifier`** (the bundle id) "must be in reverse domain name notation
  (e.g. `com.tauri.example`)… must contain only alphanumeric characters (`A-Z`,
  `a-z`, and `0-9`), hyphens (`-`), and periods (`.`)" and must be unique; the
  default `com.tauri.dev` is rejected by `tauri build`. Note this matches
  Apple's `CFBundleIdentifier` charset exactly. A name with any other character
  (spaces, underscores, `&`) would force an unrelated identifier.
  Source: https://v2.tauri.app/reference/config/#identifier

Taken together: a one-word, lowercase, alphanumeric name is the only shape that
keeps `productName`, the `.app`/`.dmg` names, the cask token, the bundle id and
(all else equal) the domain all saying the same thing.

## 5. Recommendation (top 3)

1. **Keytide** — best overall. It is the only candidate that is free on GitHub
   (bare handle 404), npm, and the Homebrew cask index simultaneously, with no
   product or company collision. The meaning is the feature: the rolling 5-hour
   / weekly / monthly window is a tide, and a fresh key "tides you over" until
   it resets. It is 7 characters, one word, pronounceable, and under Apple's
   16-character `CFBundleName` ceiling.
   - cask token: `keytide` → `brew install --cask 7skylab/tap/keytide`
   - `productName`: `Keytide` → `Keytide.app`, `Keytide_0.2.0_universal.dmg`
   - identifier: `com.7skylab.keytide` (reverse-DNS, alphanumeric + periods only)
   - domains: `keytide.io` and `keytide.sh` reported free by `whois`; `.app`
     still to verify.
2. **Keycycle** — the most self-explanatory name (you cycle keys as each hits
   its limit), clean on npm/Homebrew and free as a GitHub handle. Slightly
   longer at 8 characters; `keycycle.com` is taken so the domain story is
   weaker than Keytide's.
3. **Keyslot** — the best "menu-bar" feel: a row of slots, one lit. Clean on
   npm and Homebrew; its only drawback is that the bare GitHub handle is taken
   by a small personal account, so the repo would need the `7SkyLab` org to
   carry it (which it already does).

If a single criterion decides it: **Keytide best survives the whole chain** —
`Keytide` → `Keytide.app` → cask token `keytide` → identifier
`com.7skylab.keytide` → `keytide.sh`, with no step requiring a spelling,
spacing or character change and no existing tool or company in the way.

### Names to drop

Do not ship `keymo` (macOS menu-bar app + pronunciation problem), `keyflux`
(an AI-agent credential company), `keysmith` (existing AI-key tooling plus a
macOS app), `switchkey` (an existing macOS menu-bar switcher) or `keyring`
(the generic credential-store term).

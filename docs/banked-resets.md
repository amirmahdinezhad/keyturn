# Account banked resets

Codex account cards read `rate_limit_reset_credits` from the existing live
`GET /backend-api/wham/usage` request. Missing/malformed metadata stays unknown;
it is never a synthetic zero. Public reset announcements do not prove an
account has a reset.

Usage reads `GET /backend-api/wham/rate-limit-reset-credits` using the selected
saved profile's OAuth login and ChatGPT workspace header. No account activation
is needed. Eligibility respects `applicable_available_count` when returned,
`is_supported_by_plan`, expiry and the covered main allowances. Unknown plans
or reset types stay visible but cannot be redeemed in Keyturn. Separate model
limits are not included in a full reset's coverage.

The native review step rechecks the inventory and pins the profile identity,
credit, plan, covered windows and expiration to a ten-minute review token.
Confirmation rechecks these details before sending any mutation. Only the
explicit Use 1 reset action calls
`POST /backend-api/wham/rate-limit-reset-credits/consume`.

Before POST, Keyturn atomically saves an owner-only redemption record outside
the repository in its application-data directory. It contains a hashed account
identity, credit metadata and `redeem_request_id`, never credentials. Unknown
results block a different credit for that account. Reopening Usage can recover
the same request ID; retries never silently create another redemption. Only
recognized server outcomes settle an attempt. Usage percentages and refresh
times are fetched again, not predicted locally.

The UI extends the existing 430×600 tray surface: system font, 13px section
labels, 11px metadata, a 4/8/12/16px spacing rhythm and existing theme tokens
(background #191b1f, surface #23262b, text #f1f3f5, secondary #a8b0bd,
border #383e47, action #0070eb). Reset review replaces Usage content in the
same modal and retains account identity, back navigation and focus handling.
Account rows publish a complete quota batch after matching skeletons. Usage
shows one shared loading state for windows and resets, then a compact card
with the server title, relative expiration and 28px Use reset action. Zero
inventory renders no reset section; ineligible actions remain disabled without
a reason paragraph. Unknown requests still show an error with Retry.

## Contract references

These endpoints are used by OpenAI's official client, rather than a guaranteed
stable third-party API:

- https://github.com/openai/codex/blob/main/codex-rs/backend-client/src/client/rate_limit_resets.rs
- https://github.com/openai/codex/blob/main/codex-rs/backend-client/src/client/rate_limit_resets_tests.rs
- https://help.openai.com/en/articles/20001498-how-resets-work-in-codex

Development verification reads live balances only. Redemption tests use
synthetic responses and injected transport/storage failures; spending a real
reset requires the owner's separate approval.

# 096-B1 stage 14 — the live run against a real upstream: a runbook

> ## The upstream is Duende's public demo, not Google (decided 2026-11-15)
> `@nabbisen`: *"Authorized. Take the way."*, on the architect's proposal to
> run against `https://demo.duendesoftware.com` instead of Google.
>
> Why the change: Google was the architect's default, picked from the
> configuration reference's example. Google needs the owner to create a
> project, a consent screen, a test user and an OAuth client. The demo needs
> nothing: its client and test users are pre-registered and published.
> Duende IdentityServer is a certified OpenID Provider (OpenID Foundation).
>
> **What it gives up:** the claim shapes Google itself sends. The record says
> *"a certified public OpenID Provider"*, never *"tested with Google"*.
>
> **RFC 096 asks for *"owner-supplied test credentials"*** (`:1088-1089`).
> The demo's credentials are public, not supplied by the owner. The owner
> authorized this run with them, on that note. The wording was there to keep
> secrets out of artifacts, and a demo with public credentials has none.
>
> ## Its slot: `0.91.0`'s prepared commit, **`a0a6c1a`** (2026-11-15)
> Deferred from `0.90.0` on 2026-11-14 (`@nabbisen`: *"Authorized. Defer it.
> However, it should be controlled properly in the schedule."*). `0.91.0`'s tag
> waits for this run's record, unless a further deferral is recorded. M4-B
> cannot close, and M6 cannot enter, without it (ROADMAP, cycle L's row).

**What it proves.** RFC 096 `:1088-1089` asks for *"one representative
public upstream integration … outside CI, with secrets absent from
artifacts."* This is the first time sui-id checks a provider against a real
upstream, enables it, and signs someone in through it. Every run so far used
mocks.

**What it does not prove.**
- *"A live happy path does not substitute for hostile-provider evidence"*
  (`:1093-1094`). That is 096-A's corpus, and 096-C's.
- Any one large provider's own quirks, Google's or Microsoft's.

## 0 — safety

- **A throwaway instance only**: its own directory, database and key file,
  bound to `127.0.0.1:8801`, and deleted afterwards. **Never point a real
  sui-id at the demo.** Its client credentials are public, so anyone can
  obtain tokens for its users under that client.
- **Nothing of the owner's goes to the demo.** The client ID
  (`interactive.confidential`), the secret (`secret`) and the test user
  (`alice`/`alice`) are all published by Duende. The demo sees an ordinary
  browser visit.
- **Who signs in at the demo: the owner.** The architect does not type a
  password on a third party's sign-in page.

## 1 — the commit

`0.91.0`'s preparation landed as `461443c`, and a test-only fix after it as
**`a0a6c1a7d5922b61f2519dfdc54d527a7a0357ad`** (2026-11-15). That is the
commit to run. The architect builds it, detached, in a clean sibling clone:

```sh
git clone . ../sui-id-stage14
git -C ../sui-id-stage14 checkout --detach a0a6c1a7d5922b61f2519dfdc54d527a7a0357ad
cargo build --release -p sui-id --locked --manifest-path ../sui-id-stage14/Cargo.toml
../sui-id-stage14/target/release/sui-id --version     # must print: sui-id 0.91.0
```

## 2 — the upstream

Nothing to create. From the demo, measured 2026-11-15:
- **Discovery:** issuer `https://demo.duendesoftware.com`, with every
  endpoint on that one origin;
- **Signing and auth:** RS256; `client_secret_basic`; PKCE S256, which this
  client requires and sui-id sends;
- **Redirect URI:** `http://localhost:8801/auth/federated/callback` is
  accepted. A probe of the authorize endpoint went to its login page, not to
  an error;
- **Test users:** its login page says *"use either `bob`/`bob` or
  `alice`/`alice`"*.

## 3 — the throwaway configuration (the architect's)

In `../sui-id-live/`, from `--print-sample-config`, with:
- `issuer = "http://localhost:8801"`, `listen_addr = "127.0.0.1:8801"`,
  `cookie_secure = false`;
- `db_path` and `key_file` inside that directory;
- `federation_providers = []` deleted, and this block appended:

```toml
[[federation_providers]]
slug              = "duende-demo"
display_name      = "Duende demo"
issuer            = "https://demo.duendesoftware.com"
client_id         = "interactive.confidential"
client_secret_env = "DUENDE_DEMO_SECRET"
scopes            = "openid email profile"
provision_mode    = "provision_on_first_login"
```

Then `sui-id preview-upgrade --config sui-id.toml`. **Expected:** no
configuration error.

## 4 — start it (the architect's)

```sh
DUENDE_DEMO_SECRET=secret ../sui-id-stage14/target/release/sui-id --config sui-id.toml > server.log 2>&1
```

The value is Duende's published demo secret, not a credential of anyone's.

## 5 — the steps, and who does each

**The owner** clicks in the browser, including both sign-ins at the demo. The
architect does the clicks instead if the Claude in Chrome extension is
connected. **The architect** reads `server.log`, and checks each step against
the database: the provider row, the audit events, and the federation link. So
every step has a second, independent record.

| # | Do | Expected |
|---|---|---|
| 1 | The startup log | one `WARN` naming the provider and saying to visit `/admin/federation-providers` and press "Check and enable" |
| 2 | Open the setup URL the architect gives you, and create the admin | the admin console |
| 3 | **Admin → Federation providers** | one row: Duende demo, Disabled, Requires review, "—" for the check column, "No" linked users |
| 4 | **Check only** | "Every check passed." |
| 5 | **Check and enable** | "The provider was checked and enabled.", and the row Enabled, Validated, with a time |
| 6 | Sign out. Open the login page | a **"Sign in with Duende demo"** button |
| 7 | Press it, and sign in at the demo as `alice` / `alice` | back at sui-id, signed in as a new user, on that user's own security page (`/me/security`) |
| 8 | Sign in as the admin. **Users →** that user | a *Federated sign-in* section with Duende demo |
| 9 | **Unlink**. Read the confirm page, then confirm | the page says the unlink cannot be undone, and that open sessions stay valid until the account is disabled |
| 10 | Sign out, and press "Sign in with Duende demo" again, as `alice` | refused on the login page with "This sign-in could not be linked to a local account automatically. Contact an administrator." It is the same message whether or not a local account exists, by design |
| 11 | **Federation providers → Disable** | "The provider was disabled."; the login button is gone |
| 12 | The architect stops sui-id, removes the block, and starts it again. Then **Delete** | the confirm page states how many users' sign-in ends; then the row is gone |

## 6 — the record

The architect writes `.git-exclude/live-runs/096-b1-stage14-<date>.md`:
- the commit, and `sui-id --version`;
- the table, PASS or FAIL per step, with the page's exact text for a FAIL;
- the database check for each step;
- `server.log`'s `WARN` and `ERROR` lines, with any URL containing `code=`
  or `state=`, and any token or cookie, removed.

Then: fixes, if the run finds any, and the gates again. **Nothing from the
run goes into git** except the architect's summary.

## 7 — afterwards

Stop sui-id, and delete `../sui-id-live/` and `../sui-id-stage14/`.

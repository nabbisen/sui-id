# M4-B — the account-creation run against Google: the record

**Runbook.** [`096-b1-m4b-google-run-2026-11-21.md`](096-b1-m4b-google-run-2026-11-21.md).
**Who.**
- `@nabbisen`: the Google setup, every click, both sign-ins at Google.
- The architect: the build, the configuration, the server, and a check of
  every step against the server log and the database.

**Personal data.** None here. Redacted:
- the Google account's email;
- the client ID;
- the provisioned user's name and subject.

The client secret was never shown to anyone. It lived in a mode-600 file,
read into the environment by the start command, and was deleted with
the instance afterwards.

**Result.** **Steps 1–12 passed on `67f1140`, with no patch.** Steps 7–10
are what M4-B waited for:
- account creation from Google's ID token;
- the user page;
- unlink;
- the takeover refusal.

The record closes M4-B's live-integration evidence. Under RFC 096's closure rule, M4-B itself closes when
the 096-B1 completion assessment is accepted.

## The first run, 2026-11-21, on `643aa47`

| # | Result | Evidence |
|---|---|---|
| 1 | PASS | the startup `WARN` naming the provider and *Check and enable* |
| 2–3 | PASS | the admin was created; one Google row, disabled, *Requires review* |
| 4, first try | refused, by design | *"token_endpoint's origin "https://oauth2.googleapis.com" is not in the allowed set"*. The runbook's config had left out the four `allowed_origins` the configuration reference lists for Google. **The architect's omission**, fixed in the runbook |
| 4, second try | **FAIL: product defect** | `federation DNS answer rejected name=www.googleapis.com count=16`. Google's signing-key host answers with 8 A + 8 AAAA records; the resolver's cap was 8. **As built, sui-id could not federate with Google at all** |
| 4–12 | PASS, **on a patched build** | `643aa47` with only the cap raised to 32 (diff hash `8391d8cc4d4624c1…`), on the owner's choice to continue and repeat on the fix. Step 7: `auth.federation.provisioned`; a new non-admin user, its email from the verified ID-token claim; one `federation_link`. Step 10: `auth.federation.takeover_blocked`, `reason=email_collision` |

What it found went into **O6**
([dispatch](0.93.0-o6-what-the-google-run-found-2026-11-21.md), landed
`67f1140`):
- **the DNS cap** raised to 32 in total. RFC 096's resolver paragraph is
  amended; RFC 134's D2 carries a note. `@nabbisen`: *"32 in total
  (Recommended)"*;
- **a log line with a full URL**, which RFC 096 forbids;
- **refusal pages that said only "check the server log"**: they now name
  the cause;
- **confirm pages whose consequence was muted text.** This covers two of
  the owner's observations:
  - the delete page's count was hard to see. `@nabbisen`: *"The number
    was not easy to distinguish. It is very important information and
    should be impacted."*;
  - the unlink page's sessions warning was missed. Asked whether he saw
    it, he answered: *"No, I missed it."*

## The record run, 2026-11-22, on `67f1140`

CI run `44953415607`, 28/28 jobs. A clean sibling clone at `67f1140`,
built `--release --locked`; `sui-id version` printed `sui-id 0.92.0`,
since the version is bumped at the preparation. A fresh database and key.

| # | Result | Evidence |
|---|---|---|
| 1 | PASS | one `WARN`: provider "google" requires review … *Check and enable* |
| 2 | PASS | `admin.setup.completed` 11:09:44 |
| 3 | PASS | one Google row, as expected |
| 4 | PASS | *"Every check passed."* **No DNS rejection and no JWKS failure** in the log: the answer that failed on `643aa47` passes |
| 5 | PASS | `federation.provider.enabled` 11:10:20; enabled, validated |
| 6 | PASS | *"You are signed out."* and a *Sign in with Google* button |
| 7 | **PASS: account creation, live** | at `/me/security/overview` (where `/me/security` redirects), as a new user, account menu only. `auth.federation.provisioned` 11:11:50. Users 1 → 2: role `user`, not admin, email present and normalized, `email_verified_at` empty (by design, RFC 101 D1); one `federation_link` |
| 8 | PASS | the user page's *Federated sign-in* section names Google |
| 9 | PASS | the O6 unlink page. `@nabbisen`: *"Both of the blocks were easy enough to find and read."* `federation.link.deleted` 11:18:20; links 1 → 0 |
| 10 | PASS | refused: *"This sign-in could not be linked to a local account automatically. Contact an administrator."* `auth.federation.takeover_blocked`, `reason=email_collision`, 11:19:00; still 2 users, 0 links |
| 11 | PASS | `federation.provider.disabled` 11:21:10; the button gone |
| 12 | PASS | block removed and restarted; the startup `WARN` says the provider has no block. The O6 delete page: *"No user signs in through this provider."* `@nabbisen`: *"Yes. Clear."* `federation.provider.deleted` 11:22:39 |

One request had no explanation. In the first run, at 12:39:21, the server
received a callback with no `code`, 16 s before the provider was first
enabled. The owner did not make it. It was refused, as it should be, and
changed nothing. A browser prefetching a remembered `localhost:8801` URL,
which stage 14 also used, is a guess, not a finding.

## What the record run found, and where it went

- **The unlink page misstated its consequence for a federation-only
  user.** After step 9 the provisioned user had no password and no
  passkey, so it could not sign in at all. The page had said *"their
  account and their other sign-in methods remain"*. The owner's note,
  *"the user, create via the federation, remains"*, led to measuring this.
  Fixed in **O7** (`0.93.0`):
  - the page names what remains;
  - when nothing remains, it says so and offers *Disable*, *Delete* and,
    after unlinking, *Issue a recovery link*. `@nabbisen`: *"Offer Disable
    and Delete (Recommended)"*.

  Keeping the account is by design: an unlink removes a way to sign in,
  not the account.
- **The footer is noise on every page.** `@nabbisen`: *"there is
  improvement around common footer which always shows static sentences
  and color theme selector. They can be noise."* Carried into RFC 149, for
  cycle P ([its list](../149-the-console-follows-the-operator/README.md)).
  He chose *"Cycle P, with the chrome redesign (Recommended)"*.

## Afterwards

- The instance was deleted, with its database, key, logs and the
  secret's file.
- The build clone `../sui-id-stage14` is kept; it holds no secret.
- **The owner deletes the Google OAuth client.** The architect cannot.

# RFC 003 — HIBP scope expansion (post-v0.24.0)

**Status.** Proposed — **priority elevated by v0.29.3 codebase
review**. Originally framed as a "medium-term, ship-when-ready"
expansion; the review confirmed that the gap is not just feature
breadth but a *consistency bug*: with `hibp_mode = block` the
admin gets blocked at setup but a self-service password change
or a forgot-password redemption sails through. That's a policy
hole, not just a missing feature. Treat this RFC as high-priority
implementation work, sequenced behind RFC 010 and RFC 011.

**Tracks.** ROADMAP / Medium term — "HIBP scope expansion
(post-v0.24.0)". v0.29.3 codebase review — high-priority finding #3.
**Touches.** `sui-id-core` (`me_security`, `forgot_password`, admin
flows), `sui-id-store` (potentially a new `password_breach_fingerprint`
table for re-check), `sui-id-web` (admin settings UI for `hibp_mode`).

## Summary

v0.24.0 wired the HIBP "Pwned Passwords" check into the setup wizard.
That covers exactly one entry point: the password set during initial
admin creation. Every other point at which a sui-id password gets set
or reset still bypasses the check. This RFC extends the existing
`HibpClient` trait + `enforce_hibp` policy + `HibpMode` enum to all
remaining entry points, plus adds the admin UI for the mode setting,
plus designs (but defers) the periodic re-check feature.

The infrastructure already exists. This is mostly mechanical wiring,
with one design discussion around how to do periodic re-check
without storing anything compromising.

## Requirements

After this RFC ships:

1. Every server-side password-set entry point runs the same
   `enforce_hibp(mode, plaintext)` check that the setup wizard runs
   today. Specifically: self-service password change, admin-driven
   password reset, forgot-password redemption, and (existence
   notwithstanding) any future password-creation surfaces.
2. The `hibp_mode` setting (`off` / `warn` / `block`) is editable
   through the admin UI under a clear label, with a one-paragraph
   description explaining the trade-offs.
3. The behaviour of `block` mode is consistent: a user who tries
   to set a known-breached password gets the same error wording
   regardless of which entry point they're on.
4. Failure of the HIBP service itself is fail-open in every mode.
   A network blip on hibp.org must never prevent a legitimate
   password reset.
5. The re-check feature is designed (this RFC) but optional to
   implement (deferred to a follow-up release).

## Design

### Wiring the existing check into remaining entry points

The function exists today:

```rust
// sui-id-core/src/hibp.rs
pub fn enforce_hibp(
    client: &dyn HibpClient,
    mode: HibpMode,
    password: &str,
) -> CoreResult<()>;
```

Returns `Ok(())` on `off`, on `warn` regardless of breach, on
`block` if not breached, and on any HIBP transport failure
(fail-open). Returns `Err(CoreError::PasswordBreached)` only on
`block` + breached + HIBP responded.

Three call sites get this added, immediately before the
`credentials::upsert` they perform today:

#### `me_security::password_change_post`

```rust
// after current password verification
hibp::enforce_hibp(
    &*app.hibp_client,
    server_settings::current(&app.db)?.hibp_mode,
    &form.new_password,
)?;
credentials::upsert(...)?;
```

The `warn` mode renders an additional flash on the next page,
text via `sui-id-i18n` (`password_breach_warn_flash`).

#### `admin::users_password_reset`

Same pattern. The operator-supplied password is checked. In
`block` mode the form returns to the admin user with an inline
error; the target user's password is not changed.

There's a question of authority here: does the admin's reset
get to override `block`? Recommendation: **no.** The check is
about the *target user's* password being safe, not about who
set it. An admin who wants to set a known-bad password (e.g.
for a known-stub account they intend to disable) should change
the global mode first — that action is auditable, the override
is not.

#### `forgot_password::consume_and_reset_password`

Same pattern. The token-based reset path checks the new
password. `block` mode rejects with a generic "this password
appears in known data breaches; please choose another"
message; the token *is not consumed* on rejection so the user
can immediately try again with a different password.

### Admin settings UI

`/admin/settings/security` gains a new card:

```
[ ] Off          — Don't check passwords against the HIBP database.
[ ] Warn (default) — Allow breached passwords but show a
                     warning to the user.
[*] Block        — Refuse to set a known-breached password.
```

Plus a help paragraph explaining k-anonymity (we send only the
first 5 chars of the SHA-1 hash; HIBP cannot reconstruct the
password) and the fail-open posture. Mode change is recorded as
audit event `admin.settings.hibp_mode.changed` with old/new
values in the `note` field.

Bilingual strings live in `sui-id-i18n`:
`settings_security_hibp_*`.

### Periodic re-check (designed, deferred)

The interesting case. We don't store plaintext passwords, so we
can't re-run the check on a schedule against everyone. What we
*can* do is store the SHA-1 prefix (the part already sent to
HIBP at password-set time) and notify the user on their *next
sign-in* if the prefix has since been added to the breach list.

#### Privacy stance

This is a fingerprint, not a credential. SHA-1 of a password is
not reversible to the password (even with rainbow tables, a
12-character minimum policy puts most candidates outside
practical reach), and the *prefix* is even less reversible.
However:

- The fingerprint table is a target. If an attacker exfiltrates
  it along with the rest of the DB, they get a slightly faster
  brute-force trail per user. Not a meaningful uplift over the
  Argon2id-hashed `credentials.password_hash` they already have,
  but a non-zero one.
- Storing the *full* SHA-1 hash (not just the prefix) would make
  the check exact rather than probabilistic, but at materially
  worse privacy. We use the prefix only.

Conclusion: store only the prefix, AAD-bind to the
fingerprint table, treat as encrypted-at-rest like every other
sealed column.

#### Schema

Migration `0020_password_breach_fingerprint.sql`:

```sql
CREATE TABLE password_breach_fingerprint (
    user_id           TEXT PRIMARY KEY,
    prefix_enc        BLOB NOT NULL,           -- 5-char SHA-1 prefix, AAD-bound
    set_at            TIMESTAMP NOT NULL,
    last_checked_at   TIMESTAMP,
    last_check_result TEXT                     -- 'clean' | 'breached' | NULL
                      CHECK (last_check_result IN ('clean','breached')),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);
```

Populated at password-set time, in the same transaction as the
`credentials::upsert`. AAD: `password_breach_fingerprint.prefix`.

#### Re-check trigger

On sign-in success (after MFA, before issuing the session),
the auth handler:

1. Loads `password_breach_fingerprint` row for the user.
2. If `last_checked_at` is older than e.g. 7 days, fires off a
   single HIBP query against the prefix.
3. If the response includes the user's full hash suffix (which
   we'd need to also store, see open question below), updates
   `last_check_result = 'breached'` and sets a flash on the
   first authenticated page: "the password you're currently
   using has appeared in a public data breach; please change it."

#### Open question for re-check

The check at *password-set time* sends the prefix and compares
the response's suffixes against the suffix the server briefly
held. We never persist the suffix. To do an asynchronous
re-check, we'd need to either (a) re-compute the prefix-and-
suffix at sign-in time (which we *can* do — we have the
plaintext at sign-in), or (b) store the suffix.

Strongly prefer (a): on sign-in we have the plaintext for one
millisecond anyway, so we can compute SHA-1 and run the
prefix-only HIBP check inline, no storage needed. Drops the
schema requirement for `password_breach_fingerprint` entirely.

If (a) is the chosen approach, the table goes away and this
sub-feature collapses to: "on every sign-in, optionally
re-check the password against HIBP." The throttle (once per N
days per user) lives in a small in-memory cache or a single-
column annotation on `users` — we don't need a whole new table.

Recommend revisiting this in the implementation pass.

### Configuration

No config-file changes. `hibp_mode` is already a server-settings
column. The admin UI is the only new surface.

## Multiple implementation steps

1. **Wire existing check into the three remaining handlers.**
   Mechanical. Three new calls to `enforce_hibp`. Three new
   tests. Audit events stay the same shape (the existing
   `auth.password.breach_blocked` covers all entry points).
2. **Admin settings UI for `hibp_mode`.** New card on
   `/admin/settings/security`, new audit event for changes,
   new i18n keys.
3. **Periodic re-check.** Recommend approach (a) — plaintext
   at sign-in, no new schema. Throttle column on `users`,
   notification flash, opt-out toggle on `/me/security`.

Each step ships independently.

## Tests

- **Self-service change.** Pre-load `InMemoryHibpClient` with
  a breach for the new password, set `hibp_mode = 'block'`,
  POST password-change, assert form rejects and credentials
  unchanged.
- **Admin reset.** Same shape, but the actor is the admin and
  the target is another user.
- **Forgot-password redemption.** Same shape, against a fresh
  reset token. Assert token is *not* consumed on rejection.
- **HIBP unreachable in `block`.** Stub the client to return
  `Err(CoreError::Hibp(_))`. Assert all three handlers
  succeed (fail-open).
- **Mode change audit.** POST a new mode through the admin
  UI, assert the audit row records the transition.
- **Cross-locale flash.** Set `preferred_lang='en'`, attempt a
  blocked change, assert the flash text is the English string,
  not Japanese.
- For the re-check sub-feature when implemented: **throttle
  test.** Force two sign-ins inside the throttle window;
  assert HIBP was called only on the first.

## Security considerations

- **k-anonymity preserved.** All three new call sites use the
  existing `HibpClient::is_breached` shape, which already only
  ever sends 5-char prefixes. The Add-Padding header is set by
  the existing client implementation.
- **Fail-open is unchanged.** The existing policy stays. The
  rationale (a HIBP outage must not become a sui-id outage)
  is reinforced by every new entry point inheriting it.
- **No plaintext leakage in audit.** `auth.password.breach_blocked`
  records `user_id`, `mode`, and the literal string `"breached"`
  — never the password, never the prefix.
- **Admin override.** As above: no inline override of `block`
  for admin reset. Mode change is the only knob, and it's
  audited.
- **Re-check storage** (when implemented per option (a)):
  nothing new is stored. The `last_checked_at` annotation on
  `users` carries no derivable secret.

## Open questions

- The "warn" mode flash wording on `/forgot-password`
  redemption: do we show it to a user who's *already* setting
  a new password after losing the old one? Recommend: yes; it
  matches user expectation that a password reset is a sensible
  moment to mention this.
- Re-check option (a) versus (b) — confirm at implementation
  time. Strong lean toward (a).

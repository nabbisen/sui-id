# RFC 118 implementation handoff

**Governing RFC.** [RFC 118](../../proposed/118-lockout-clears-on-credential-change.md), **Proposed**.
Nothing here is authorized until it is Accepted.
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Scheduled.** Release cycle B, **first item**, ahead of RFC 116 stages 1–2.

## The measurements this RFC rests on

Re-run before starting; a disagreement is a blocker.

| Claim | How |
|---|---|
| The backoff reaches 24 h at ten failures and stays | `lockout_backoff`, `crates/sui-id-core/src/authn/session.rs` |
| An active lock refuses sign-in before the password is checked | same file, the `locked_until > now` branch |
| **U10 clears neither the counter nor the lock** | read `consume_and_reset_password` in `commands.rs` |
| **U09 clears neither either** | read `change_password_self` in the same file |
| The only reset is a successful password verify | `clear_lockout`'s call sites |

## One stage

Small enough that splitting it would cost more in review than it saves.

**D1.** U09 and U10 clear `failed_login_count` and `locked_until` inside the
transaction that writes the credential. **Do not add a second path** — if the
two commands end up with their own copies of the clearing logic, they will
drift, which is the defect this RFC exists to remove one level up.

**D3.** The completion flow tells a user whose account is locked that it is,
and when it lifts. i18n in en, ja and zh-Hans, as every user-facing string is.

**D4.** The sign-in form is untouched.

## Evidence

- **The defect, before the fix:** an account is locked by failed attempts, its
  password is then reset through a valid link, and sign-in with the new
  password is still refused. Write this test first and show it failing.
- U09 clears the lock; U10 clears the lock; each with a mutation that removes
  the clearing and is caught.
- **The lock cannot outlive the credential** — the clearing is in the same
  transaction, shown by an injected failure before the append: the credential
  and the cleared lock roll back together. The store's fault injector does
  this; RFC 102's U09/U10 tests are the pattern.
- **D4's boundary, which is the part to get right:** the sign-in path's
  response for a locked account is byte-identical to a wrong password, and
  identical to what it was before this change. The new message is reachable
  only through a completion that presented a valid token. **State how you
  tested that it is not reachable otherwise**, rather than asserting it.
- `docs/threat-model.md`: RFC 115's second residual is the one this closes.
  Update it in the same package — do not leave the threat model claiming a
  residual that no longer exists.

## What to return

A review-request package under `.git-exclude/review-requests/`, in the usual
form: what was built, the evidence table, mutations with results, gates on the
final tree, and per-hunk SHA-256 hashes against the stated baseline.

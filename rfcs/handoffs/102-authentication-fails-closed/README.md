# RFC 102 implementation handoff

**Governing RFC.** [RFC 102](../../accepted/102-authentication-fails-closed-without-audit.md),
Accepted 2026-09-17. The RFC is the contract; this file sets the order and the
evidence. **Implementer.** Mid-capability model.
**Reviews.** [design review](102-103-design-review-2026-09-16.md),
[confirmation review](102-103-confirmation-review-2026-09-17.md); its §2.4 lists
the `*_within_tx` variants each command needs.

## Order

Each stage is one review request. Stages 1–4 are dispatched in order. Stage 1 is
dispatched **now**.

| Stage | Content | Why here |
|---|---|---|
| **1** | **B7** (factor additions gated; first-factor re-authentication), **L06** (per-session step-up failure count, revocation at 5), the **step-up rate-limit bucket**, **B3** (recovery codes refused for step-up), `auth.mfa.factor_added` | Live defect: a stolen session can add a factor and pass step-up. Owner-authorized ahead of the rest, 2026-09-17 |
| 2 | **L01** with A7 (password path); A9 | R11's path |
| 3 | **L02** and **L07** (per-user second-factor count), N14's sign-in WebAuthn script | the replay races |
| 4 | **L03**, **L04** | external-source and federated paths |
| 5 | **A4** — `sessions::insert` crate-private; U30 retired | after every sign-in path is converted |
| 6 | **L05** — step-up success atomic; WebAuthn ceremony kind swap removed | |
| 7 | **B4** on the four sealed gated commands, including `not_applicable`; **B6** | |
| 8 | matrix classes and events; threat model; handoff closure | |

## Stage 1 — dispatched 2026-09-17

**Baseline.** The commit that adds this file, or later.

**Scope.**
- **Migration.** Add `sessions.step_up_failure_count INTEGER NOT NULL DEFAULT 0`
  and `sessions.last_step_up_method TEXT` (nullable). Stage 6 uses the second
  column, and adding both now avoids a second migration of `sessions`.
- **L06** as a sealed Class-A command. Increment the count; at 5, revoke that one
  session. Branch events are `auth.step_up.failure` (count) or
  `auth.step_up.session_revoked` (count). It needs `sessions::revoke_within_tx`
  for a single row.
- **The step-up rate-limit bucket.** Add `RateLimitKey::StepUp`. It applies to
  `POST /me/security/step-up`, `step-up/webauthn/start`, `step-up/webauthn/finish`,
  and every B7 re-authentication form.
- **Wire the failures.** A wrong code on any step-up path runs L06. A9 holds: a
  store error on the success path does **not** run L06. Keep the existing
  responses (RFC 102's failure table, step-up rows).
- **B3.** `verify_totp_code` stops accepting recovery codes, and the doc comment
  at `step_up.rs:89-90` is corrected (first-review L1).
- **B7 gates.** When the user has any second factor,
  `passkey_register_start`/`complete`, `mfa_regenerate_recovery` and
  `mfa_enroll_start`/`confirm` require `require_fresh_step_up`. When the user has
  none:
  - a local user re-enters the password on the form; a wrong password runs L06
    and uses the step-up bucket;
  - an LDAP user re-binds against the user source, and a failure counts the same;
  - a federated user re-authenticates upstream with `prompt=login` and
    `max_age=0`, and the returned `auth_time` must be later than the enrolment
    request. **If this needs federation plumbing you cannot add without touching
    RFC 096-B1's files, stop and report.** Until then, refuse first-factor
    enrolment for federated users, and say so in the UI.
  - If re-authentication is unavailable, enrolment is refused.
- **`auth.mfa.factor_added`** (method) is a Class-A event with the enrolment write
  for all three factor kinds. If a factor's enrolment write is not yet on the
  seam, convert that write in this stage (U12, U14 or U15 per the inventory) and
  name it.
- **Registry and matrix.** Register the new events in `registry.rs` and
  `ci/audit-coverage-matrix.md`. G13 must pass.

**Evidence.**
- **Stolen-session tests** (RFC 102 test plan, B7): a second client holding the
  cookie cannot register a passkey, regenerate codes or enrol TOTP without a
  fresh step-up. A local user with no factor cannot enrol without the password.
- **L06 tests.** Four failures then a success: the count resets and the session
  is kept. Five failures: the session is revoked and the next request is
  unauthenticated. The bucket refuses a burst. A store error on L05's
  predecessor path runs no L06 (A9).
- **N1 test.** Wrong passwords on the enrolment form are counted, and the fifth
  revokes the session.
- **B3.** A valid recovery code on the step-up form is refused, not consumed, and
  freshness is unchanged.
- **Mutation.** Remove each gate and each count in turn; each removal is caught.
- **Build and gates.** fmt, both clippy scopes, `cargo test --workspace` count
  before and after, MSRV 1.95, G13, G12 (new UI strings in en, ja and zh_hans).

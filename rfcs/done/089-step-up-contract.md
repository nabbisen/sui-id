# RFC 089 — Step-up Authentication Contract

**Status.** Implemented (v0.71.0)
**Tracks.** UI/UX handoff v2.3 §6 — unit 3. Category A.
**Touches.** `sui-id/src/handlers/step_up.rs` (`sanitise_return_to` →
allowlist gate), `sui-id-core/src/step_up.rs` (recovery-codes exclusion
assertion), `sui-id-web/src/pages/auth/step_up.rs` (passkey-first ordering),
i18n (no new keys required).

## Summary

Tighten the step-up authentication contract to the v2.3 §6 specification:
add a server-side allowlist to `sanitise_return_to` so that `?return_to`
can only redirect to explicitly approved step-up-gated paths; assert that
recovery codes are excluded from step-up factor eligibility; and render the
step-up page with passkey offered before TOTP when both are enrolled.

## Motivation

The existing `sanitise_return_to` performs format checks (relative URL,
no protocol-relative prefix, no backslash/newline/NUL) but does not
validate that the destination is a step-up-gated route. A crafted
`?return_to=/admin/settings/security/idle-timeout` could redirect a
user directly to a high-risk settings POST form after step-up without
the intended confirm page. The contract requires same-origin **and**
allowlisted.

Recovery codes are already not wired to the step-up challenge flow in
the implementation, but that is incidental; the contract should be made
explicit so future additions cannot accidentally accept them.

Passkey-first ordering on the step-up form is a UX invariant: phishing-
resistant factors are preferred and offered first. Currently the form
offers password/TOTP first and passkey as a secondary link.

## Background

From §6 of the v2.3 handoff:
> "The `next` parameter … must be a same-origin, relative URL … **and**
> match a server-side allowlist of step-up-gated confirm/self-service
> routes."
>
> "Recovery codes: **Not accepted for step-up** — recovery codes are for
> account recovery, not routine re-authentication."
>
> "When an account has more than one eligible factor, the **passkey is
> preferred and offered first** because it is phishing-resistant."

## Target code areas

1. **`handlers/step_up.rs` — `sanitise_return_to`**: after the existing
   format checks, reject any path not in `STEP_UP_RETURN_ALLOWLIST`. Return
   the default fallback (`/me/security`) for non-listed paths.

2. **`sui-id-core/src/step_up.rs` — `policy_for_session`**: add an
   assertion/doc comment that the recovery-codes path is not an eligible
   step-up factor. The actual policy already works correctly (TOTP and
   WebAuthn are the two eligible paths); the assertion is documentation
   hardening.

3. **`sui-id-web/src/pages/auth/step_up.rs`** (or the equivalent render
   function): reorder the challenge UI so that if the user has a WebAuthn
   credential, the passkey option is rendered first (above the
   password/TOTP field), not as a secondary link below.

## Security properties / invariants

- **P1 (allowlist).** `return_to` after a successful step-up can only
  navigate to a path in `STEP_UP_RETURN_ALLOWLIST`. Any unrecognised path
  falls back to `/me/security`. An attacker who can forge a link containing
  an arbitrary `?return_to=` cannot redirect the user to an arbitrary
  admin page post-step-up.
- **P2 (recovery-code exclusion).** Recovery codes are not a valid step-up
  factor in any code path. This is structural (the step-up handler only
  accepts TOTP and WebAuthn challenges) plus a doc-comment assertion.
- **P3 (passkey-first).** When the user has a WebAuthn credential enrolled,
  the passkey option is rendered before the TOTP/password option on the
  step-up page.
- **P4 (5-minute reusable window, unchanged).** The existing 300-second
  freshness window and the "reuse within the window" rule are confirmed
  correct per the contract and remain unchanged.
- **P5 (no auto-execute).** After step-up succeeds, the user is redirected
  to the confirm page. The confirm page requires an explicit POST from the
  user. This is already enforced by the existing flow; this RFC documents
  it as a named invariant.

## Non-goals

- No change to the freshness window value (300 s is correct).
- No change to which operations require step-up (that list is stable;
  see §6 of the handoff).
- No change to the TOTP / WebAuthn verification logic.
- The `return_to` parameter name is kept as-is (not renamed to `next`);
  the contract uses `next` as a concept, but the wire parameter has been
  `return_to` since v0.21.0 and changing it would break bookmarked links.

## Proposed design

```rust
/// Step-up-gated routes that may appear in a `?return_to=` parameter.
/// Patterns use prefix matching — a listed prefix authorises any suffix
/// (e.g. `"/admin/users/"` covers `/admin/users/{id}/delete-confirm`).
const STEP_UP_RETURN_ALLOWLIST: &[&str] = &[
    "/admin/users/",
    "/admin/clients/",
    "/admin/signing-keys/",
    "/admin/settings/",
    "/me/security/",
];

fn sanitise_return_to(raw: &str) -> String {
    // Existing format checks …
    // NEW: allowlist check
    let in_allowlist = STEP_UP_RETURN_ALLOWLIST
        .iter()
        .any(|prefix| raw.starts_with(prefix));
    if !in_allowlist {
        return "/me/security".to_owned();
    }
    raw.to_owned()
}
```

The allowlist uses prefix matching so it covers any sub-path under a
step-up-gated section without enumerating every confirm-page URL. The
alternative (exact-match) would require updating the allowlist for every
new confirm page, which is maintenance-brittle. Prefix matching is safe
here because every prefix listed is under admin auth (`/admin/*`) or
self-service auth (`/me/security/*`) — both are already session-gated.

## Data model impact

None.

## API impact

None. The step-up endpoint URL and query parameter name are unchanged.
The only observable change: `?return_to=/admin/dashboard` (not in any
allowlist prefix) now falls back to `/me/security` instead of navigating
to the dashboard. This is a security fix, not a regression.

## Testing strategy

- Unit: `sanitise_return_to("/admin/users/abc/delete-confirm")` → same value.
- Unit: `sanitise_return_to("/admin/dashboard")` → `"/me/security"`.
- Unit: `sanitise_return_to("https://evil.example/")` → `"/me/security"`.
- Unit: `sanitise_return_to("//evil.example")` → `"/me/security"` (existing
  test, confirm still passes).
- Unit: `sanitise_return_to("")` → `"/me/security"` (existing).
- Visual: step-up page with WebAuthn credential enrolled → passkey option
  appears above the TOTP/password input.

## Migration strategy

No data migration. The allowlist is additive logic in a pure function; all
existing step-up flows that use correct `return_to` values continue to work.

## Rollout plan

Ships as v0.71.0 (same release as RFC 089 implementation). No phased
rollout required — the allowlist is a security tightening with no
breaking change for well-formed clients.

## Risks and mitigations

- *Risk:* a legitimate step-up `return_to` path is not in the allowlist,
  causing unexpected fallback to `/me/security`. Mitigation: the allowlist
  uses broad prefixes (`/admin/users/`, `/admin/settings/`, etc.) covering
  all currently known step-up-gated routes. A missing path would be caught
  in QA by testing the sensitive flow end-to-end.
- *Risk:* passkey-first rendering breaks for users with no WebAuthn
  credential. Mitigation: the reorder is conditional on `user_has_webauthn`;
  users without a passkey see the unchanged TOTP/password form.

## Acceptance criteria

- `sanitise_return_to` rejects any path not covered by the allowlist.
- Recovery-code exclusion is documented in `step_up::policy_for_session`.
- Step-up page with WebAuthn credential enrolled renders passkey CTA before
  TOTP/password field.
- All existing step-up-gated confirm flows (delete user, delete client,
  signing-key delete, high-risk settings) still work end-to-end.
- 0 warnings; baseline test suite green.

## Open questions

- Should `/admin/signing-keys/rotate-confirm` be a separate allowlist entry
  (exact match) rather than covered by the signing-keys prefix? The rotate-
  confirm route is step-up-gated and is the only route under that prefix
  that users reach via step-up. Using prefix is consistent; exact match is
  more restrictive. Decision at implementation: keep prefix.

# RFC 010 — Revoke sessions and refresh tokens on forgot-password completion

**Status.** Proposed
**Priority.** Highest. Security-critical bug fix.
**Tracks.** v0.29.3 codebase review — high-priority finding #1.
**Touches.** `sui-id-core::forgot_password`, plus a regression test in
`sui-id/tests/e2e/email_forgot.rs`.

## Summary

A user who completes a forgot-password reset has, by definition,
lost control of the credentials they had before — typically because
those credentials were compromised. The reset flow today
substitutes the password and consumes the reset token, but it
*does not invalidate the user's existing sessions or refresh
tokens*. An attacker who already has a stolen session cookie or
refresh token continues to hold valid access after the legitimate
user has reset their password.

This RFC closes that gap by extending `consume_and_reset_password()`
to revoke all of the target user's sessions and refresh tokens in
the same transaction as the credential update. It also adds a
regression test so the gap doesn't reappear.

The fix is local. There is no new public API, no schema change,
no UI change. The user-visible effect is exactly the one
expected by anyone reading "you have been signed out everywhere"
in their own bank's password-reset flow.

## Why this is the highest-priority item

Two reasons it sits above everything else in the review backlog:

- **Severity.** Account-recovery flows exist precisely so a user
  who has lost control can regain it. A recovery flow that
  doesn't revoke prior access is half-built — the user thinks
  they've recovered the account, but the attacker still has it.
- **Inconsistency with siblings.** Admin-driven password reset
  *does* revoke (`admin::password_reset` calls
  `sessions::revoke_all_for_user` and `refresh_tokens::revoke_all_for_user`).
  Self-service password change *does* revoke. Only the email-
  link reset path doesn't. This isn't a design decision; it's
  a copy-omission. Restoring symmetry takes a few lines.

## Design

### Scope of the change

```rust
// crates/sui-id-core/src/forgot_password.rs

pub fn consume_and_reset_password(
    db: &Database,
    clock: &SharedClock,
    hibp_client: &dyn HibpClient,
    hibp_mode: HibpMode,
    token_plaintext: &str,
    new_password: &str,
) -> CoreResult<()> {
    let now = clock.now();
    let row = repos::password_reset_tokens::find_by_hash(db, &hash(token_plaintext), now)?
        .ok_or(CoreError::BadRequest("invalid or expired reset token".into()))?;

    // (existing) HIBP enforcement at the chosen mode.
    hibp::enforce_hibp(hibp_client, hibp_mode, new_password)?;

    let new_hash = password::hash(new_password)?;
    db.transaction(|tx| {
        // (existing) update credentials.
        repos::credentials::upsert_within_tx(tx, row.user_id, &new_hash, now)?;

        // (existing) consume the token.
        repos::password_reset_tokens::consume_within_tx(tx, row.id, now)?;

        // NEW: revoke all sessions and refresh tokens for the user.
        repos::sessions::revoke_all_for_user_within_tx(tx, row.user_id, now)?;
        repos::refresh_tokens::revoke_all_for_user_within_tx(tx, row.user_id, now)?;

        Ok(())
    })?;

    // (existing) audit + notification email.
    audit::append(db, &AuditLogRow {
        action: "auth.password.reset_via_email".into(),
        actor: Some(row.user_id),
        target: Some(row.user_id.to_string()),
        result: "ok".into(),
        ..audit_defaults(now)
    })?;
    Ok(())
}
```

### Audit signal

The existing `auth.password.reset_via_email` audit event already
fires on a successful reset. Two existing events do double-duty
on the revoke:

- `auth.session.revoke_all` — emitted by
  `sessions::revoke_all_for_user_within_tx` for each batch.
- `auth.refresh.revoke_all` — emitted by
  `refresh_tokens::revoke_all_for_user_within_tx`.

Both are already present and emit-on-write. No new event is
introduced. Operators reading the audit log see the reset event
followed immediately by the two revoke events, in the same
transaction's wall-clock instant.

### Transactionality

All three writes — credential upsert, token consume, revoke —
land in the same SQLite transaction. Either everything happens
or nothing does. The existing `db.transaction(|tx| …)` boundary
expands to include the revoke calls. No new locking primitive.

The `_within_tx` variants of the repo functions either already
exist or are trivial wrappers; if they don't exist for sessions
and refresh_tokens, that's a one-line addition each (call the
existing function with the `tx` instead of `db`).

### Off-by-one: user not signed in elsewhere

If the user has no active sessions or refresh tokens at reset
time, the revoke calls are no-ops and return `Ok(())`. This is
the common case (a user who's locked out probably wasn't signed
in anywhere) and needs no special handling.

### Concurrent reset attempts

The token consume + revoke happen atomically. A concurrent
second reset attempt with the same token sees the consumed token
and rejects with the existing `BadRequest` path. No race window
for "first attempt revokes, second attempt creates a new
session before revoke completes."

## Tests

A new regression test in `tests/e2e/email_forgot.rs`:

```rust
#[tokio::test]
async fn forgot_password_reset_revokes_all_sessions_and_refresh_tokens() {
    let (state, _mailer) = test_app_with_mailer();
    enable_smtp(&state).await;

    // Bootstrap: complete setup, sign in twice (two sessions),
    // exchange one for a refresh token.
    let session_a = complete_setup_and_login(&state).await;
    let session_b = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    let (_client_id, _secret) = create_client(&state, &session_a).await;
    // Authorize + token exchange to get a refresh token. Helper TBD
    // but sketched as `obtain_refresh_token_for(&state, &session_a)`.

    // Sanity: two sessions and one refresh-token row, all active.
    assert_eq!(active_session_count(&state, alice_id()), 2);
    assert_eq!(active_refresh_count(&state, alice_id()), 1);

    // Issue a forgot-password token, redeem it with a fresh password.
    let token = issue_forgot_password_token(&state, "alice@example.test").await;
    redeem_forgot_password(&state, &token, "alice-the-new-password").await;

    // Expectation: zero active sessions, zero active refresh tokens.
    assert_eq!(active_session_count(&state, alice_id()), 0);
    assert_eq!(active_refresh_count(&state, alice_id()), 0);

    // The old session cookies must now reject:
    let resp = build_router(state.clone())
        .oneshot(req_admin_get_with_cookie(&session_a))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);  // -> /admin/login
}
```

Three additional micro-tests in `crates/sui-id-core/src/forgot_password.rs`'s
`#[cfg(test)] mod tests`:

- `reset_revokes_zero_sessions_when_user_has_none` — no-op path.
- `reset_revokes_when_user_has_only_sessions_no_refresh` — partial.
- `reset_is_atomic_on_revoke_failure` — inject a synthetic
  failure in `revoke_all_for_user_within_tx`, assert the
  credential is unchanged and the token is unconsumed (the
  whole transaction rolls back).

The third test is the most important: it pins the
transactionality contract. Without it, a future refactor could
land the credential update outside the transaction with the
revoke and reintroduce a window.

## Security considerations

This RFC's change reduces attack surface, not adds it. The
relevant considerations are about *not regressing*:

- **The reset token itself remains single-use.** Revoking
  sessions doesn't affect the token's consume state — the
  token is consumed in the same transaction, so a replay
  hits the consumed token and the revoke doesn't run a
  second time.
- **The notification email continues to fire.** An attacker
  who somehow times a reset against a stolen email account
  will trigger a "your password was changed" email to
  whatever address sui-id has on file (which is usually the
  same address — but the user might have a forwarding rule).
  Behaviour unchanged from today.
- **Audit log integrity.** Revoke events join the same hash
  chain as the reset event. No new chain, no parallel store.

There is no plausible scenario where this change *worsens* an
existing security property. The only risk is implementation
error in the transaction boundary — covered by the atomicity
test above.

## Open questions

None. The fix is unambiguous; the test plan is clear; the
audit semantics fall out of existing events.

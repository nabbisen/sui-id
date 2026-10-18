use super::*;
use crate::time::MockClock;
use chrono::{Duration as ChronoDuration, Utc};
use sui_id_shared::ids::{FederationLoginAttemptId, FederationProviderId, UserId};
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{FederationProviderRow, ProvisionMode, Role, UserRow, UserSource};
use sui_id_store::repos::{federation_link, federation_login_attempt, federation_provider, users};

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn fixed_now() -> chrono::DateTime<Utc> {
    "2026-10-14T12:00:00Z".parse().unwrap()
}

fn clock_at(t: chrono::DateTime<Utc>) -> SharedClock {
    std::sync::Arc::new(MockClock::at(t))
}

fn digest32(raw: &str) -> [u8; 32] {
    Sha256::digest(raw.as_bytes()).into()
}

async fn seed_provider(db: &Database) -> FederationProviderId {
    let id = FederationProviderId::new();
    let row = FederationProviderRow {
        id,
        slug: format!("p-{id}"),
        display_name: "Provider".to_owned(),
        issuer: "https://idp.example".to_owned(),
        client_id: "client".to_owned(),
        client_secret_enc: None,
        scopes: "openid email".to_owned(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

async fn seed_user(db: &Database) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    let row = UserRow {
        id,
        username: id.to_string(),
        display_name: None,
        is_admin: false,
        role: Role::User,
        is_disabled: false,
        is_deleted: false,
        user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
        email: None,
        preferred_lang: None,
        email_normalized: None,
        email_verified_at: None,
        last_login_at: None,
        source: UserSource::Local,
        external_stable_id: None,
    };
    users::create(db, &row).await.unwrap();
    id
}

async fn seed_link(db: &Database, user_id: UserId, provider_id: FederationProviderId) {
    let now = Utc::now();
    federation_link::upsert(
        db,
        sui_id_store::models::FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: "fixture-sub".to_owned(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
}

async fn an_exchanging_attempt(
    db: &Database,
    provider_id: FederationProviderId,
    tag: u8,
) -> FederationLoginAttemptId {
    let now = Utc::now();
    let row = federation_login_attempt::insert(
        db,
        provider_id,
        0,
        0,
        [tag; 32],
        [tag.wrapping_add(1); 32],
        [tag.wrapping_add(2); 32],
        b"verifier",
        "https://example.com/cb".to_owned(),
        None,
        now,
    )
    .await
    .unwrap();
    federation_login_attempt::claim(
        db,
        row.id,
        now,
        provider_id,
        0,
        0,
        [tag.wrapping_add(2); 32],
    )
    .await
    .unwrap()
    .id
}

/// Seeds a real, confirmed TOTP enrolment (through `mfa::start_enrollment`/
/// `confirm_enrollment`, not hand-rolled) and returns the plaintext
/// recovery codes `confirm_enrollment` hands back once.
async fn seed_totp_enabled(db: &Database, clock: &SharedClock, user_id: UserId) -> Vec<String> {
    let ticket = crate::mfa::start_enrollment(db, "https://issuer.example", user_id, "user")
        .await
        .unwrap();
    let step = clock.now().timestamp().div_euclid(30);
    let code = totp::code_for_step(&ticket.secret, step).await;
    crate::mfa::confirm_enrollment(db, clock, user_id, code)
        .await
        .unwrap()
}

/// Full setup: provider, user, link, an exchanging attempt, a federated
/// MFA continuation row (through F02 itself, not hand-sealed), and a
/// confirmed TOTP enrolment with recovery codes. Returns everything a
/// test needs to call [`verify`] against a real row.
struct Fixture {
    db: Database,
    clock: SharedClock,
    user_id: UserId,
    pending_id: FederationMfaPendingId,
    browser_binding_raw: String,
    csrf_raw: String,
    recovery_codes: Vec<String>,
}

async fn fixture() -> Fixture {
    let db = open_test_db();
    let clock = clock_at(fixed_now());
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 10).await;
    let browser_binding_raw = "browser-binding-raw".to_owned();
    let csrf_raw = "csrf-raw".to_owned();
    let (pending_id, _) = sui_id_store::commands::issue_federated_mfa_pending(
        &db,
        attempt_id,
        provider_id,
        0,
        0,
        user_id,
        None,
        digest32(&browser_binding_raw),
        digest32(&csrf_raw),
        fixed_now(),
    )
    .await
    .unwrap();
    let recovery_codes = seed_totp_enabled(&db, &clock, user_id).await;
    Fixture {
        db,
        clock,
        user_id,
        pending_id,
        browser_binding_raw,
        csrf_raw,
        recovery_codes,
    }
}

async fn row_snapshot(
    db: &Database,
    id: FederationMfaPendingId,
) -> (
    i64,
    sui_id_store::models::FederationMfaPendingStatus,
    Option<Vec<u8>>,
) {
    let row = federation_mfa_pending::find_by_id(db, id)
        .await
        .unwrap()
        .expect("row exists");
    (row.failure_count, row.status, row.continuation_sealed)
}

/// Raw SQL, deliberately bypassing this crate's own write paths — there
/// is no F03 yet (stage 10b's job); this is the shape F03's own
/// terminalizing `UPDATE` will take; migration 0047's table `CHECK`
/// (":733-734 terminal rows retain no usable continuation") is what
/// forces `continuation_sealed` to `NULL` here, the same precedent
/// `tests_rfc096_b1_stage8.rs`'s own terminalizing `UPDATE` already
/// used against the live table.
async fn terminalize(db: &Database, id: FederationMfaPendingId, status: &str) {
    let id_str = id.to_string();
    let status = status.to_owned();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE federation_mfa_pending SET status = ?1, continuation_sealed = NULL \
                 WHERE id = ?2",
            [status, id_str],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

/// [`verify`]'s own error, pattern-matched down to the one value
/// [`BindingFailure`] should be rather than compared as a whole `Result`
/// (stage 10a fix round, R7: a hand-written `PartialEq` on
/// `FederatedMfaOutcome` existed only to make exactly this comparison
/// possible, and gave the candidate-bearing outcome type a capability
/// it should not have). Panics — rather than silently miscomparing — if
/// the result was `Ok` or a [`VerifyError::Internal`], which is the
/// point: a caller that expected a binding refusal and got something
/// else should see that distinction, not have it erased by an
/// equality check that could not tell the difference either.
///
/// Generic over the `Ok` type (RFC 096-B1 stage 11): [`verify`]/
/// [`verify_webauthn`] return `FederatedMfaOutcome`,
/// [`begin_webauthn_ceremony`] returns a challenge `String`, and
/// [`promote`]/[`promote_webauthn`] return
/// `sui_id_store::commands::F03Outcome` — all four share the exact
/// same [`VerifyError`], and this helper's job is entirely about that
/// shared error type, not any one of their differing success types.
fn as_binding_failure<T: std::fmt::Debug>(result: Result<T, VerifyError>) -> BindingFailure {
    match result {
        Err(VerifyError::Binding(b)) => b,
        other => panic!("expected a binding failure, got {other:?}"),
    }
}

// ── Happy paths ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_correct_totp_code_is_a_valid_candidate() {
    let f = fixture().await;
    // One step (30s) after enrolment's own confirmation step -- enrolling
    // already consumed that step as `last_used_step`, so submitting the
    // same step again here would be a replay, not a fresh proof. Still
    // well inside the row's own 300-second window.
    let verify_clock = clock_at(fixed_now() + ChronoDuration::seconds(30));
    let step = verify_clock.now().timestamp().div_euclid(30);
    // Re-derive the code the same way a real submission would: decrypt
    // the secret back out rather than keep a second copy from enrolment.
    let totp_row = user_totp::get(&f.db, f.user_id).await.unwrap().unwrap();
    let secret = user_totp::decrypt_secret(&f.db, &totp_row).await.unwrap();
    let code = totp::code_for_step(&secret, step).await;

    let outcome = verify(
        &f.db,
        &verify_clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &code.to_string(),
    )
    .await
    .expect("binding succeeds");
    assert!(matches!(outcome, FederatedMfaOutcome::Totp(_)));
}

// ── Stage 10b: `promote`, the seam-side caller of F03 ───────────────────

#[tokio::test]
async fn promote_with_a_correct_totp_code_promotes_through_f03() {
    let f = fixture().await;
    let verify_clock = clock_at(fixed_now() + ChronoDuration::seconds(30));
    let step = verify_clock.now().timestamp().div_euclid(30);
    let totp_row = user_totp::get(&f.db, f.user_id).await.unwrap().unwrap();
    let secret = user_totp::decrypt_secret(&f.db, &totp_row).await.unwrap();
    let code = totp::code_for_step(&secret, step).await;

    let outcome = promote(
        &f.db,
        &verify_clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &code.to_string(),
    )
    .await
    .expect("verify and F03 both succeed");
    assert!(matches!(
        outcome,
        sui_id_store::commands::F03Outcome::Promoted { .. }
    ));
}

#[tokio::test]
async fn promote_with_a_wrong_totp_code_is_rejected_still_pending_through_f03() {
    let f = fixture().await;
    let outcome = promote(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "000000",
    )
    .await
    .expect("verify succeeds; F03 counts the wrong proof");
    match outcome {
        sui_id_store::commands::F03Outcome::RejectedStillPending { failure_count, .. } => {
            assert_eq!(failure_count, 1)
        }
        other => panic!("expected RejectedStillPending(1), got {other:?}"),
    }
}

#[tokio::test]
async fn promote_on_an_unknown_pending_id_is_a_binding_failure() {
    let f = fixture().await;
    let result = promote(
        &f.db,
        &f.clock,
        &FederationMfaPendingId::new().to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;
    match result {
        Err(VerifyError::Binding(b)) => assert_eq!(b, BindingFailure),
        other => panic!("expected a binding failure, got {other:?}"),
    }
}

#[tokio::test]
async fn promote_on_an_expired_row_invalidates_through_f03() {
    let f = fixture().await;
    let past_expiry = clock_at(
        fixed_now() + ChronoDuration::seconds(federation_mfa_pending::PENDING_LIFETIME_SECS + 1),
    );
    let outcome = promote(
        &f.db,
        &past_expiry,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await
    .expect("verify reports Expired; F03 durably invalidates");
    assert!(matches!(
        outcome,
        sui_id_store::commands::F03Outcome::Invalidated { .. }
    ));
}

#[tokio::test]
async fn a_correct_recovery_code_is_a_valid_candidate() {
    let f = fixture().await;
    let code = &f.recovery_codes[0];

    let outcome = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        code,
    )
    .await
    .expect("binding succeeds");
    assert!(matches!(outcome, FederatedMfaOutcome::Recovery(_)));
}

#[tokio::test]
async fn a_wrong_totp_code_is_rejected_with_the_totp_reason() {
    let f = fixture().await;
    let outcome = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "000000",
    )
    .await
    .expect("binding succeeds, the proof is just wrong");
    match outcome {
        FederatedMfaOutcome::Rejected(r) => assert_eq!(r.reason, RejectedReason::Totp),
        other => panic!("expected Rejected(Totp), got a different outcome variant: {other:?}"),
    }
}

#[tokio::test]
async fn a_wrong_recovery_code_is_rejected_with_the_recovery_reason() {
    let f = fixture().await;
    let outcome = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "not-a-real-recovery-code",
    )
    .await
    .expect("binding succeeds, the proof is just wrong");
    match outcome {
        FederatedMfaOutcome::Rejected(r) => assert_eq!(r.reason, RejectedReason::Recovery),
        other => panic!("expected Rejected(Recovery), got a different outcome variant: {other:?}"),
    }
}

/// RFC 096 `:741-743`'s "malformed" reason: empty input is not a wrong
/// TOTP guess and not a wrong recovery code, and must not be scored as
/// either -- without this case, empty input falls through to the
/// recovery-code branch and comes back `RejectedReason::Recovery`,
/// which is the wrong reason for input that was never a recovery-code
/// attempt.
#[tokio::test]
async fn empty_input_is_rejected_with_the_malformed_reason() {
    let f = fixture().await;
    let outcome = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "   ",
    )
    .await
    .expect("binding succeeds, the input is just empty");
    match outcome {
        FederatedMfaOutcome::Rejected(r) => assert_eq!(r.reason, RejectedReason::Malformed),
        other => panic!("expected Rejected(Malformed), got a different outcome variant: {other:?}"),
    }
}

// ── The two required properties (dispatch §2) ───────────────────────────

/// RFC 096 `:754-755`: a binding failure is the same value whichever of
/// several distinct reasons caused it. Four scenarios, asserted equal
/// to each other -- not merely that each is *an* error.
#[tokio::test]
async fn every_binding_failure_is_the_same_value() {
    let f = fixture().await;

    let wrong_browser = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        "not-the-real-browser-binding",
        &f.csrf_raw,
        "123456",
    )
    .await;

    let wrong_csrf = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        "not-the-real-csrf-value",
        "123456",
    )
    .await;

    let unknown_pending_id = verify(
        &f.db,
        &f.clock,
        &FederationMfaPendingId::new().to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;

    // A pending id belonging to a different (otherwise valid) identity.
    let other_provider_id = seed_provider(&f.db).await;
    let other_user_id = seed_user(&f.db).await;
    seed_link(&f.db, other_user_id, other_provider_id).await;
    let other_attempt_id = an_exchanging_attempt(&f.db, other_provider_id, 20).await;
    let (others_pending_id, _) = sui_id_store::commands::issue_federated_mfa_pending(
        &f.db,
        other_attempt_id,
        other_provider_id,
        0,
        0,
        other_user_id,
        None,
        digest32("someone-elses-browser-binding"),
        digest32("someone-elses-csrf"),
        fixed_now(),
    )
    .await
    .unwrap();
    // Present *our* bindings against someone else's pending id -- "not
    // your row", not "no such row".
    let not_your_row = verify(
        &f.db,
        &f.clock,
        &others_pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;

    // `as_binding_failure` both asserts each scenario really is a
    // binding failure (not `Ok`, not `VerifyError::Internal`) and
    // extracts the one comparable value underneath -- `BindingFailure`
    // itself, not the whole `Result` (stage 10a fix round, R7).
    let wrong_browser = as_binding_failure(wrong_browser);
    let wrong_csrf = as_binding_failure(wrong_csrf);
    let unknown_pending_id = as_binding_failure(unknown_pending_id);
    let not_your_row = as_binding_failure(not_your_row);
    assert_eq!(wrong_browser, BindingFailure);
    assert_eq!(wrong_csrf, BindingFailure);
    assert_eq!(unknown_pending_id, BindingFailure);
    assert_eq!(not_your_row, BindingFailure);
    // The point itself: all four are the *same* value, not just four
    // separate `Err`s of the right type.
    assert_eq!(wrong_browser, wrong_csrf);
    assert_eq!(wrong_csrf, unknown_pending_id);
    assert_eq!(unknown_pending_id, not_your_row);
}

/// RFC 096 `:755`: "cannot ... touch the row." `failure_count`, `status`
/// and `continuation_sealed` are unchanged after every one of the four
/// binding failures above -- with a control in the same test showing a
/// binding that *does* succeed is reachable for an untouched second
/// identity, so "unchanged" cannot pass merely because nothing ever
/// looked at the row at all.
#[tokio::test]
async fn a_binding_failure_touches_no_row() {
    let f = fixture().await;
    let before = row_snapshot(&f.db, f.pending_id).await;

    let _ = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        "not-the-real-browser-binding",
        &f.csrf_raw,
        "123456",
    )
    .await;
    assert_eq!(row_snapshot(&f.db, f.pending_id).await, before);

    let _ = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        "not-the-real-csrf-value",
        "123456",
    )
    .await;
    assert_eq!(row_snapshot(&f.db, f.pending_id).await, before);

    let _ = verify(
        &f.db,
        &f.clock,
        &FederationMfaPendingId::new().to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;
    assert_eq!(row_snapshot(&f.db, f.pending_id).await, before);

    let _ = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "not-a-well-formed-anything",
    )
    .await;
    // Even a wrong *proof* (not a binding failure -- `Rejected`) must
    // not touch the row either: this stage writes nothing at all, and
    // "not-a-well-formed-anything" fails as a recovery-code mismatch,
    // which is `Ok(Rejected(..))`, not `Err`.
    assert_eq!(row_snapshot(&f.db, f.pending_id).await, before);

    // Control: a second, untouched identity's binding really does
    // succeed -- without this, "unchanged" above would pass identically
    // if `verify` were a no-op that never read the row at all.
    let control_provider_id = seed_provider(&f.db).await;
    let control_user_id = seed_user(&f.db).await;
    seed_link(&f.db, control_user_id, control_provider_id).await;
    let _ = seed_totp_enabled(&f.db, &f.clock, control_user_id).await;
    let control_attempt_id = an_exchanging_attempt(&f.db, control_provider_id, 30).await;
    let control_browser_raw = "control-browser-binding".to_owned();
    let control_csrf_raw = "control-csrf".to_owned();
    let (control_pending_id, _) = sui_id_store::commands::issue_federated_mfa_pending(
        &f.db,
        control_attempt_id,
        control_provider_id,
        0,
        0,
        control_user_id,
        None,
        digest32(&control_browser_raw),
        digest32(&control_csrf_raw),
        fixed_now(),
    )
    .await
    .unwrap();
    let control_outcome = verify(
        &f.db,
        &f.clock,
        &control_pending_id.to_string(),
        &control_browser_raw,
        &control_csrf_raw,
        "000000",
    )
    .await
    .expect("the control's binding succeeds");
    assert!(matches!(control_outcome, FederatedMfaOutcome::Rejected(_)));
}

// ── Expiry (dispatch §2's last paragraph) ────────────────────────────────

#[tokio::test]
async fn an_expired_row_is_handed_on_not_swallowed() {
    let f = fixture().await;
    let past_expiry = clock_at(
        fixed_now() + ChronoDuration::seconds(federation_mfa_pending::PENDING_LIFETIME_SECS + 1),
    );
    let outcome = verify(
        &f.db,
        &past_expiry,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await
    .expect("expiry is reported, not a binding failure");
    assert!(matches!(outcome, FederatedMfaOutcome::Expired));
    // Expiry must not touch the row either -- stage 10b's F03 does the
    // Invalidated write, not this stage.
    let (failure_count, status, _) = row_snapshot(&f.db, f.pending_id).await;
    assert_eq!(failure_count, 0);
    assert_eq!(
        status,
        sui_id_store::models::FederationMfaPendingStatus::Pending
    );
}

#[tokio::test]
async fn a_row_one_second_before_expiry_still_binds() {
    let f = fixture().await;
    let just_before = clock_at(
        fixed_now() + ChronoDuration::seconds(federation_mfa_pending::PENDING_LIFETIME_SECS - 1),
    );
    let outcome = verify(
        &f.db,
        &just_before,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "000000",
    )
    .await
    .expect("not yet expired");
    assert!(!matches!(outcome, FederatedMfaOutcome::Expired));
}

// ── The other binding-gate rechecks ──────────────────────────────────────

#[tokio::test]
async fn a_disabled_user_is_a_binding_failure() {
    let f = fixture().await;
    users::set_disabled(&f.db, f.user_id, true).await.unwrap();
    let result = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
}

#[tokio::test]
async fn a_disabled_provider_is_a_binding_failure() {
    let f = fixture().await;
    let provider = {
        let row = federation_mfa_pending::find_by_id(&f.db, f.pending_id)
            .await
            .unwrap()
            .unwrap();
        row.provider_id
    };
    federation_provider::set_enabled(&f.db, provider, false, Utc::now())
        .await
        .unwrap();
    let result = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
}

/// RFC 096-B1 stage 10a fix round, R1: the status guard
/// (`row.status != Pending`) was unreachable before this round --
/// `find_by_id` errored on every terminal row, since the pre-fix row
/// type could not hold a `NULL continuation_sealed`
/// (`VerifyError::Internal`, not `VerifyError::Binding`). With R1's
/// `Option<Vec<u8>>` fix, the read now succeeds and the guard itself
/// runs. Matching precisely on `Err(VerifyError::Binding(BindingFailure))`
/// -- not merely `.is_err()` -- is what makes the assertion
/// non-vacuous: before R3's two-case `VerifyError`, a read error and a
/// deliberate refusal were the same value and this test could not have
/// told them apart; now an `Internal` result fails this match arm
/// distinctly.
#[tokio::test]
async fn a_terminalized_row_is_a_binding_failure_through_the_status_branch() {
    let f = fixture().await;
    terminalize(&f.db, f.pending_id, "completed").await;
    let result = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;
    match result {
        Err(VerifyError::Binding(BindingFailure)) => {}
        other => panic!(
            "expected the status guard to produce Err(VerifyError::Binding(BindingFailure)), got {other:?}"
        ),
    }
}

/// **Folded in by the architect at the fix round's acceptance (§M).**
/// R3 split `VerifyError` in two so a broken store stops looking like a
/// wrong cookie, and every other test in this file asserts the
/// [`VerifyError::Binding`] side. Nothing asserted the other one, so
/// collapsing `classify_store_error` to `_ => Binding(BindingFailure)`
/// — R3 entirely undone — left all twelve tests passing. This is the
/// test that fails for that mutation.
///
/// The returned package reasoned that no fixture could reach the
/// branch "without directly corrupting the database file or key". It
/// can: `secret_enc` is an ordinary `BLOB` column and `decrypt_secret`
/// is `open(db.key(), &row.secret_enc, AAD)`, so one `UPDATE` through
/// the same `with_conn` shape [`terminalize`] already uses makes the
/// decrypt — and only the decrypt — fail. What that proves is this
/// function's own classification, not SQLite's and not the AEAD's: the
/// binding checks above all passed, so a `Binding` result here could
/// only come from laundering.
#[tokio::test]
async fn a_store_failure_is_internal_not_a_binding_failure() {
    let f = fixture().await;
    let uid = f.user_id.to_string();
    f.db.with_conn(move |conn| {
        conn.execute(
            "UPDATE user_totp SET secret_enc = X'00112233445566778899' WHERE user_id = ?1",
            [uid],
        )?;
        Ok(())
    })
    .await
    .expect("an ordinary row update, not a corrupted file or key");

    let result = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await;
    match result {
        Err(VerifyError::Internal(_)) => {}
        other => panic!(
            "a failed decrypt must be Err(VerifyError::Internal(..)), never a binding refusal; got {other:?}"
        ),
    }
}

// ── RFC 096-B1 stage 11: verify_webauthn / begin_webauthn_ceremony /
//    promote_webauthn ──────────────────────────────────────────────────
//
// No virtual WebAuthn authenticator exists anywhere in this codebase
// (`authn::webauthn::integration_tests`'s own three tests never
// construct a real `Passkey` or a real successful
// `finish_passkey_authentication` either — only the surrounding guard
// logic and a "dummy", syntactically-valid-but-cryptographically-wrong
// credential). These tests hold to the same limit: they prove
// `bind_pending` reuse, both directions of `MethodMismatch`, the
// ceremony's own AAD tamper check, and the wiring from each entry
// point through to F03 -- not a real cryptographic completion, which
// the dispatch's own package states explicitly rather than silently
// shipping a thinner webauthn suite than its own neighbours without
// saying why.

use sui_id_shared::ids::FederationWebauthnCeremonyId;
use sui_id_store::models::FederationWebauthnCeremonyRow;
use sui_id_store::repos::federation_webauthn_ceremony;
use webauthn_rs::prelude::PublicKeyCredential;

/// Stores a ceremony row directly through the repo layer, bypassing
/// [`begin_webauthn_ceremony`] entirely -- these tests need a ceremony
/// to *exist*, bound correctly, with full control over what
/// `ceremony_state_sealed` actually contains (correctly sealed
/// garbage, or sealed under the wrong AAD), neither of which
/// `begin_webauthn_ceremony`'s own real `webauthn-rs` call could
/// produce without a real enrolled passkey.
async fn seed_ceremony(f: &Fixture, sealed: Vec<u8>) -> FederationWebauthnCeremonyId {
    let row =
        f.db.with_conn({
            let pending_id = f.pending_id;
            let user_id = f.user_id;
            move |conn| {
                let pending =
                    federation_mfa_pending::get_within_tx(conn, pending_id)?.expect("pending row");
                let ceremony = FederationWebauthnCeremonyRow {
                    id: FederationWebauthnCeremonyId::new(),
                    pending_id,
                    provider_id: pending.provider_id,
                    provider_config_version: pending.provider_config_version,
                    provider_activation_generation: pending.provider_activation_generation,
                    user_id,
                    ceremony_state_sealed: sealed,
                    rp_id: "idp.example".to_owned(),
                    origin: "https://idp.example".to_owned(),
                    created_at: Utc::now(),
                    pending_expires_at: pending.expires_at,
                    expires_at: pending.expires_at,
                };
                federation_webauthn_ceremony::insert_or_replace_within_tx(conn, &ceremony)?;
                Ok(ceremony)
            }
        })
        .await
        .expect("seed ceremony");
    row.id
}

/// A correctly-sealed ceremony whose plaintext is not valid
/// `PasskeyAuthentication` JSON -- opens cleanly (proving the AAD
/// binding is right) but fails to deserialize, the farthest this test
/// suite can push [`verify_webauthn`] without a real passkey.
async fn seed_openable_ceremony(f: &Fixture) -> FederationWebauthnCeremonyId {
    let pending = federation_mfa_pending::find_by_id(&f.db, f.pending_id)
        .await
        .unwrap()
        .unwrap();
    let aad = federation_webauthn_ceremony::build_aad(
        f.pending_id,
        pending.provider_id,
        pending.provider_config_version,
        pending.provider_activation_generation,
        f.user_id,
    );
    let sealed =
        sui_id_store::crypto::seal(f.db.key(), b"not a passkey authentication state", &aad)
            .unwrap();
    seed_ceremony(f, sealed).await
}

fn a_dummy_credential() -> PublicKeyCredential {
    serde_json::from_str(
        r#"{"id":"AA","rawId":"AA","type":"public-key","response":{"authenticatorData":"AA","clientDataJSON":"AA","signature":"AA"},"extensions":{}}"#,
    )
    .expect("dummy credential parses")
}

#[tokio::test]
async fn verify_webauthn_reuses_the_same_binding_gate_as_verify() {
    let f = fixture().await;
    let result = verify_webauthn(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &FederationMfaPendingId::new().to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
}

/// RFC 096 `:802`, direction 1: no live ceremony for this row at all.
#[tokio::test]
async fn verify_webauthn_direction_1_method_mismatch_when_no_ceremony_exists() {
    let f = fixture().await;
    let outcome = verify_webauthn(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await
    .expect("a binding success, not an error");
    match outcome {
        FederatedMfaOutcome::Rejected(bound) => {
            assert_eq!(bound.reason, RejectedReason::MethodMismatch)
        }
        other => panic!("expected Rejected(MethodMismatch), got {other:?}"),
    }
}

/// The ceremony's own AAD tamper check (mirrors `continuation_sealed`'s
/// own, stage 9): sealed under a *different* user's AAD, so opening it
/// with this row's own identity must fail.
#[tokio::test]
async fn verify_webauthn_reports_internal_when_the_ceremony_state_fails_to_open() {
    let f = fixture().await;
    let wrong_aad = federation_webauthn_ceremony::build_aad(
        FederationMfaPendingId::new(),
        FederationProviderId::new(),
        0,
        0,
        f.user_id,
    );
    let sealed = sui_id_store::crypto::seal(f.db.key(), b"anything", &wrong_aad).unwrap();
    seed_ceremony(&f, sealed).await;

    let result = verify_webauthn(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await;
    match result {
        Err(VerifyError::Internal(_)) => {}
        other => panic!(
            "a tampered ceremony must be Internal, not a binding refusal or a verdict on the proof; got {other:?}"
        ),
    }
}

/// A ceremony that opens correctly but is not valid
/// `PasskeyAuthentication` JSON -- the farthest this suite can push
/// without a real passkey (see the module doc above).
#[tokio::test]
async fn verify_webauthn_reports_internal_for_a_corrupt_but_openable_ceremony_state() {
    let f = fixture().await;
    seed_openable_ceremony(&f).await;

    let result = verify_webauthn(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await;
    match result {
        Err(VerifyError::Internal(_)) => {}
        other => panic!("expected Internal, got {other:?}"),
    }
}

/// RFC 096 `:802`, direction 2: a live WebAuthn ceremony already bound
/// to this row makes a code submission the wrong method, checked
/// before any TOTP/recovery evaluation -- the row's TOTP enrolment
/// from `fixture()` is deliberately still present and still correct,
/// so a pass here cannot be mistaken for `verify`'s own pre-existing
/// TOTP path happening to reject this input for an unrelated reason.
#[tokio::test]
async fn verify_direction_2_method_mismatch_when_a_live_ceremony_exists() {
    let f = fixture().await;
    seed_openable_ceremony(&f).await;

    let outcome = verify(
        &f.db,
        &f.clock,
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "123456",
    )
    .await
    .expect("a binding success, not an error");
    match outcome {
        FederatedMfaOutcome::Rejected(bound) => {
            assert_eq!(bound.reason, RejectedReason::MethodMismatch)
        }
        other => panic!("expected Rejected(MethodMismatch), got {other:?}"),
    }
}

#[tokio::test]
async fn begin_webauthn_ceremony_reuses_the_same_binding_gate() {
    let f = fixture().await;
    let result = begin_webauthn_ceremony(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &FederationMfaPendingId::new().to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "idp.example",
        "https://idp.example",
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
}

#[tokio::test]
async fn begin_webauthn_ceremony_refuses_an_expired_row() {
    let f = fixture().await;
    let past_expiry = clock_at(fixed_now() + ChronoDuration::seconds(301));
    let result = begin_webauthn_ceremony(
        &f.db,
        &past_expiry,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "idp.example",
        "https://idp.example",
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
}

/// RFC 096 `:822-824`'s own "grants no authentication authority by
/// itself" extends to this: a bound, unexpired request with no
/// enrolled passkey at all has nothing to challenge.
#[tokio::test]
async fn begin_webauthn_ceremony_refuses_a_user_with_no_credentials() {
    let f = fixture().await;
    let result = begin_webauthn_ceremony(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        "idp.example",
        "https://idp.example",
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
    assert!(
        federation_webauthn_ceremony::find_by_pending_id(&f.db, f.pending_id)
            .await
            .unwrap()
            .is_none(),
        "no ceremony was created"
    );
}

#[tokio::test]
async fn promote_webauthn_on_an_unknown_pending_id_is_a_binding_failure() {
    let f = fixture().await;
    let result = promote_webauthn(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &FederationMfaPendingId::new().to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await;
    assert_eq!(as_binding_failure(result), BindingFailure);
}

/// Mirrors `promote_on_an_expired_row_invalidates_through_f03` for the
/// WebAuthn entry point: `FederatedMfaOutcome::Expired` becomes
/// `proof: None`, and F03's own durable recheck is what actually
/// terminalizes the row.
#[tokio::test]
async fn promote_webauthn_on_an_expired_row_invalidates_through_f03() {
    let f = fixture().await;
    let past_expiry = clock_at(fixed_now() + ChronoDuration::seconds(301));
    let outcome = promote_webauthn(
        &f.db,
        &past_expiry,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await
    .expect("invalidation succeeds");
    assert!(matches!(
        outcome,
        sui_id_store::commands::F03Outcome::Invalidated {
            reason: sui_id_store::repos::federation_mfa_pending::InvalidatedReason::Expired
        }
    ));
}

/// The wiring stage 11 adds: `verify_webauthn`'s own `MethodMismatch`
/// (no ceremony) reaching F03 through `promote_webauthn`, ending in
/// the same `RejectedStillPending` outcome
/// `commands::tests::runner::federation::f03_method_mismatch_is_rejected_without_touching_any_ceremony`
/// already proves F03's own side of directly.
#[tokio::test]
async fn promote_webauthn_with_no_ceremony_is_rejected_still_pending_through_f03() {
    let f = fixture().await;
    let outcome = promote_webauthn(
        &f.db,
        &f.clock,
        "https://issuer.example",
        &f.pending_id.to_string(),
        &f.browser_binding_raw,
        &f.csrf_raw,
        &a_dummy_credential(),
    )
    .await
    .expect("rejection succeeds");
    match outcome {
        sui_id_store::commands::F03Outcome::RejectedStillPending { reason, .. } => {
            assert_eq!(
                reason,
                sui_id_store::repos::federation_mfa_pending::MfaRejectionReason::MethodMismatch
            )
        }
        other => panic!("expected RejectedStillPending(MethodMismatch), got {other:?}"),
    }
}

/// `ValidWebAuthn`'s own fields are private, no public constructor --
/// the same proof-of-shape this file never writes for `ValidTotp`/
/// `ValidRecovery` either (their own privacy is proven by
/// `crates/sui-id/tests/compile_fail/`, not here); listed so a reader
/// of this module's own tests does not wonder why the fourth candidate
/// has no analogous in-crate test — it has the same compile-fail
/// fixture instead, added alongside this file.
#[allow(dead_code)]
fn _valid_webauthn_has_no_public_constructor_is_proven_in_compile_fail() {}

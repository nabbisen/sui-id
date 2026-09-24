use super::*;
use crate::errors::RecoveryRefusal;
use crate::models::{PasswordResetTokenRow, ResetTokenOrigin, Role, UserSource};

// ── RFC 103 stage 3 — U37, the D3 invalidations, U10 `origin` ─────────

fn hash() -> Vec<u8> {
    uuid::Uuid::new_v4().as_bytes().to_vec()
}

const REASON: &str = "caller verified by call-back, ticket 4711";

async fn seed_admin_with_fresh_step_up(db: &Database) -> (UserId, sui_id_shared::ids::SessionId) {
    let (admin, session) = an_admin_session(db).await;
    repos::user_totp::upsert_pending(db, admin, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    db.with_conn(move |c| {
        c.execute("UPDATE user_totp SET enabled = 1", [])?;
        repos::sessions::record_step_up_within_tx(c, session, "totp", Utc::now())
    })
    .await
    .expect("step up");
    (admin, session)
}

async fn seed_user(db: &Database, f: impl FnOnce(&mut UserRow)) -> UserId {
    let mut user = a_user();
    f(&mut user);
    repos::users::create(db, &user).await.expect("create user");
    // `create` leaves `source` to the column default; write the requested one.
    let (id, source) = (user.id, user.source.as_str());
    db.with_conn(move |c| {
        c.execute(
            "UPDATE users SET source = ?1 WHERE id = ?2",
            rusqlite::params![source, id.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("set source");
    user.id
}

async fn seed_token(
    db: &Database,
    user_id: UserId,
    via: ResetTokenOrigin,
    issued_by: Option<UserId>,
) -> sui_id_shared::ids::PasswordResetTokenId {
    let id = sui_id_shared::ids::PasswordResetTokenId::new();
    repos::password_reset_tokens::insert(
        db,
        &PasswordResetTokenRow {
            id,
            user_id,
            token_hash: hash(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + TimeDelta::minutes(30),
            consumed_at: None,
            requester_ip: None,
            issued_via: via,
            issued_by,
            revoked_at: None,
        },
    )
    .await
    .expect("seed token");
    id
}

async fn token(
    db: &Database,
    id: sui_id_shared::ids::PasswordResetTokenId,
) -> PasswordResetTokenRow {
    let sql = id.to_string();
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT id FROM password_reset_tokens WHERE id = ?1",
            [sql],
            |_| Ok(()),
        )?)
    })
    .await
    .expect("token exists");
    // Read the full row through its hash.
    let hash = db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT token_hash FROM password_reset_tokens WHERE id = ?1",
                [id.to_string()],
                |r| r.get::<_, Vec<u8>>(0),
            )?)
        })
        .await
        .expect("hash");
    repos::password_reset_tokens::find_by_hash(db, &hash)
        .await
        .expect("find")
        .expect("row")
}

async fn token_rows(db: &Database) -> i64 {
    db.with_conn(|c| {
        Ok(
            c.query_row("SELECT COUNT(*) FROM password_reset_tokens", [], |r| {
                r.get(0)
            })?,
        )
    })
    .await
    .expect("count")
}

async fn audit_rows(db: &Database) -> i64 {
    db.with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))?))
        .await
        .expect("count")
}

async fn last_event(db: &Database) -> crate::models::AuditLogRow {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
}

fn refused(
    result: &StoreResult<crate::registry::Audited<RecoveryLinkGrant>>,
) -> Option<RecoveryRefusal> {
    match result {
        Err(StoreError::RecoveryRefused(why)) => Some(*why),
        _ => None,
    }
}

async fn issue_web(
    db: &Database,
    admin: UserId,
    session: sui_id_shared::ids::SessionId,
    target: UserId,
    reason: &str,
    now: chrono::DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<RecoveryLinkGrant>> {
    issue_recovery_link_as_admin(
        db,
        admin,
        session,
        target,
        hash(),
        reason.to_owned(),
        now + TimeDelta::minutes(30),
        now,
    )
    .await
}

async fn issue_cli(
    db: &Database,
    target: UserId,
    reason: &str,
    now: chrono::DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<RecoveryLinkGrant>> {
    issue_recovery_link_as_operator(
        db,
        target,
        hash(),
        reason.to_owned(),
        now + TimeDelta::minutes(30),
        now,
    )
    .await
}

// ── happy paths ───────────────────────────────────────────────────────

#[tokio::test]
async fn u37_web_issues_a_link_and_writes_one_event() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let now = Utc::now();
    let grant = issue_web(&db, admin, session, target, REASON, now)
        .await
        .expect("issue")
        .into_inner();
    assert_eq!(grant.invalidated, 0);

    let row = token(&db, grant.token_id).await;
    assert_eq!(row.user_id, target);
    assert_eq!(row.issued_via, ResetTokenOrigin::Web);
    assert_eq!(row.issued_by, Some(admin));
    assert_eq!(row.expires_at, now + TimeDelta::minutes(30));

    let event = last_event(&db).await;
    assert_eq!(event.action, "user.recovery_link.issued");
    assert_eq!(event.actor, Some(admin));
    assert_eq!(event.target.as_deref(), Some(target.to_string().as_str()));
    let note = event.note.expect("note");
    assert!(
        note.starts_with(&format!("reason={REASON} via=web expires_at=")),
        "{note}"
    );
    assert!(
        note.contains(" invalidated=0 step_up=fresh:totp:"),
        "{note}"
    );
    assert_eq!(audit_rows(&db).await, 1, "exactly one event");
}

#[tokio::test]
async fn u37_operator_issues_a_link_with_no_actor_and_not_applicable() {
    let db = fresh_db();
    // An administrator target is allowed on the CLI (D5).
    let target = seed_user(&db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await;
    let now = Utc::now();
    let grant = issue_cli(&db, target, REASON, now)
        .await
        .expect("issue")
        .into_inner();

    let row = token(&db, grant.token_id).await;
    assert_eq!(row.issued_via, ResetTokenOrigin::Cli);
    assert_eq!(row.issued_by, None);

    let event = last_event(&db).await;
    assert_eq!(event.action, "user.recovery_link.issued");
    assert_eq!(event.actor, None, "no actor for the CLI");
    let note = event.note.expect("note");
    assert!(note.contains(" via=cli "), "{note}");
    assert!(
        note.ends_with(" step_up=not_applicable:system_principal"),
        "{note}"
    );
}

/// Give `user` a credential row, making it a live account (RFC 115 D10).
async fn give_credential(db: &Database, user: UserId) {
    repos::credentials::upsert(
        db,
        &crate::models::CredentialRow {
            user_id: user,
            password_hash: "some-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("credential");
}

// ── D5: each refusal writes nothing ───────────────────────────────────

#[tokio::test]
async fn u37_web_refusals_write_nothing() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    // A *live* administrator: it holds a credential (RFC 115 D10).
    let other_admin = seed_user(&db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await;
    give_credential(&db, other_admin).await;
    let non_local = seed_user(&db, |u| u.source = UserSource::Ldap).await;
    let disabled = seed_user(&db, |u| u.is_disabled = true).await;
    let deleted = seed_user(&db, |u| {
        u.is_deleted = true;
        u.is_disabled = true;
    })
    .await;
    let cases = [
        (
            "an administrator",
            other_admin,
            RecoveryRefusal::TargetIsAdmin,
        ),
        ("the issuer", admin, RecoveryRefusal::TargetIsSelf),
        (
            "a non-local user",
            non_local,
            RecoveryRefusal::TargetNonLocal,
        ),
        ("a disabled user", disabled, RecoveryRefusal::TargetDisabled),
        ("a deleted user", deleted, RecoveryRefusal::TargetDeleted),
        (
            "an unknown user",
            UserId::new(),
            RecoveryRefusal::TargetUnknown,
        ),
    ];
    let tokens_before = token_rows(&db).await;
    let events_before = audit_rows(&db).await;
    for (label, target, why) in cases {
        let result = issue_web(&db, admin, session, target, REASON, Utc::now()).await;
        assert_eq!(refused(&result), Some(why), "{label}");
        assert_eq!(token_rows(&db).await, tokens_before, "{label}: no token");
        assert_eq!(audit_rows(&db).await, events_before, "{label}: no event");
    }
}

// ── RFC 115 D10: an administrator is refused only when it is live ─────

#[tokio::test]
async fn u37_web_issues_for_an_administrator_that_has_never_been_activated() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let new_admin = seed_user(&db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await;
    let grant = issue_web(&db, admin, session, new_admin, REASON, Utc::now())
        .await
        .expect("a never-activated administrator can be activated on the web")
        .into_inner();
    assert_eq!(token(&db, grant.token_id).await.user_id, new_admin);
}

#[tokio::test]
async fn u37_web_refuses_an_administrator_that_is_live_by_either_signal() {
    // Each half of the predicate `NOT EXISTS credentials AND last_login_at IS
    // NULL` must independently keep an administrator out of reach. The
    // `last_login_at` half is the one a later passwordless-account RFC would
    // rely on; dropping it must be caught here.
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let with_credential = seed_user(&db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await;
    give_credential(&db, with_credential).await;
    let signed_in_no_credential = seed_user(&db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await;
    // `users::create` does not carry `last_login_at`; set it as a sign-in would.
    let signed_in = signed_in_no_credential;
    db.with_conn(move |c| {
        c.execute(
            "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
            rusqlite::params![Utc::now(), signed_in.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("set last_login_at");
    for (label, target) in [
        ("has a credential, never signed in", with_credential),
        ("has signed in, no credential row", signed_in_no_credential),
    ] {
        let result = issue_web(&db, admin, session, target, REASON, Utc::now()).await;
        assert_eq!(
            refused(&result),
            Some(RecoveryRefusal::TargetIsAdmin),
            "{label}"
        );
    }
}

#[tokio::test]
async fn u37_web_still_refuses_a_never_activated_administrator_that_is_not_local() {
    // D10's relaxation cannot reach a directory account: the later non-local
    // check still refuses it (a shadow row has no credential either).
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let ldap_admin = seed_user(&db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
        u.source = UserSource::Ldap;
    })
    .await;
    let result = issue_web(&db, admin, session, ldap_admin, REASON, Utc::now()).await;
    assert_eq!(refused(&result), Some(RecoveryRefusal::TargetNonLocal));
}

#[tokio::test]
async fn u37_operator_refusals_write_nothing() {
    let db = fresh_db();
    let non_local = seed_user(&db, |u| u.source = UserSource::Ldap).await;
    let disabled = seed_user(&db, |u| u.is_disabled = true).await;
    let deleted = seed_user(&db, |u| {
        u.is_deleted = true;
        u.is_disabled = true;
    })
    .await;
    let cases = [
        (
            "a non-local user",
            non_local,
            RecoveryRefusal::TargetNonLocal,
        ),
        ("a disabled user", disabled, RecoveryRefusal::TargetDisabled),
        ("a deleted user", deleted, RecoveryRefusal::TargetDeleted),
        (
            "an unknown user",
            UserId::new(),
            RecoveryRefusal::TargetUnknown,
        ),
    ];
    for (label, target, why) in cases {
        let result = issue_cli(&db, target, REASON, Utc::now()).await;
        assert_eq!(refused(&result), Some(why), "{label}");
        assert_eq!(token_rows(&db).await, 0, "{label}: no token");
        assert_eq!(audit_rows(&db).await, 0, "{label}: no event");
    }
}

#[tokio::test]
async fn u37_an_empty_reason_is_refused_on_both_entries() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    for reason in ["", "   \t"] {
        let web = issue_web(&db, admin, session, target, reason, Utc::now()).await;
        assert_eq!(refused(&web), Some(RecoveryRefusal::ReasonRequired));
        let cli = issue_cli(&db, target, reason, Utc::now()).await;
        assert_eq!(refused(&cli), Some(RecoveryRefusal::ReasonRequired));
    }
    assert_eq!(token_rows(&db).await, 0);
    assert_eq!(audit_rows(&db).await, 0, "no event");
}

// ── D6: the web entry needs a fresh step-up by an administrator with a factor ─

#[tokio::test]
async fn u37_web_needs_a_fresh_step_up() {
    let db = fresh_db();
    let target = seed_user(&db, |_| {}).await;

    // No second factor: `not_required` is refused for this operation.
    let (no_factor, no_factor_session) = an_admin_session(&db).await;
    let r = issue_web(
        &db,
        no_factor,
        no_factor_session,
        target,
        REASON,
        Utc::now(),
    )
    .await;
    assert!(
        matches!(r, Err(StoreError::StepUpRequired)),
        "not_required rolls back"
    );

    // A factor, but the step-up lapsed.
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let later = Utc::now() + TimeDelta::minutes(10);
    let r = issue_web(&db, admin, session, target, REASON, later).await;
    assert!(
        matches!(r, Err(StoreError::StepUpRequired)),
        "lapsed rolls back"
    );

    assert_eq!(token_rows(&db).await, 0);
    assert_eq!(audit_rows(&db).await, 0);
}

// ── D8: the throttle, counted from the database ───────────────────────

#[tokio::test]
async fn u37_the_sixth_web_issuance_in_an_hour_is_refused_per_issuer() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let now = Utc::now();
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        issue_web(
            &db,
            admin,
            session,
            target,
            REASON,
            now + TimeDelta::seconds(i),
        )
        .await
        .unwrap_or_else(|_| panic!("issuance {i}"));
    }
    let tokens = token_rows(&db).await;
    let events = audit_rows(&db).await;
    let sixth = issue_web(
        &db,
        admin,
        session,
        target,
        REASON,
        now + TimeDelta::minutes(1),
    )
    .await;
    assert_eq!(refused(&sixth), Some(RecoveryRefusal::Throttled));
    assert_eq!(token_rows(&db).await, tokens, "the refusal wrote no token");
    assert_eq!(audit_rows(&db).await, events, "the refusal wrote no event");

    // The limit is per issuing administrator.
    let (other, other_session) = seed_admin_with_fresh_step_up(&db).await;
    issue_web(
        &db,
        other,
        other_session,
        target,
        REASON,
        now + TimeDelta::minutes(1),
    )
    .await
    .expect("another administrator is not throttled");

    // And it rolls: an hour after the first, the window has moved.
    let much_later = now + TimeDelta::minutes(61);
    // The fixture session lives an hour; keep it alive past the window.
    db.with_conn(move |c| {
        c.execute(
            "UPDATE sessions SET expires_at = ?1 WHERE id = ?2",
            rusqlite::params![much_later + TimeDelta::hours(1), session.to_string()],
        )?;
        repos::sessions::record_step_up_within_tx(c, session, "totp", much_later)
    })
    .await
    .expect("step up again");
    issue_web(&db, admin, session, target, REASON, much_later)
        .await
        .expect("issuable again after the window rolls");
}

#[tokio::test]
async fn u37_the_sixth_cli_issuance_in_an_hour_is_refused() {
    let db = fresh_db();
    let target = seed_user(&db, |_| {}).await;
    let now = Utc::now();
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        issue_cli(&db, target, REASON, now + TimeDelta::seconds(i))
            .await
            .unwrap_or_else(|_| panic!("issuance {i}"));
    }
    let tokens = token_rows(&db).await;
    let events = audit_rows(&db).await;
    let sixth = issue_cli(&db, target, REASON, now + TimeDelta::minutes(1)).await;
    assert_eq!(refused(&sixth), Some(RecoveryRefusal::Throttled));
    assert_eq!(token_rows(&db).await, tokens);
    assert_eq!(audit_rows(&db).await, events);
    issue_cli(&db, target, REASON, now + TimeDelta::minutes(61))
        .await
        .expect("issuable again after the window rolls");
}

#[tokio::test]
async fn u37_the_web_and_cli_limits_are_counted_separately() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let now = Utc::now();
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        issue_cli(&db, target, REASON, now + TimeDelta::seconds(i))
            .await
            .expect("cli");
    }
    issue_web(
        &db,
        admin,
        session,
        target,
        REASON,
        now + TimeDelta::minutes(1),
    )
    .await
    .expect("the web limit is not spent by the CLI");
}

// ── D3: one live link per user ────────────────────────────────────────

#[tokio::test]
async fn u37_issuing_revokes_the_targets_outstanding_links_only() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let bystander = seed_user(&db, |_| {}).await;
    let first = seed_token(&db, target, ResetTokenOrigin::Email, None).await;
    let second = seed_token(&db, target, ResetTokenOrigin::Email, None).await;
    let bystanders = seed_token(&db, bystander, ResetTokenOrigin::Email, None).await;
    let used = seed_token(&db, target, ResetTokenOrigin::Email, None).await;
    db.with_conn(move |c| {
        c.execute(
            "UPDATE password_reset_tokens SET consumed_at = ?1 WHERE id = ?2",
            rusqlite::params![Utc::now(), used.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("consume");

    let grant = issue_web(&db, admin, session, target, REASON, Utc::now())
        .await
        .expect("issue")
        .into_inner();
    assert_eq!(grant.invalidated, 2, "the two outstanding links");
    assert!(token(&db, first).await.revoked_at.is_some());
    assert!(token(&db, second).await.revoked_at.is_some());
    assert!(
        token(&db, used).await.revoked_at.is_none(),
        "a used link is not revoked"
    );
    assert!(
        token(&db, bystanders).await.revoked_at.is_none(),
        "another user's link is untouched"
    );
    assert!(token(&db, grant.token_id).await.revoked_at.is_none());
    let active = repos::password_reset_tokens::count_active_for_user(&db, target, Utc::now())
        .await
        .expect("count");
    assert_eq!(active, 1, "exactly one live link");
    assert!(
        last_event(&db)
            .await
            .note
            .expect("note")
            .contains(" invalidated=2 ")
    );
}

#[tokio::test]
async fn u37_injected_append_failure_leaves_no_token_and_no_revocation() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let earlier = seed_token(&db, target, ResetTokenOrigin::Email, None).await;
    let tokens = token_rows(&db).await;
    let events = audit_rows(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = issue_web(&db, admin, session, target, REASON, Utc::now()).await;
    assert!(result.is_err(), "the injected failure surfaces");

    assert_eq!(token_rows(&db).await, tokens, "no token was inserted");
    assert!(
        token(&db, earlier).await.revoked_at.is_none(),
        "the earlier link was not revoked"
    );
    assert_eq!(audit_rows(&db).await, events);
}

// ── D3: the other transactions invalidate too ─────────────────────────

async fn two_outstanding(
    db: &Database,
    user: UserId,
) -> [sui_id_shared::ids::PasswordResetTokenId; 2] {
    [
        seed_token(db, user, ResetTokenOrigin::Email, None).await,
        seed_token(db, user, ResetTokenOrigin::Email, None).await,
    ]
}

#[tokio::test]
async fn d3_self_password_change_revokes_outstanding_links() {
    for revoke_others in [true, false] {
        let db = fresh_db();
        let user = seed_user(&db, |_| {}).await;
        let tokens = two_outstanding(&db, user).await;
        change_password_self(
            &db,
            user,
            crate::models::CredentialRow {
                user_id: user,
                password_hash: "new-hash-placeholder".into(),
                must_change: false,
                updated_at: Utc::now(),
            },
            None,
            revoke_others,
        )
        .await
        .expect("change");
        for id in tokens {
            assert!(
                token(&db, id).await.revoked_at.is_some(),
                "revoke_others={revoke_others}: the link is revoked"
            );
        }
    }
}

#[tokio::test]
async fn d3_completing_one_link_revokes_the_others() {
    let db = fresh_db();
    let user = seed_user(&db, |_| {}).await;
    let [used, other] = two_outstanding(&db, user).await;
    consume_and_reset_password(
        &db,
        user,
        used,
        crate::models::CredentialRow {
            user_id: user,
            password_hash: "new-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
        Utc::now(),
        false,
    )
    .await
    .expect("complete");
    let used = token(&db, used).await;
    assert!(used.consumed_at.is_some());
    assert!(
        used.revoked_at.is_none(),
        "the consumed link is used, not revoked"
    );
    assert!(
        token(&db, other).await.revoked_at.is_some(),
        "the other link is revoked"
    );
}

#[tokio::test]
async fn d3_disabling_and_deleting_revoke_outstanding_links() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;

    let disabled = seed_user(&db, |_| {}).await;
    let tokens = two_outstanding(&db, disabled).await;
    disable_user(&db, admin, session, disabled, None, Utc::now())
        .await
        .expect("disable");
    for id in tokens {
        assert!(token(&db, id).await.revoked_at.is_some(), "disable revokes");
    }

    let deleted = seed_user(&db, |_| {}).await;
    let tokens = two_outstanding(&db, deleted).await;
    delete_user(&db, admin, session, deleted, None, Utc::now())
        .await
        .expect("delete");
    for id in tokens {
        assert!(token(&db, id).await.revoked_at.is_some(), "delete revokes");
    }

    // Enabling again does not bring a revoked link back.
    enable_user(&db, admin, session, disabled, Utc::now())
        .await
        .expect("enable");
    assert!(
        repos::password_reset_tokens::count_active_for_user(&db, disabled, Utc::now())
            .await
            .expect("count")
            == 0
    );
}

// ── a revoked token cannot be consumed; U10 records `origin` ──────────

#[tokio::test]
async fn a_revoked_link_cannot_complete_and_writes_no_credential() {
    let db = fresh_db();
    let user = seed_user(&db, |_| {}).await;
    let id = seed_token(&db, user, ResetTokenOrigin::Email, None).await;
    let stamp = Utc::now();
    db.with_conn(move |c| {
        repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(c, user, None, stamp)
    })
    .await
    .expect("revoke");
    let result = consume_and_reset_password(
        &db,
        user,
        id,
        crate::models::CredentialRow {
            user_id: user,
            password_hash: "new-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
        Utc::now(),
        false,
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "the guard refuses it"
    );
    assert!(
        repos::credentials::get(&db, user).await.is_err(),
        "no credential row"
    );
    assert_eq!(audit_rows(&db).await, 0);
}

#[tokio::test]
async fn u10_records_the_origin_of_the_consumed_link() {
    let admin_holder = fresh_db();
    let admin = seed_user(&admin_holder, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await;
    drop(admin_holder);
    for (via, issued_by) in [
        (ResetTokenOrigin::Email, None),
        (ResetTokenOrigin::Cli, None),
        (ResetTokenOrigin::Web, Some(admin)),
    ] {
        let db = fresh_db();
        if issued_by.is_some() {
            // The issuing administrator must exist for the foreign key.
            let mut a = a_user();
            a.id = admin;
            a.is_admin = true;
            a.role = Role::Admin;
            repos::users::create(&db, &a).await.expect("admin");
        }
        let user = seed_user(&db, |_| {}).await;
        let id = seed_token(&db, user, via, issued_by).await;
        consume_and_reset_password(
            &db,
            user,
            id,
            crate::models::CredentialRow {
                user_id: user,
                password_hash: "new-hash-placeholder".into(),
                must_change: false,
                updated_at: Utc::now(),
            },
            Utc::now(),
            false,
        )
        .await
        .expect("complete");
        let event = last_event(&db).await;
        assert_eq!(event.action, "auth.password.reset_completed");
        assert_eq!(
            event.note.as_deref(),
            Some(format!("origin={}", via.as_str()).as_str()),
            "{via:?}"
        );
    }
}

// ── the reason is free text in a `key=value` note ─────────────────────

#[tokio::test]
async fn u37_a_reason_that_imitates_fields_cannot_displace_the_recorded_ones() {
    // The note format does not escape values, so a reason can add fake
    // fields. The real ones are written after it, which is what makes the
    // last occurrence of each the true one; this pins that order.
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let forged = "x via=cli invalidated=9 step_up=not_applicable:system_principal";
    issue_web(&db, admin, session, target, forged, Utc::now())
        .await
        .expect("issue");
    let note = last_event(&db).await.note.expect("note");
    let after_reason = note
        .strip_prefix(&format!("reason={forged} "))
        .unwrap_or_else(|| panic!("the reason is the first field: {note}"));
    assert!(after_reason.starts_with("via=web expires_at="), "{note}");
    assert!(
        after_reason.contains(" invalidated=0 step_up=fresh:totp:"),
        "{note}"
    );
    assert_eq!(
        note.rsplit(" via=").next().map(|t| t.starts_with("web ")),
        Some(true)
    );
    assert!(
        note.rsplit(" step_up=")
            .next()
            .is_some_and(|v| v.starts_with("fresh:totp:")),
        "the last step_up= is the real one: {note}"
    );
}

/// RFC 103 stage 3 ruling 3: the reason is bounded and printable, as typed
/// refusals raised before any write, on both entries.
#[tokio::test]
async fn u37_reason_bounds_are_typed_refusals_that_write_nothing() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    let earlier = seed_token(&db, target, ResetTokenOrigin::Email, None).await;
    let cases: Vec<(&str, String, RecoveryRefusal)> = vec![
        (
            "201 characters",
            "r".repeat(201),
            RecoveryRefusal::ReasonTooLong,
        ),
        // 200 characters, but 600 bytes: over the audit attribute's byte
        // bound. Before the byte cap this failed as a generic storage error.
        (
            "200 three-byte characters",
            "あ".repeat(200),
            RecoveryRefusal::ReasonTooLong,
        ),
        (
            "171 three-byte characters (513 bytes)",
            "あ".repeat(171),
            RecoveryRefusal::ReasonTooLong,
        ),
        (
            "a newline inside",
            "line one\nline two".into(),
            RecoveryRefusal::ReasonHasControlCharacters,
        ),
        (
            "a tab inside",
            "a\tb".into(),
            RecoveryRefusal::ReasonHasControlCharacters,
        ),
        (
            "an escape character",
            "a\u{1b}[31mb".into(),
            RecoveryRefusal::ReasonHasControlCharacters,
        ),
        (
            "a DEL character",
            "a\u{7f}b".into(),
            RecoveryRefusal::ReasonHasControlCharacters,
        ),
        (
            "a NUL character",
            "a\0b".into(),
            RecoveryRefusal::ReasonHasControlCharacters,
        ),
    ];
    for (label, reason, why) in &cases {
        let web = issue_web(&db, admin, session, target, reason, Utc::now()).await;
        assert_eq!(refused(&web), Some(*why), "web: {label}");
        let cli = issue_cli(&db, target, reason, Utc::now()).await;
        assert_eq!(refused(&cli), Some(*why), "cli: {label}");
    }
    assert_eq!(token_rows(&db).await, 1, "only the seeded token");
    assert!(
        token(&db, earlier).await.revoked_at.is_none(),
        "nothing was revoked"
    );
    assert_eq!(audit_rows(&db).await, 0, "no event for any refusal");
}

#[tokio::test]
async fn u37_reasons_at_the_bounds_are_accepted_and_recorded_trimmed() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    for (label, reason, recorded) in [
        ("200 characters", "r".repeat(200), "r".repeat(200)),
        (
            "170 three-byte characters (510 bytes)",
            "あ".repeat(170),
            "あ".repeat(170),
        ),
        // Surrounding whitespace, including a newline, is trimmed away before
        // the control-character check; only an inner one is refused.
        (
            "padded",
            "  \treason with spaces\n ".into(),
            "reason with spaces".into(),
        ),
    ] {
        issue_web(&db, admin, session, target, &reason, Utc::now())
            .await
            .unwrap_or_else(|e| panic!("web: {label}: {e:?}"));
        let note = last_event(&db).await.note.expect("note");
        assert!(
            note.starts_with(&format!("reason={recorded} via=web ")),
            "{label}: {note}"
        );
        issue_cli(&db, target, &reason, Utc::now())
            .await
            .unwrap_or_else(|e| panic!("cli: {label}: {e:?}"));
        let note = last_event(&db).await.note.expect("note");
        assert!(
            note.starts_with(&format!("reason={recorded} via=cli ")),
            "{label}: {note}"
        );
    }
}

// ── the repo function's own contract ─────────────────────────────────

#[tokio::test]
async fn revoke_outstanding_spares_the_excepted_used_and_expired_tokens() {
    let db = fresh_db();
    let user = seed_user(&db, |_| {}).await;
    let spared = seed_token(&db, user, ResetTokenOrigin::Email, None).await;
    let revoked = seed_token(&db, user, ResetTokenOrigin::Email, None).await;
    let used = seed_token(&db, user, ResetTokenOrigin::Email, None).await;
    let expired = seed_token(&db, user, ResetTokenOrigin::Email, None).await;
    db.with_conn(move |c| {
        c.execute(
            "UPDATE password_reset_tokens SET consumed_at = ?1 WHERE id = ?2",
            rusqlite::params![Utc::now(), used.to_string()],
        )?;
        c.execute(
            "UPDATE password_reset_tokens SET expires_at = ?1 WHERE id = ?2",
            rusqlite::params![Utc::now() - TimeDelta::minutes(1), expired.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("arrange");

    let count = db
        .with_conn(move |c| {
            repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(
                c,
                user,
                Some(spared),
                Utc::now(),
            )
        })
        .await
        .expect("revoke");
    assert_eq!(count, 1, "only the one outstanding, unexcepted token");
    assert!(token(&db, revoked).await.revoked_at.is_some());
    for (label, id) in [("excepted", spared), ("used", used), ("expired", expired)] {
        assert!(
            token(&db, id).await.revoked_at.is_none(),
            "{label} is left alone"
        );
    }
}

// ── D2/D3: an expired link, of any origin, is refused at completion ────

#[tokio::test]
async fn an_expired_token_cannot_complete_and_writes_no_credential() {
    // RFC 103 T4/T7's control is that the link expires; measured directly
    // against the completion guard rather than assumed from the D3
    // revocation tests, which never exercise the `expires_at` branch.
    let db = fresh_db();
    let user = seed_user(&db, |_| {}).await;
    let id = sui_id_shared::ids::PasswordResetTokenId::new();
    let now = Utc::now();
    repos::password_reset_tokens::insert(
        &db,
        &PasswordResetTokenRow {
            id,
            user_id: user,
            token_hash: hash(),
            issued_at: now - TimeDelta::minutes(31),
            expires_at: now - TimeDelta::minutes(1),
            consumed_at: None,
            requester_ip: None,
            issued_via: ResetTokenOrigin::Email,
            issued_by: None,
            revoked_at: None,
        },
    )
    .await
    .expect("seed an already-expired token");

    let result = consume_and_reset_password(
        &db,
        user,
        id,
        crate::models::CredentialRow {
            user_id: user,
            password_hash: "new-hash-placeholder".into(),
            must_change: false,
            updated_at: now,
        },
        now,
        false,
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "an expired token is refused"
    );
    assert!(
        repos::credentials::get(&db, user).await.is_err(),
        "no credential row"
    );
    assert_eq!(audit_rows(&db).await, 0);
}

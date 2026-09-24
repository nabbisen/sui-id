use super::recovery::{
    REASON, audit_rows, give_credential, issue_cli, issue_web, last_event, refused,
    seed_admin_with_fresh_step_up, seed_live_user, seed_user, token_rows,
};
use super::*;
use crate::errors::RecoveryRefusal;
use crate::models::Role;

// ── RFC 115 D11 — the provisioning throttle ───────────────────────────
//
// A link is *provisioning* when U37 reads its target, inside its own
// transaction, as a local non-administrator that has never held a credential,
// never signed in and never had a token of any kind. It is counted against its
// own, larger ceiling and never against RFC 103 D8's five. A live account can
// never qualify, which is the property these tests own and mutate.

async fn is_provisioning_row(db: &Database, id: sui_id_shared::ids::PasswordResetTokenId) -> bool {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT provisioning FROM password_reset_tokens WHERE id = ?1",
            [id.to_string()],
            |r| r.get::<_, i64>(0),
        )? != 0)
    })
    .await
    .expect("read provisioning")
}

async fn fresh_admin_target(db: &Database) -> UserId {
    seed_user(db, |u| {
        u.is_admin = true;
        u.role = Role::Admin;
    })
    .await
}

#[tokio::test]
async fn the_first_link_for_a_fresh_non_administrator_is_a_provisioning_link_and_says_so() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let fresh = seed_user(&db, |_| {}).await;
    let grant = issue_web(&db, admin, session, fresh, REASON, Utc::now())
        .await
        .expect("issue")
        .into_inner();
    assert!(is_provisioning_row(&db, grant.token_id).await);
    let note = last_event(&db).await.note.expect("note");
    assert!(
        note.contains(" invalidated=0 provisioning=1 step_up=fresh:totp:"),
        "{note}"
    );
}

#[tokio::test]
async fn a_live_account_can_never_be_provisioning() {
    // Every way an account can be "not brand new" keeps its links ordinary.
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;

    let with_credential = seed_live_user(&db).await;

    let signed_in_no_credential = seed_user(&db, |_| {}).await;
    db.with_conn(move |c| {
        c.execute(
            "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
            rusqlite::params![Utc::now(), signed_in_no_credential.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("set last_login_at");

    // Fresh, but it already had a link (consumed, revoked or expired all count:
    // "a token of any kind"). The first issuance below made it ineligible.
    let had_a_link = seed_user(&db, |_| {}).await;
    let first = issue_web(&db, admin, session, had_a_link, REASON, Utc::now())
        .await
        .expect("first")
        .into_inner();
    assert!(is_provisioning_row(&db, first.token_id).await);
    let second = issue_web(&db, admin, session, had_a_link, REASON, Utc::now())
        .await
        .expect("second")
        .into_inner();

    let mut ordinary = vec![second.token_id];
    for (label, target) in [
        ("has a credential", with_credential),
        ("has signed in, no credential", signed_in_no_credential),
    ] {
        let g = issue_web(&db, admin, session, target, REASON, Utc::now())
            .await
            .unwrap_or_else(|_| panic!("{label}"))
            .into_inner();
        assert!(
            !is_provisioning_row(&db, g.token_id).await,
            "{label}: must not be provisioning"
        );
        ordinary.push(g.token_id);
    }
    assert!(
        !is_provisioning_row(&db, ordinary[0]).await,
        "a second link for an account that already had one is ordinary"
    );

    // The event of an ordinary link carries no `provisioning` field.
    let note = last_event(&db).await.note.expect("note");
    assert!(!note.contains("provisioning"), "{note}");
}

#[tokio::test]
async fn an_earlier_token_of_any_state_keeps_a_fresh_account_ordinary() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    for state in ["consumed", "revoked", "expired"] {
        let target = seed_user(&db, |_| {}).await;
        let id = sui_id_shared::ids::PasswordResetTokenId::new();
        let (consumed, revoked, expires) = match state {
            "consumed" => (Some(Utc::now()), None, Utc::now() + TimeDelta::minutes(5)),
            "revoked" => (None, Some(Utc::now()), Utc::now() + TimeDelta::minutes(5)),
            _ => (None, None, Utc::now() - TimeDelta::minutes(5)),
        };
        repos::password_reset_tokens::insert(
            &db,
            &crate::models::PasswordResetTokenRow {
                id,
                user_id: target,
                token_hash: super::recovery::hash(),
                issued_at: Utc::now() - TimeDelta::minutes(10),
                expires_at: expires,
                consumed_at: consumed,
                requester_ip: None,
                issued_via: crate::models::ResetTokenOrigin::Email,
                issued_by: None,
                revoked_at: revoked,
            },
        )
        .await
        .expect("seed earlier token");
        let g = issue_web(&db, admin, session, target, REASON, Utc::now())
            .await
            .unwrap_or_else(|_| panic!("{state}"))
            .into_inner();
        assert!(
            !is_provisioning_row(&db, g.token_id).await,
            "an earlier {state} token: not provisioning"
        );
    }
}

#[tokio::test]
async fn a_never_activated_administrator_is_not_exempt_and_counts_against_the_five() {
    // D10 lets an administrator be activated; D11 does not let that be
    // unthrottled. Five ordinary links, then the sixth is refused, while a
    // fresh non-administrator can still be provisioned.
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let now = Utc::now();
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        let target = fresh_admin_target(&db).await;
        let g = issue_web(
            &db,
            admin,
            session,
            target,
            REASON,
            now + TimeDelta::seconds(i),
        )
        .await
        .unwrap_or_else(|_| panic!("administrator {i}"))
        .into_inner();
        assert!(!is_provisioning_row(&db, g.token_id).await, "admin {i}");
    }
    let sixth = fresh_admin_target(&db).await;
    let result = issue_web(
        &db,
        admin,
        session,
        sixth,
        REASON,
        now + TimeDelta::minutes(1),
    )
    .await;
    assert_eq!(refused(&result), Some(RecoveryRefusal::Throttled));

    let fresh_user = seed_user(&db, |_| {}).await;
    let g = issue_web(
        &db,
        admin,
        session,
        fresh_user,
        REASON,
        now + TimeDelta::minutes(1),
    )
    .await
    .expect("a fresh non-administrator is still provisioned")
    .into_inner();
    assert!(is_provisioning_row(&db, g.token_id).await);
}

#[tokio::test]
async fn provisioning_has_its_own_ceiling_and_it_is_not_free() {
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let now = Utc::now();
    // Far more than five, well under the provisioning ceiling: a bulk import.
    for i in 0..PROVISIONING_LINKS_PER_HOUR {
        let target = seed_user(&db, |_| {}).await;
        issue_web(
            &db,
            admin,
            session,
            target,
            REASON,
            now + TimeDelta::seconds(i),
        )
        .await
        .unwrap_or_else(|_| panic!("provisioning {i}"));
    }
    let tokens = token_rows(&db).await;
    let events = audit_rows(&db).await;
    let over = seed_user(&db, |_| {}).await;
    let result = issue_web(
        &db,
        admin,
        session,
        over,
        REASON,
        now + TimeDelta::minutes(1),
    )
    .await;
    assert_eq!(
        refused(&result),
        Some(RecoveryRefusal::Throttled),
        "a runaway is stopped"
    );
    assert_eq!(token_rows(&db).await, tokens, "the refusal wrote no token");
    assert_eq!(audit_rows(&db).await, events, "and no event");

    // The ceilings do not share a count: ordinary links are still available,
    // for a live account, up to their own five.
    let live = seed_live_user(&db).await;
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        issue_web(
            &db,
            admin,
            session,
            live,
            REASON,
            now + TimeDelta::minutes(2) + TimeDelta::seconds(i),
        )
        .await
        .unwrap_or_else(|_| panic!("ordinary {i}"));
    }
    let sixth = issue_web(
        &db,
        admin,
        session,
        live,
        REASON,
        now + TimeDelta::minutes(3),
    )
    .await;
    assert_eq!(refused(&sixth), Some(RecoveryRefusal::Throttled));
}

#[tokio::test]
async fn using_the_provisioning_ceiling_cannot_unthrottle_a_link_for_an_existing_account() {
    // "Create a user in order to get an unthrottled issuance" yields a link
    // only to the account just created. An administrator who has used all five
    // ordinary links is still refused for a live account, however many fresh
    // ones they provision.
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let now = Utc::now();
    let live = seed_live_user(&db).await;
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        issue_web(
            &db,
            admin,
            session,
            live,
            REASON,
            now + TimeDelta::seconds(i),
        )
        .await
        .expect("ordinary");
    }
    for _ in 0..3 {
        let fresh = seed_user(&db, |_| {}).await;
        issue_web(
            &db,
            admin,
            session,
            fresh,
            REASON,
            now + TimeDelta::minutes(1),
        )
        .await
        .expect("provisioning");
    }
    let again = issue_web(
        &db,
        admin,
        session,
        live,
        REASON,
        now + TimeDelta::minutes(2),
    )
    .await;
    assert_eq!(refused(&again), Some(RecoveryRefusal::Throttled));
}

#[tokio::test]
async fn the_operator_cli_counts_provisioning_separately_and_never_exempts_an_administrator() {
    let db = fresh_db();
    let now = Utc::now();
    let fresh = seed_user(&db, |_| {}).await;
    let g = issue_cli(&db, fresh, REASON, now)
        .await
        .expect("cli provisioning")
        .into_inner();
    assert!(is_provisioning_row(&db, g.token_id).await);
    assert!(
        last_event(&db)
            .await
            .note
            .expect("note")
            .contains(" invalidated=0 provisioning=1 step_up=not_applicable:system_principal"),
    );

    // An administrator target on the CLI is ordinary: five, then refused.
    for i in 0..RECOVERY_LINKS_PER_HOUR {
        let admin_target = fresh_admin_target(&db).await;
        let g = issue_cli(&db, admin_target, REASON, now + TimeDelta::seconds(i))
            .await
            .unwrap_or_else(|_| panic!("cli administrator {i}"))
            .into_inner();
        assert!(!is_provisioning_row(&db, g.token_id).await);
    }
    let sixth = fresh_admin_target(&db).await;
    let result = issue_cli(&db, sixth, REASON, now + TimeDelta::minutes(1)).await;
    assert_eq!(refused(&result), Some(RecoveryRefusal::Throttled));
}

#[tokio::test]
async fn a_live_account_stays_ordinary_even_when_it_is_the_only_thing_with_no_token() {
    // The property, stated as one line: give an account a credential and it is
    // never provisioning, whatever else is true of it.
    let db = fresh_db();
    let (admin, session) = seed_admin_with_fresh_step_up(&db).await;
    let target = seed_user(&db, |_| {}).await;
    give_credential(&db, target).await;
    let g = issue_web(&db, admin, session, target, REASON, Utc::now())
        .await
        .expect("issue")
        .into_inner();
    assert!(!is_provisioning_row(&db, g.token_id).await);
}

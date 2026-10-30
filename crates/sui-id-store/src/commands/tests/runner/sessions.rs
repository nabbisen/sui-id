use super::*;

// ── U18 — revoke one session (cycle I stage 2) ───────────────────────────
//
// Three call sites, two shapes: `revoke_session` (a human actor —
// self sign-out or self-service) and `revoke_session_idle_timeout` (no
// actor, the system principal). Both commit `auth.session.revoked`; the
// `trigger` attribute says which. The idle-timeout path is the one that
// needed `system_principal: permitted` (U08/U22's precedent) — exercised
// here, not merely asserted.

#[tokio::test]
async fn u18_idle_timeout_injected_failure_before_append_leaves_session_live() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = revoke_session_idle_timeout(&db, user.id, session_id, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");
    assert!(
        repos::sessions::get(&db, session_id)
            .await
            .expect("get session")
            .revoked_at
            .is_none(),
        "the revoke rolled back: the session is still live"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");

    // Control: the same call, with no failure injected, revokes the
    // session and writes the event.
    let before = audit_rows(&db).await;
    revoke_session_idle_timeout(&db, user.id, session_id, Utc::now())
        .await
        .expect("the control succeeds");
    assert!(
        repos::sessions::get(&db, session_id)
            .await
            .expect("get session")
            .revoked_at
            .is_some(),
        "the session must be revoked"
    );
    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.session.revoked");
    assert_eq!(tail.note.as_deref(), Some("trigger=idle_timeout"));
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_rollback_coverage("U18");
    record_exactly_once_coverage("U18");
}

#[tokio::test]
async fn u18_self_signout_and_self_service_record_their_own_trigger() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    let signout_session = seed_active_session(&db, user.id).await;
    revoke_session(
        &db,
        user.id,
        user.id,
        signout_session,
        SessionRevokeTrigger::SelfSignout,
        Utc::now(),
    )
    .await
    .expect("self sign-out revoke");
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.session.revoked")
    );
    assert_eq!(
        repos::audit::recent(&db, 1)
            .await
            .expect("tail")
            .into_iter()
            .next()
            .expect("row")
            .note
            .as_deref(),
        Some("trigger=self_signout")
    );

    let service_session = seed_active_session(&db, user.id).await;
    revoke_session(
        &db,
        user.id,
        user.id,
        service_session,
        SessionRevokeTrigger::SelfService,
        Utc::now(),
    )
    .await
    .expect("self-service revoke");
    assert_eq!(
        repos::audit::recent(&db, 1)
            .await
            .expect("tail")
            .into_iter()
            .next()
            .expect("row")
            .note
            .as_deref(),
        Some("trigger=self_service"),
        "the two human-actor triggers must be distinguishable in the log"
    );

    assert!(
        repos::sessions::get(&db, signout_session)
            .await
            .expect("get")
            .revoked_at
            .is_some()
    );
    assert!(
        repos::sessions::get(&db, service_session)
            .await
            .expect("get")
            .revoked_at
            .is_some()
    );
}

// ── U19 — revoke all sessions / force logout (cycle I stage 2, amended
//    stage 2b) ───────────────────────────────────────────────────────────
//
// Two surfaces, one event: RP-initiated logout (which is itself one of
// two entry functions, `_as_user` or `_unattributed`, depending on how
// the caller identified the user) and `revoke_other_sessions_for_user`
// (self-service "sign out other sessions"). All three commit
// `auth.sessions.revoked`, with `trigger`, `count` and `identified_by`
// attributes. Retires the best-effort `auth.sessions.bulk_revoke_self`.

#[tokio::test]
async fn u19_rp_initiated_injected_failure_before_append_leaves_sessions_live() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let s1 = seed_active_session(&db, user.id).await;
    let s2 = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = revoke_all_sessions_for_user_as_user(
        &db,
        user.id,
        SessionsRevokeTrigger::RpInitiatedLogout,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");
    assert!(
        repos::sessions::get(&db, s1)
            .await
            .expect("get s1")
            .revoked_at
            .is_none(),
        "the revoke rolled back: s1 is still live"
    );
    assert!(
        repos::sessions::get(&db, s2)
            .await
            .expect("get s2")
            .revoked_at
            .is_none(),
        "the revoke rolled back: s2 is still live"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");

    // Control: the same call, with no failure injected, revokes both
    // sessions and writes one event naming the count.
    let before = audit_rows(&db).await;
    let audited = revoke_all_sessions_for_user_as_user(
        &db,
        user.id,
        SessionsRevokeTrigger::RpInitiatedLogout,
        Utc::now(),
    )
    .await
    .expect("the control succeeds");
    assert_eq!(audited.into_inner(), 2, "both sessions counted");
    assert!(
        repos::sessions::get(&db, s1)
            .await
            .expect("get s1")
            .revoked_at
            .is_some()
    );
    assert!(
        repos::sessions::get(&db, s2)
            .await
            .expect("get s2")
            .revoked_at
            .is_some()
    );
    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.sessions.revoked");
    assert_eq!(
        tail.note.as_deref(),
        Some("trigger=rp_initiated_logout count=2 identified_by=session")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_rollback_coverage("U19");
    record_exactly_once_coverage("U19");
}

#[tokio::test]
async fn u19_rp_initiated_via_session_records_the_user_as_actor() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    seed_active_session(&db, user.id).await;

    revoke_all_sessions_for_user_as_user(
        &db,
        user.id,
        SessionsRevokeTrigger::RpInitiatedLogout,
        Utc::now(),
    )
    .await
    .expect("revoke via a validated session");

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.sessions.revoked");
    assert_eq!(
        tail.actor,
        Some(user.id),
        "identity came from a validated session, so the actor is proven"
    );
    assert_eq!(
        tail.note.as_deref(),
        Some("trigger=rp_initiated_logout count=1 identified_by=session")
    );
}

#[tokio::test]
async fn u19_rp_initiated_via_id_token_hint_records_no_actor() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    seed_active_session(&db, user.id).await;

    let audited = revoke_all_sessions_for_user_unattributed(
        &db,
        user.id,
        SessionsRevokeTrigger::RpInitiatedLogout,
        Utc::now(),
    )
    .await
    .expect("revoke via id_token_hint alone");
    assert_eq!(audited.into_inner(), 1, "the session is still counted");

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.sessions.revoked");
    assert_eq!(
        tail.actor, None,
        "identity came from id_token_hint alone, with no validated session \
         behind it, so no actor is recorded"
    );
    assert_eq!(
        tail.note.as_deref(),
        Some("trigger=rp_initiated_logout count=1 identified_by=id_token_hint")
    );
}

#[tokio::test]
async fn u19_self_service_except_keeps_the_named_session_and_counts_the_rest() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let keep = seed_active_session(&db, user.id).await;
    let other1 = seed_active_session(&db, user.id).await;
    let other2 = seed_active_session(&db, user.id).await;

    let audited = revoke_other_sessions_for_user(
        &db,
        user.id,
        keep,
        SessionsRevokeTrigger::SelfService,
        Utc::now(),
    )
    .await
    .expect("revoke others");
    assert_eq!(
        audited.into_inner(),
        2,
        "the two non-kept sessions are counted"
    );

    assert!(
        repos::sessions::get(&db, keep)
            .await
            .expect("get keep")
            .revoked_at
            .is_none(),
        "the kept session must not be revoked"
    );
    assert!(
        repos::sessions::get(&db, other1)
            .await
            .expect("get")
            .revoked_at
            .is_some()
    );
    assert!(
        repos::sessions::get(&db, other2)
            .await
            .expect("get")
            .revoked_at
            .is_some()
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.sessions.revoked");
    assert_eq!(
        tail.note.as_deref(),
        Some("trigger=self_service count=2 identified_by=session"),
        "retires the best-effort auth.sessions.bulk_revoke_self this surface used to append"
    );
}

#[tokio::test]
async fn u19_with_no_other_sessions_counts_zero_and_still_records_the_attempt() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let keep = seed_active_session(&db, user.id).await;
    let before = audit_rows(&db).await;

    let audited = revoke_other_sessions_for_user(
        &db,
        user.id,
        keep,
        SessionsRevokeTrigger::SelfService,
        Utc::now(),
    )
    .await
    .expect("revoke others, none to revoke");
    assert_eq!(audited.into_inner(), 0);
    assert_eq!(
        repos::audit::recent(&db, 1)
            .await
            .expect("tail")
            .into_iter()
            .next()
            .expect("row")
            .note
            .as_deref(),
        Some("trigger=self_service count=0 identified_by=session"),
        "a zero-count attempt is still a fact worth recording, not a silent no-op"
    );
    assert_eq!(audit_rows(&db).await, before + 1);
}

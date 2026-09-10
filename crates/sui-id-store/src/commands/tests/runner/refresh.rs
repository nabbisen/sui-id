use super::*;

// ── T04/T09 — refresh-token rotation and initial issuance ───────

async fn seed_family(
    db: &Database,
) -> (UserId, ClientId, FamilyId, sui_id_shared::RefreshTokenHash) {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    let client = a_client();
    repos::clients::create(db, &client)
        .await
        .expect("create client");

    let root_id = sui_id_shared::RefreshTokenId::generate();
    let family = FamilyId::root_of(&root_id);
    let row = crate::models::RefreshTokenRow {
        id: root_id,
        user_id: user.id,
        client_id: client.id,
        scope: "openid".into(),
        expires_at: Utc::now() + TimeDelta::hours(1),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: family.clone(),
    };
    let prepared =
        repos::refresh_tokens::prepare_refresh_token(db.key(), row).expect("prepare root token");
    let hash = sui_id_shared::RefreshTokenHash::of(&prepared.raw_token);
    insert_initial_refresh_token(db, prepared)
        .await
        .expect("t09 initial issue");
    (user.id, client.id, family, hash)
}

fn a_successor(
    user_id: UserId,
    client_id: ClientId,
    family: FamilyId,
) -> crate::models::RefreshTokenRow {
    crate::models::RefreshTokenRow {
        id: sui_id_shared::RefreshTokenId::generate(),
        user_id,
        client_id,
        scope: "openid".into(),
        expires_at: Utc::now() + TimeDelta::hours(1),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: family,
    }
}

#[tokio::test]
async fn t09_protocol_issues_initial_token_with_no_audit_row() {
    let db = fresh_db();
    let before = latest_audit_action(&db).await;
    let (_, _, _, hash) = seed_family(&db).await;

    // The row genuinely exists (T09 really wrote it) ...
    assert!(
        repos::refresh_tokens::begin_rotation(&db, &hash, &ClientId::new(), Utc::now())
            .await
            .is_err(),
        "sanity: wrong client must reject without revoking"
    );
    // ... but no audit row exists for it -- T09 has no path to
    // Audited<T>, by construction (Database::protocol).
    assert_eq!(latest_audit_action(&db).await, before);
}

#[tokio::test]
async fn t04_normal_rotation_revokes_old_inserts_successor_and_appends_rotated() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;

    let successor = a_successor(user_id, client_id, family.clone());
    let successor_id = successor.id.clone();
    let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

    let audited = rotate_refresh_token(&db, hash, client_id, Utc::now(), prepared)
        .await
        .expect("rotate");
    match audited.into_inner() {
        T04Outcome::Rotated { successor, .. } => {
            assert_eq!(successor.id, successor_id);
        }
        T04Outcome::TheftDetected { .. } => panic!("expected Rotated"),
    }

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.refresh.rotated")
    );
}

#[tokio::test]
async fn t04_reuse_revokes_family_and_appends_theft_detected() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;

    // First rotation: legitimate, wins.
    let s1 = a_successor(user_id, client_id, family.clone());
    let p1 = repos::refresh_tokens::prepare_refresh_token(db.key(), s1).unwrap();
    rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), p1)
        .await
        .expect("first rotation");

    // Replay of the now-revoked root token: reuse.
    let s2 = a_successor(user_id, client_id, family.clone());
    let p2 = repos::refresh_tokens::prepare_refresh_token(db.key(), s2).unwrap();
    let audited = rotate_refresh_token(&db, hash, client_id, Utc::now(), p2)
        .await
        .expect("replay");
    match audited.into_inner() {
        T04Outcome::TheftDetected { family_revoked } => {
            assert!(family_revoked >= 1, "at least the winner's successor");
        }
        T04Outcome::Rotated { .. } => panic!("expected TheftDetected"),
    }

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.refresh.theft_detected")
    );

    let active: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM refresh_tokens \
                     WHERE family_id = ?1 AND revoked_at IS NULL",
                    [family.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(active, 0, "reuse must close the whole family");
}

#[tokio::test]
async fn t04_injected_failure_before_append_rolls_back_revoke_and_successor_insert() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;
    let before_audit = latest_audit_action(&db).await;

    let successor = a_successor(user_id, client_id, family);
    let successor_id = successor.id.clone();
    let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

    db.fault_injector().fail_before_next_append();
    let result = rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), prepared).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The old row must still be active -- the guarded revoke rolled
    // back with everything else in this transaction.
    assert!(
        matches!(
            repos::refresh_tokens::begin_rotation(&db, &hash, &client_id, Utc::now())
                .await
                .expect("old token must still be rotatable"),
            repos::refresh_tokens::RotationLookup::RotatedHere(_)
        ),
        "old row must still be active: the revoke rolled back"
    );
    assert!(
        db.with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM refresh_tokens WHERE id = ?1",
                    [successor_id.as_str()],
                    |r| r.get::<_, i64>(0),
                )
                .is_err())
        })
        .unwrap(),
        "successor must not have been inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn t04_injected_commit_failure_rolls_back_everything() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;
    let before_audit = latest_audit_action(&db).await;

    let successor = a_successor(user_id, client_id, family);
    let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

    db.fail_next_commit_for_test();
    let result = rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), prepared).await;
    assert!(
        result.is_err(),
        "a rejected commit must surface as Err, not a silently-empty Ok"
    );

    assert!(
        matches!(
            repos::refresh_tokens::begin_rotation(&db, &hash, &client_id, Utc::now())
                .await
                .expect("old token must still be rotatable"),
            repos::refresh_tokens::RotationLookup::RotatedHere(_)
        ),
        "old row must still be active after a rejected commit"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

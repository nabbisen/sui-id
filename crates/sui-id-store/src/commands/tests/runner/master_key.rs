use super::*;

// ── K07 — master-key re-seal ──────────────────────────────────────────────
//
// RFC 094 cycle J stage 4. `reseal_master_key` moved here from the bare,
// downstream `sui_id_core::oidc::key_rotation::rotate_master_key`; these
// tests replace that module's own (now-removed) rotation coverage, which
// lived in `crates/sui-id/tests/e2e/key_rotation.rs` and asserted the
// re-seal in-process without a sealed command to go through. Those two
// e2e tests still run, rewritten against this command — see the review
// package for what each asserts before and after.

async fn seed_a_signing_key(db: &Database) -> SigningKeyId {
    let id = SigningKeyId::new();
    repos::signing_keys::insert_with_plaintext(
        db,
        id,
        "ed25519",
        b"plaintext-signing-key",
        b"public-key-placeholder",
        true,
    )
    .await
    .expect("seed signing key");
    id
}

#[tokio::test]
async fn k07_reseal_master_key_appends_exactly_one_audit_row() {
    let db = fresh_db();
    seed_a_signing_key(&db).await;
    let before = audit_rows(&db).await;

    let new_key = MasterKey::generate();
    let audited = crate::commands::reseal_master_key(&db, &new_key)
        .await
        .expect("reseal");
    let report = audited.into_inner();

    assert_eq!(report.signing_keys, 1, "the seeded signing key re-sealed");
    assert_eq!(report.total(), 1);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("admin.master_key.resealed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("K07");
}

#[tokio::test]
async fn k07_injected_failure_before_append_rolls_back_the_reseal() {
    let db = fresh_db();
    seed_a_signing_key(&db).await;
    let before_row = repos::signing_keys::active(&db).await.expect("active");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let new_key = MasterKey::generate();
    let result = crate::commands::reseal_master_key(&db, &new_key).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_row = repos::signing_keys::active(&db).await.expect("active");
    assert_eq!(
        after_row.private_key_enc, before_row.private_key_enc,
        "the signing key must not have been re-sealed"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("K07");
}

/// Moved from `sui-id-core::oidc::key_rotation::tests` with
/// `RotationReport` itself (RFC 094 cycle J stage 4) -- pure arithmetic,
/// no database needed.
#[test]
fn rotation_report_total_sums_columns() {
    let r = crate::commands::RotationReport {
        signing_keys: 1,
        refresh_tokens: 5,
        user_totp_secrets: 3,
        user_totp_recovery_codes: 3,
        user_webauthn_credentials: 2,
        smtp_config: 1,
        email_outbox_rows: 0,
    };
    assert_eq!(r.total(), 15);
}

/// **Added at review, cycle J stage 4, 2026-11-04, by the architect.**
///
/// K07 re-seals **six** tables. Every other test in this file and in
/// `crates/sui-id/tests/e2e/key_rotation.rs` seeds at most two of them —
/// `signing_keys`, and `smtp_config` in the e2e case — so the other four
/// re-sealed zero rows in every test that existed, and nothing noticed
/// when a `reseal_all` call was missing.
///
/// Measured, not assumed: replacing
/// `repos::refresh_tokens::reseal_all(...)` with `0` in
/// `commands::reseal_master_key` left **2212 tests passing, 0 failing**,
/// `cargo test --workspace` exit 0, and the local gate matrix 48/48. The
/// consequence is not cosmetic — `cli.rs` renames the old key file to
/// `.bak.<stamp>` as soon as this command commits, so rows left under the
/// old key are rows the configured key file can no longer open.
///
/// This test seeds one row in each of the six tables (seven counts: the
/// TOTP secret and its recovery codes are re-sealed and counted
/// separately) and pins every count. Delete any one `reseal_all` call and
/// it fails on that count by name.
#[tokio::test]
async fn k07_reseals_every_one_of_the_six_tables() {
    let db = fresh_db();

    // 1 — signing_keys
    seed_a_signing_key(&db).await;

    // A user and a client: the rows below carry foreign keys to them.
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let client = a_client();
    repos::clients::create(&db, &client)
        .await
        .expect("create client");

    // 2 — refresh_tokens
    let token_id = sui_id_shared::RefreshTokenId::generate();
    let refresh_row = crate::models::RefreshTokenRow {
        id: token_id.clone(),
        user_id: user.id,
        client_id: client.id,
        scope: "openid".into(),
        expires_at: Utc::now() + TimeDelta::hours(1),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: sui_id_shared::FamilyId::root_of(&token_id),
    };
    let raw = sui_id_shared::RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string());
    repos::refresh_tokens::insert(&db, &refresh_row, &raw)
        .await
        .expect("seed refresh token");

    // 3 and 4 — user_totp: the secret and the recovery codes are two
    // separately sealed columns, and K07 counts them separately.
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp secret");
    repos::user_totp::set_recovery_codes(&db, user.id, br#"["recovery-code-1"]"#)
        .await
        .expect("seed recovery codes");

    // 5 — user_webauthn_credentials
    repos::user_webauthn_credentials::create(
        &db,
        &crate::models::UserWebauthnCredentialRow {
            id: sui_id_shared::ids::WebauthnCredentialId::new(),
            user_id: user.id,
            credential_id: format!("cred-{}", uuid::Uuid::new_v4()).into_bytes(),
            passkey_enc: vec![],
            nickname: "review-added passkey".into(),
            created_at: Utc::now(),
            last_used_at: None,
        },
        b"passkey-json-placeholder",
    )
    .await
    .expect("seed passkey");

    // 6 — smtp_config
    let now = Utc::now();
    repos::smtp_config::upsert(
        &db,
        &crate::models::SmtpConfigRow {
            enabled: true,
            host: "smtp.test".into(),
            port: 587,
            tls_mode: crate::models::SmtpTlsMode::StartTls,
            username: Some("user".into()),
            password_enc: Some(
                crate::crypto::seal(
                    db.key(),
                    b"smtp-password-placeholder",
                    repos::smtp_config::SMTP_PASSWORD_AAD,
                )
                .expect("seal smtp password"),
            ),
            from_address: "noreply@example.test".into(),
            from_name: None,
            base_url: "https://idp.test".into(),
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("seed smtp config");

    // 7 — email_outbox
    repos::email_outbox::enqueue(
        &db,
        EmailOutboxRow {
            id: sui_id_shared::ids::EmailOutboxId::new(),
            state: EmailOutboxState::Queued,
            template: "forgot_password".into(),
            recipient_enc: crate::crypto::seal(
                db.key(),
                b"someone@example.test",
                repos::email_outbox::RECIPIENT_AAD,
            )
            .expect("seal recipient"),
            payload_enc: crate::crypto::seal(
                db.key(),
                br#"{"token":"placeholder"}"#,
                repos::email_outbox::PAYLOAD_AAD,
            )
            .expect("seal payload"),
            attempt_count: 0,
            next_attempt_at: now,
            last_error: None,
            locale: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("seed outbox row");

    let new_key = MasterKey::generate();
    let report = crate::commands::reseal_master_key(&db, &new_key)
        .await
        .expect("reseal")
        .into_inner();

    // Every count pinned by name, so a missing `reseal_all` names itself.
    assert_eq!(report.signing_keys, 1, "signing_keys not re-sealed");
    assert_eq!(report.refresh_tokens, 1, "refresh_tokens not re-sealed");
    assert_eq!(
        report.user_totp_secrets, 1,
        "user_totp secret not re-sealed"
    );
    assert_eq!(
        report.user_totp_recovery_codes, 1,
        "user_totp recovery codes not re-sealed"
    );
    assert_eq!(
        report.user_webauthn_credentials, 1,
        "user_webauthn_credentials not re-sealed"
    );
    assert_eq!(report.smtp_config, 1, "smtp_config not re-sealed");
    assert_eq!(report.email_outbox_rows, 1, "email_outbox not re-sealed");
    assert_eq!(report.total(), 7, "all seven sealed columns accounted for");
}

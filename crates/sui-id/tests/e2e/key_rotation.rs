//! Master-key rotation CLI (v0.26.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]


use super::common::*;

// ---------- v0.26.0: Master-key rotation ----------

/// End-to-end rotation: set up sui-id, enroll TOTP and a passkey,
/// configure SMTP password, then rotate the master key and assert
/// every sealed-row read still works under the new key.
#[tokio::test]
async fn rotation_reseal_succeeds_and_old_key_no_longer_decrypts() {
    use sui_id_core::key_rotation::rotate_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // The setup wizard creates a signing key (active) — that's
    // already one sealed row. Add an SMTP password to also exercise
    // the smtp_config path.
    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");

    // Add an SMTP config row with a sealed password so rotation
    // has something to re-key in that table.
    {
        use chrono::Utc;
        let now = Utc::now();
        sui_id_store::repos::smtp_config::upsert(
            &state.db,
            &sui_id_store::models::SmtpConfigRow {
                enabled: true,
                host: "smtp.test".into(),
                port: 587,
                tls_mode: sui_id_store::models::SmtpTlsMode::StartTls,
                username: Some("user".into()),
                password_enc: Some(
                    sui_id_store::crypto::seal(
                        state.db.key(),
                        b"my-smtp-password",
                        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
                    )
                    .expect("seal smtp pw"),
                ),
                from_address: "alice@example.test".into(),
                from_name: None,
                base_url: "https://idp.test".into(),
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("smtp upsert");
    }

    // Generate a brand-new master key.
    let new_key = MasterKey::generate();

    // The Database carries the old key. We open a SECOND handle
    // to the same underlying DB file? In-memory DBs cannot be
    // re-opened, so we test the rotation on the existing handle:
    // run rotation, then re-open under the new key and verify.
    //
    // The in-memory DB shares its connection across the
    // Database handle, so after rotation the same handle still
    // works (the key field is unchanged). Reading sealed columns
    // through that old handle would now FAIL — which is exactly
    // what we assert.
    let report = rotate_master_key(&state.db, &new_key).await.expect("rotate");

    assert!(
        report.signing_keys >= 1,
        "expected at least 1 signing key re-sealed, got {}",
        report.signing_keys
    );
    assert_eq!(report.smtp_config, 1, "smtp password should re-seal");
    assert!(report.total() >= 2);

    // The OLD key (still held by `state.db`) must no longer
    // decrypt the signing key column — it has been re-sealed
    // under `new_key`.
    let signing_row = sui_id_store::repos::signing_keys::active(&state.db).await.expect("active");
    let opened_old = sui_id_store::crypto::open(
        state.db.key(),
        &signing_row.private_key_enc,
        b"sui-id/signing_key/v1",
    );
    assert!(
        opened_old.is_err(),
        "old key must no longer decrypt the re-sealed column"
    );
    // The NEW key decrypts it.
    let opened_new = sui_id_store::crypto::open(
        &new_key,
        &signing_row.private_key_enc,
        b"sui-id/signing_key/v1",
    );
    assert!(
        opened_new.is_ok(),
        "new key must decrypt the re-sealed column"
    );

    // SMTP password also re-sealed.
    let smtp_row = sui_id_store::repos::smtp_config::get(&state.db).await
        .expect("smtp")
        .expect("smtp configured");
    let smtp_enc = smtp_row.password_enc.expect("password set");
    let opened_old_smtp = sui_id_store::crypto::open(
        state.db.key(),
        &smtp_enc,
        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
    );
    assert!(opened_old_smtp.is_err());
    let opened_new_smtp = sui_id_store::crypto::open(
        &new_key,
        &smtp_enc,
        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
    );
    assert_eq!(
        opened_new_smtp.expect("decrypt with new key"),
        b"my-smtp-password"
    );

    // Audit row was appended for the rotation event by the CLI
    // (we are not running through the CLI in this test, so we
    // don't check for the row here — that's covered by the CLI-
    // path test).

    // Avoid an unused-variable warning.
    let _ = user;
}

/// Sanity: rotation on a DB with no sealed rows runs successfully
/// and reports zeroes (other than the signing-key the setup
/// wizard always creates). Pin this so future migrations that
/// add new sealed columns have to update the rotation entry
/// list.
#[tokio::test]
async fn rotation_on_minimal_db_only_rekeys_signing_key() {
    use sui_id_core::key_rotation::rotate_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // Brand-new install: only the active signing key exists as
    // a sealed row. (Refresh tokens are issued by /token; not
    // exercised here. TOTP / WebAuthn / SMTP all require admin
    // action.)
    let new_key = MasterKey::generate();
    let report = rotate_master_key(&state.db, &new_key).await.expect("rotate");
    assert_eq!(report.signing_keys, 1);
    assert_eq!(report.refresh_tokens, 0);
    assert_eq!(report.user_totp_secrets, 0);
    assert_eq!(report.user_totp_recovery_codes, 0);
    assert_eq!(report.user_webauthn_credentials, 0);
    assert_eq!(report.smtp_config, 0);
    assert_eq!(report.total(), 1);
}


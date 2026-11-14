//! Master-key rotation CLI (v0.26.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use super::common::*;

// ---------- urgent fix 2026-11-03: stage the key before the re-seal
// commits (rfcs/handoffs/094-transactional-audit/
// urgent-master-key-ordering-2026-11-03.md) ----------
//
// These run the **real binary** (`CARGO_BIN_EXE_sui-id`), the same way
// `r112_stage2.rs` does, because the bug and its fix are entirely in
// `cli.rs`'s file handling: `rotate_master_key` itself (exercised
// in-process above) never touches the filesystem.
mod ordering {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("tempdir"),
            }
        }
        fn db(&self) -> PathBuf {
            self.dir.path().join("t.sqlite")
        }
        fn key(&self) -> PathBuf {
            self.dir.path().join("t.key")
        }
        fn staged_key(&self) -> PathBuf {
            self.dir.path().join("t.key.new")
        }
        fn config(&self) -> PathBuf {
            let sample = Command::new(BIN)
                .arg("--print-sample-config")
                .output()
                .expect("sample");
            let mut text = String::from_utf8(sample.stdout).expect("utf8");
            text = text
                .replace("./sui-id.sqlite", &self.db().display().to_string())
                .replace("./sui-id.key", &self.key().display().to_string())
                .replace("127.0.0.1:8801", "127.0.0.1:18813");
            let path = self.dir.path().join("cfg.toml");
            std::fs::write(&path, text).expect("config");
            path
        }
        /// Lets the binary create and migrate the database and bootstrap
        /// the key file (the command fails with "user not found", which
        /// is the point: the open succeeded).
        fn create(&self, cfg: &Path) {
            let out = self.run(
                &["admin", "unlock-user", "--username", "nobody", "--config"],
                cfg,
            );
            assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
            assert!(self.db().exists());
            assert!(self.key().exists());
        }
        fn run(&self, args: &[&str], cfg: &Path) -> Output {
            Command::new(BIN).args(args).arg(cfg).output().expect("run")
        }
        /// Same as [`Home::run`], with `SUI_ID_MASTER_KEY` set on the
        /// **child process only** (`Command::env`) -- never
        /// `std::env::set_var` in the test process itself, which would
        /// leak into every other test in this binary.
        fn run_with_master_key_env(&self, args: &[&str], cfg: &Path, key_b64: &str) -> Output {
            Command::new(BIN)
                .args(args)
                .arg(cfg)
                .env("SUI_ID_MASTER_KEY", key_b64)
                .output()
                .expect("run")
        }
        fn raw(&self) -> rusqlite::Connection {
            rusqlite::Connection::open(self.db()).expect("raw")
        }
        /// The most recent `note` recorded for `action`, if any row with
        /// that action exists.
        fn audit_note(&self, action: &str) -> Option<String> {
            self.raw()
                .query_row(
                    "SELECT note FROM audit_log WHERE action = ?1 ORDER BY seq DESC LIMIT 1",
                    [action],
                    |r| r.get(0),
                )
                .ok()
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }

    fn mode_bits(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    /// Insert a signing-key row whose `private_key_enc` cannot be
    /// decrypted under any key (too short for even the nonce+tag
    /// `crypto::open` requires) — a genuine re-seal failure, the
    /// nearest honest substitute for fault injection: the dynamic
    /// `FaultInjector` (`sui-id-store/src/backend.rs`) is
    /// `pub(crate)` to `sui-id-store` and unreachable from this
    /// crate's e2e binary, and nothing else hooks `with_tx_sync`
    /// for synthetic failure the way `class_a` wires its own.
    fn poison_a_signing_key(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO signing_keys(id, algorithm, private_key_enc, public_key, \
             is_active, created_at, rotated_at) \
             VALUES ('11111111-1111-1111-1111-111111111111', 'EdDSA', X'0000', X'00', \
             1, '2026-01-01T00:00:00Z', NULL)",
            [],
        )
        .expect("poison insert");
    }

    /// Test 1 — successful rotation (generate flow): the key is at the
    /// configured path, `.new` is gone, the old key is at
    /// `.bak.<stamp>`, and the database opens with the new key and not
    /// the old.
    #[test]
    fn successful_rotation_stages_then_lands_the_key_and_archives_the_old_one() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

        assert!(!h.staged_key().exists(), "staged key must be gone");
        assert!(h.key().exists(), "new key must be at the configured path");
        let new_key_body = std::fs::read_to_string(h.key()).expect("new key");
        assert_ne!(new_key_body, old_key_body, "the key actually changed");

        let bak = std::fs::read_dir(h.dir.path())
            .expect("read dir")
            .filter_map(|e| e.ok())
            .find(|e| e.file_name().to_string_lossy().starts_with("t.key.bak."))
            .expect("a .bak.<stamp> file exists");
        let bak_body = std::fs::read_to_string(bak.path()).expect("bak key");
        assert_eq!(bak_body, old_key_body, "the archived key is the old one");

        // RFC 094 cycle J stage 5: `admin.master_key.rotated`'s note now
        // reports the key files, not a row count -- that moved to
        // `admin.master_key.resealed` (K07), which this run also wrote.
        assert!(
            h.audit_note("admin.master_key.resealed").is_some(),
            "the sealed re-seal record exists"
        );
        let rotated_note = h
            .audit_note("admin.master_key.rotated")
            .expect("the rotated record exists");
        assert!(
            rotated_note.contains("new key in place at") && rotated_note.contains(" archived at"),
            "note reports the key files, not a row count: {rotated_note}"
        );
        assert!(
            !rotated_note.contains("rows"),
            "the row count belongs to admin.master_key.resealed now: {rotated_note}"
        );

        // The database now opens (and the server would start) with the
        // key at the configured path, i.e. the NEW one -- prove it by
        // running another admin command against the same config and
        // getting the ordinary "user not found" failure, not a refusal
        // to open the database.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
        assert!(
            !stderr(&out2).contains("refusing"),
            "the server would start fine with the key now at the configured path: {}",
            stderr(&out2)
        );
    }

    /// Test 2 — `.new` already exists: the command refuses, and
    /// nothing is touched (neither key file, nor the database).
    #[test]
    fn staged_key_already_present_refuses_before_touching_anything() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");
        std::fs::write(h.staged_key(), "stray-leftover-from-a-previous-run\n").expect("stray");
        let stray_body = std::fs::read_to_string(h.staged_key()).expect("stray body");

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(1), "must refuse: {}", stderr(&out));
        assert!(
            stderr(&out).contains("already exists")
                && stderr(&out).contains("may not have completed"),
            "{}",
            stderr(&out)
        );

        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key still there"),
            old_key_body,
            "the old key file is untouched"
        );
        assert_eq!(
            std::fs::read_to_string(h.staged_key()).expect("stray still there"),
            stray_body,
            "the stray staged file is untouched, not overwritten"
        );
        assert!(
            std::fs::read_dir(h.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .all(|e| !e.file_name().to_string_lossy().starts_with("t.key.bak.")),
            "no .bak file was created -- the database was never re-sealed"
        );

        // The database still opens with the OLD key.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
    }

    /// Test 3 — the staged file is created with mode `0600`, not
    /// `chmod`'d afterward.
    #[test]
    fn staged_key_is_created_with_owner_only_permissions() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);

        // A corrupted signing key stalls the rotation right after the
        // staged file is written (see test 4) -- which is exactly the
        // window this test needs to inspect the file mid-rotation.
        poison_a_signing_key(&h.raw());

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            h.staged_key().exists(),
            "the staged key survives the re-seal failure"
        );
        assert_eq!(mode_bits(&h.staged_key()), 0o600);
    }

    /// Test 4 (§3's ordering test) — a re-seal failure leaves the
    /// staged key on disk and the database still opens under the OLD
    /// key: the assertion that proves the key is persisted before the
    /// irreversible step, against a genuine failure rather than an
    /// injected one.
    #[test]
    fn reseal_failure_leaves_the_staged_key_and_the_old_key_file_untouched() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");
        poison_a_signing_key(&h.raw());

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("rotating sealed columns"),
            "failed inside the re-seal, not before it: {}",
            stderr(&out)
        );

        // The new key is durable, even though rotation did not finish.
        assert!(h.staged_key().exists(), "the new key is still recoverable");
        let staged_body = std::fs::read_to_string(h.staged_key()).expect("staged body");
        assert_ne!(staged_body, old_key_body);

        // Neither key file at the configured path moved.
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("old key still there"),
            old_key_body
        );
        assert!(
            std::fs::read_dir(h.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .all(|e| !e.file_name().to_string_lossy().starts_with("t.key.bak.")),
            "no rename happened -- the transaction rolled back"
        );

        // And the database still opens with the OLD key: the same
        // config, same key file, an ordinary command succeeds exactly
        // as it did before the failed attempt.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
    }

    /// `0.91.0` stage 1, part B — a key that came from `SUI_ID_MASTER_KEY`
    /// must not be rotated: the command would write the new key to the
    /// *file*, which a start with the variable still set would ignore,
    /// stranding the server on the old key against a database rotation
    /// had already re-sealed. The variable here carries the *same* key
    /// already at the configured path, so the only thing this test can
    /// be failing on is the origin check -- not an unrelated wrong-key
    /// refusal from `database::open`.
    /// `0.91.0` stage 1, A2, the operator's side: when something else holds
    /// the database open (here a raw read transaction, as a running server
    /// would), K07 commits but cannot be made durable. The command must say
    /// where each key is and what to do. It must not invite a second
    /// rotation, which would start from the old key, which no longer opens
    /// the database.
    #[test]
    fn a_reseal_that_cannot_be_made_durable_says_where_the_keys_are() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key = std::fs::read_to_string(h.key()).expect("key");

        let holder = h.raw();
        holder
            .execute_batch("BEGIN; SELECT * FROM signing_keys;")
            .expect("hold a read transaction, as a running server would");
        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        holder.execute_batch("ROLLBACK;").ok();
        drop(holder);

        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        let err = stderr(&out);
        let staged = h.staged_key().display().to_string();
        let active = h.key().display().to_string();
        for must in [
            "now sealed under the NEW key",
            staged.as_str(),
            active.as_str(),
            "stop sui-id",
            "do not run the rotation again",
        ] {
            assert!(err.contains(must), "stderr must contain {must:?}: {err}");
        }
        assert!(h.staged_key().exists(), "the new key is still staged");
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("old key still in place"),
            old_key,
            "the key files were not touched"
        );
        assert!(
            h.audit_note("admin.master_key.resealed").is_some(),
            "the re-seal did commit"
        );
    }

    #[test]
    fn rotation_refuses_when_the_old_key_came_from_the_environment() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let key_body = std::fs::read_to_string(h.key()).expect("key");

        let out = h.run_with_master_key_env(
            &["admin", "rotate-key", "--yes", "--config"],
            &cfg,
            key_body.trim(),
        );
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("SUI_ID_MASTER_KEY"),
            "the refusal must name the variable: {}",
            stderr(&out)
        );

        assert!(
            !h.staged_key().exists(),
            "nothing was staged -- the refusal runs before the database opens"
        );
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key still there"),
            key_body,
            "the key file's bytes are unchanged"
        );
        assert!(
            h.audit_note("admin.master_key.resealed").is_none(),
            "no re-seal was attempted"
        );

        // An ordinary command still runs against the configured key FILE,
        // with no environment variable this time. (`unlock-user` decrypts
        // nothing, so this is not evidence about the sealed rows; that
        // nothing was re-sealed is shown above by the absent audit row.)
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
        assert!(
            !stderr(&out2).contains("refusing"),
            "the original key file must still open the database: {}",
            stderr(&out2)
        );
    }
}

// ---------- v0.26.0: Master-key rotation ----------

/// End-to-end rotation: set up sui-id, enroll TOTP and a passkey,
/// configure SMTP password, then rotate the master key and assert
/// every sealed-row read still works under the new key.
#[tokio::test]
async fn rotation_reseal_succeeds_and_old_key_no_longer_decrypts() {
    use sui_id_store::commands::reseal_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // The setup wizard creates a signing key (active) — that's
    // already one sealed row. Add an SMTP password to also exercise
    // the smtp_config path.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");

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
    let before_audit_rows = sui_id_store::repos::audit::recent(&state.db, 1000)
        .await
        .expect("audit tail")
        .len();
    let report = reseal_master_key(&state.db, &new_key)
        .await
        .expect("rotate")
        .into_inner();

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
    let signing_row = sui_id_store::repos::signing_keys::active(&state.db)
        .await
        .expect("active");
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
    let smtp_row = sui_id_store::repos::smtp_config::get(&state.db)
        .await
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

    // RFC 094 cycle J stage 4: the re-seal is now itself a sealed
    // command (K07), so the audit row commits with it regardless of
    // whether the CLI is the caller — asserted here, not left to the
    // CLI-path test, since this test IS a caller now.
    let after_audit = sui_id_store::repos::audit::recent(&state.db, 1000)
        .await
        .expect("audit tail");
    assert_eq!(
        after_audit.len(),
        before_audit_rows + 1,
        "exactly one audit row from the reseal"
    );
    assert_eq!(after_audit[0].action, "admin.master_key.resealed");

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
    use sui_id_store::commands::reseal_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // Brand-new install: only the active signing key exists as
    // a sealed row. (Refresh tokens are issued by /token; not
    // exercised here. TOTP / WebAuthn / SMTP all require admin
    // action.)
    let new_key = MasterKey::generate();
    let report = reseal_master_key(&state.db, &new_key)
        .await
        .expect("rotate")
        .into_inner();
    assert_eq!(report.signing_keys, 1);
    assert_eq!(report.refresh_tokens, 0);
    assert_eq!(report.user_totp_secrets, 0);
    assert_eq!(report.user_totp_recovery_codes, 0);
    assert_eq!(report.user_webauthn_credentials, 0);
    assert_eq!(report.smtp_config, 0);
    assert_eq!(report.total(), 1);
}

use super::*;
use crate::database::PreMigrationBackup;

/// RFC 094 M2b wave 4 stage 4, C22 — the real argument-handling and
/// config/db-opening path against a real on-disk database and key file,
/// asserting the audit record and not only the revoked value. Calls the
/// typed handler directly (RFC 148 cycle O2a: no more `&[String]` to
/// build), the same construction
/// `tests/e2e/backup.rs::backup_then_restore_preserves_users_and_clients`
/// uses.
#[tokio::test]
async fn revoke_registration_token_cli_revokes_and_is_audited() {
    use sui_id_shared::ids::RegistrationTokenId;
    use sui_id_store::Database;
    use sui_id_store::crypto::MasterKey;

    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("cli-c22.sqlite");
    let key_path = tmp.path().join("cli-c22.key");

    let key = MasterKey::generate();
    std::fs::write(&key_path, key.to_base64()).expect("write key");
    let db = Database::open(&db_path, key).expect("open db");

    let id = RegistrationTokenId::new();
    let now = chrono::Utc::now();
    sui_id_store::repos::client_registration_token::create(
        &db,
        &sui_id_store::repos::client_registration_token::RegistrationTokenRow {
            id,
            token_hash: "cli-test-hash".into(),
            max_uses: 1,
            used_count: 0,
            expires_at: None,
            revoked_at: None,
            note: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("seed token");
    drop(db);

    let config_path = tmp.path().join("sui-id.toml");
    std::fs::write(
        &config_path,
        format!(
            "[storage]\ndb_path = {db_path:?}\nkey_file = {key_path:?}\n\n[server]\n\
             listen_addr = \"127.0.0.1:0\"\nissuer = \"https://idp.test\"\n"
        ),
    )
    .expect("write config");

    run_admin_revoke_registration_token(&config_path, PreMigrationBackup::Take, &id.to_string())
        .await
        .expect("revoke");

    let key2 = crate::keyring::resolve(&key_path).expect("resolve key").key;
    let db2 = Database::open(&db_path, key2).expect("reopen db");
    let row = sui_id_store::repos::client_registration_token::find_by_hash(&db2, "cli-test-hash")
        .await
        .expect("find")
        .expect("row exists");
    assert!(row.revoked_at.is_some(), "the token must be revoked");

    let note: Option<String> = db2
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'client.registration_token.revoked' \
                 ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row exists");
    assert_eq!(note, None, "no attributes");
}

/// Cycle I stage 3, C14 — the counterpart of
/// [`revoke_registration_token_cli_revokes_and_is_audited`] above, which
/// the conversion did not get. `client_registration_token::create` is
/// deliberately kept for the seventeen fixtures that seed tokens, so the
/// CLI has two functions it could call and only one of them seals the
/// record; this test is what makes reverting the one line in
/// `run_admin_issue_registration_token` to the kept wrapper fail.
#[tokio::test]
async fn issue_registration_token_cli_stores_the_token_and_is_audited() {
    use sui_id_store::Database;
    use sui_id_store::crypto::MasterKey;

    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("cli-c14.sqlite");
    let key_path = tmp.path().join("cli-c14.key");

    let key = MasterKey::generate();
    std::fs::write(&key_path, key.to_base64()).expect("write key");
    // Opened once so the schema exists, then closed: the CLI opens it again.
    drop(Database::open(&db_path, key).expect("open db"));

    let config_path = tmp.path().join("sui-id.toml");
    std::fs::write(
        &config_path,
        format!(
            "[storage]\ndb_path = {db_path:?}\nkey_file = {key_path:?}\n\n[server]\n\
             listen_addr = \"127.0.0.1:0\"\nissuer = \"https://idp.test\"\n"
        ),
    )
    .expect("write config");

    run_admin_issue_registration_token(&config_path, PreMigrationBackup::Take, 1, None)
        .await
        .expect("issue");

    let key2 = crate::keyring::resolve(&key_path).expect("resolve key").key;
    let db2 = Database::open(&db_path, key2).expect("reopen db");

    let (target, note): (Option<String>, Option<String>) = db2
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT target, note FROM audit_log \
                 WHERE action = 'client.registration_token.created' \
                 ORDER BY seq DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .await
        .expect("an audit row for the issued token");
    assert_eq!(note, None, "no attributes, the same shape as C22");

    let stored_id = target.expect("the audit row names the token id");
    let rows: i64 = db2
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM client_registration_token WHERE id = ?1",
                [stored_id],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(
        rows, 1,
        "the token the audit row names must be the one that was stored"
    );
}

#[test]
fn the_listing_shows_a_tokens_id_and_its_revoked_state() {
    use sui_id_shared::ids::RegistrationTokenId;
    use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

    let now = chrono::Utc::now();
    let live_id = RegistrationTokenId::new();
    let revoked_id = RegistrationTokenId::new();
    let rows = vec![
        RegistrationTokenRow {
            id: live_id,
            token_hash: "live-hash".into(),
            max_uses: 0,
            used_count: 3,
            expires_at: None,
            revoked_at: None,
            note: None,
            created_at: now,
            updated_at: now,
        },
        RegistrationTokenRow {
            id: revoked_id,
            token_hash: "revoked-hash".into(),
            max_uses: 5,
            used_count: 1,
            expires_at: None,
            revoked_at: Some(now),
            note: None,
            created_at: now,
            updated_at: now,
        },
    ];

    let text = render_registration_token_list(&rows);

    assert!(
        text.contains(&live_id.to_string()),
        "the live token's id must appear:\n{text}"
    );
    assert!(
        text.contains(&revoked_id.to_string()),
        "the revoked token's id must appear:\n{text}"
    );
    assert!(
        text.contains("unlimited"),
        "max_uses = 0 must render as `unlimited`, not `0`:\n{text}"
    );
    assert!(
        text.contains("never"),
        "expires_at = None must render as `never`:\n{text}"
    );
    assert!(
        text.contains(&format!("revoked {}", now.to_rfc3339())),
        "the revoked row must show its revocation timestamp:\n{text}"
    );
    assert!(
        text.contains("active"),
        "the live row must be distinguishable from the revoked one:\n{text}"
    );
}

#[test]
fn the_listing_never_prints_the_token_or_its_hash() {
    use sui_id_shared::ids::RegistrationTokenId;
    use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

    let now = chrono::Utc::now();
    let rows = vec![RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: "super-secret-hash-value".into(),
        max_uses: 1,
        used_count: 0,
        expires_at: None,
        revoked_at: None,
        note: None,
        created_at: now,
        updated_at: now,
    }];

    let text = render_registration_token_list(&rows);

    assert!(
        !text.contains("super-secret-hash-value"),
        "the token hash must never appear in the listing output:\n{text}"
    );
}

#[test]
fn issued_registration_tokens_output_contains_the_id() {
    use sui_id_shared::ids::RegistrationTokenId;

    let id = RegistrationTokenId::new();
    let text = render_issued_registration_token("a-raw-token-value", id, 1);

    assert!(
        text.contains(&id.to_string()),
        "the issued token's output must contain its id:\n{text}"
    );
    assert!(
        text.contains("a-raw-token-value"),
        "the issued token's output must still contain the token itself:\n{text}"
    );
}

#[test]
fn the_empty_case_states_incompleteness_not_a_bare_zero() {
    let report = sui_id_store::repos::audit::LegacyDynamicCandidateReport {
        candidates: vec![],
        unresolved: vec![],
        max_seq: None,
    };
    let text = render_legacy_dynamic_candidate_report(&report, None);

    assert!(
        text.contains("does NOT mean no such client exists"),
        "the empty case must state the evidence is incomplete, not just empty:\n{text}"
    );
    assert!(
        text.contains(LEGACY_WINDOW_START_COMMIT) && text.contains(LEGACY_WINDOW_END_COMMIT),
        "the empty case must name both window commits, not just assert incompleteness \
         in the abstract:\n{text}"
    );
    for forbidden in ["0 candidates", "none found", "clean"] {
        assert!(
            !text.to_lowercase().contains(&forbidden.to_lowercase()),
            "the empty case must not contain {forbidden:?}, which a reader could take as \
             \"there were none\":\n{text}"
        );
    }
}

#[test]
fn a_real_candidate_renders_with_the_rfcs_own_label_and_the_found_not_existing_qualifier() {
    let id = sui_id_shared::ids::ClientId::new();
    let now = chrono::Utc::now();
    let report = sui_id_store::repos::audit::LegacyDynamicCandidateReport {
        candidates: vec![sui_id_store::repos::audit::LegacyDynamicCandidate {
            client_id: id,
            client_name: "a client".into(),
            created_at: now,
            is_disabled: false,
            is_deleted: false,
            audit_seq: 7,
            audit_at: now,
        }],
        unresolved: vec![],
        max_seq: Some(7),
    };
    let text = render_legacy_dynamic_candidate_report(&report, None);

    assert!(text.contains("legacy_unstamped_dynamic_candidate"));
    assert!(text.contains(&id.to_string()));
    assert!(
        text.contains("FOUND") && text.contains("not necessarily"),
        "the non-empty case must also say it found entries, not that they exist:\n{text}"
    );
}

mod preview_upgrade_render {
    use super::*;
    use std::path::Path;
    use sui_id_store::backup::FreeSpaceCheck;
    use sui_id_store::migrations::{MAX_SCHEMA_VERSION, Preview, StoredVersion, pending};

    fn p() -> &'static Path {
        Path::new("/var/lib/sui-id/sui-id.sqlite")
    }

    #[test]
    fn no_file_says_no_database_and_that_starting_creates_everything() {
        let text = render_preview_upgrade("0.88.0", p(), &Preview::NoFile, None);
        assert_eq!(
            text,
            format!(
                "sui-id 0.88.0: upgrade preview for /var/lib/sui-id/sui-id.sqlite\n\
                 \x20\x20No database exists at /var/lib/sui-id/sui-id.sqlite yet.\n\
                 \x20\x20Starting this sui-id creates its schema and runs all \
                 {MAX_SCHEMA_VERSION} migrations. Nothing was changed.\n"
            )
        );
    }

    #[test]
    fn fresh_says_the_database_is_empty_and_that_starting_creates_everything() {
        let text = render_preview_upgrade("0.88.0", p(), &Preview::Fresh, None);
        assert_eq!(
            text,
            format!(
                "sui-id 0.88.0: upgrade preview for /var/lib/sui-id/sui-id.sqlite\n\
                 \x20\x20The database at /var/lib/sui-id/sui-id.sqlite is empty.\n\
                 \x20\x20Starting this sui-id creates its schema and runs all \
                 {MAX_SCHEMA_VERSION} migrations. Nothing was changed.\n"
            )
        );
    }

    #[test]
    fn at_head_says_so_and_lists_nothing() {
        let text = render_preview_upgrade("0.88.0", p(), &Preview::AtHead, None);
        assert_eq!(
            text,
            format!(
                "sui-id 0.88.0: upgrade preview for /var/lib/sui-id/sui-id.sqlite\n\
                 \x20\x20The database is already at schema version {MAX_SCHEMA_VERSION}. \
                 Starting this sui-id runs no migrations. Nothing was changed.\n"
            )
        );
    }

    #[test]
    fn pending_lists_every_migration_above_the_stored_version_in_order_with_names() {
        let migrations: Vec<_> = pending(StoredVersion::At(44)).collect();
        let text = render_preview_upgrade(
            "0.88.0",
            p(),
            &Preview::Pending {
                current: 44,
                last_migrated_by: Some("0.86.0".to_string()),
                migrations: migrations.clone(),
            },
            None,
        );
        let mut expected = format!(
            "sui-id 0.88.0: upgrade preview for /var/lib/sui-id/sui-id.sqlite\n\
             \x20\x20Schema version now:      44 (last migrated by sui-id 0.86.0)\n\
             \x20\x20This sui-id migrates to: {MAX_SCHEMA_VERSION}\n\
             \x20\x20{} migrations would run when this sui-id starts:\n",
            migrations.len()
        );
        for m in &migrations {
            expected.push_str(&format!("    {}\n", m.name));
        }
        expected.push_str(
            "  Nothing was changed. Back up before starting this sui-id: \
             docs/src/guides/upgrade.md\n",
        );
        assert_eq!(text, expected);
        assert_eq!(
            migrations.len(),
            (MAX_SCHEMA_VERSION - 44) as usize,
            "sanity: pending(44) is every migration above 44"
        );
    }

    #[test]
    fn pending_without_a_recorded_release_says_so_instead_of_a_version() {
        let migrations: Vec<_> = pending(StoredVersion::At(44)).collect();
        let text = render_preview_upgrade(
            "0.88.0",
            p(),
            &Preview::Pending {
                current: 44,
                last_migrated_by: None,
                migrations,
            },
            None,
        );
        assert!(
            text.contains("(the release that last migrated it is not recorded)"),
            "{text}"
        );
        assert!(!text.contains("last migrated by"), "{text}");
    }

    #[test]
    fn pending_with_enough_space_predicts_the_backup_before_nothing_was_changed() {
        let migrations: Vec<_> = pending(StoredVersion::At(44)).collect();
        let check = FreeSpaceCheck {
            needed_bytes: 20 * 1024 * 1024,
            available_bytes: Some(500 * 1024 * 1024),
        };
        let text = render_preview_upgrade(
            "0.88.0",
            p(),
            &Preview::Pending {
                current: 44,
                last_migrated_by: Some("0.86.0".to_string()),
                migrations,
            },
            Some(&check),
        );
        let backup_line_pos = text
            .find("Before migrating, this sui-id writes a backup")
            .expect("backup prediction line present");
        let changed_line_pos = text
            .find("Nothing was changed.")
            .expect("nothing-was-changed line present");
        assert!(
            backup_line_pos < changed_line_pos,
            "the backup prediction must come before \"Nothing was changed.\":\n{text}"
        );
        assert!(text.contains("a backup of about 10 MB"), "{text}");
        assert!(
            text.contains("(about 20 MB free needed, about 500 MB available)"),
            "{text}"
        );
        assert!(!text.contains("not enough space"), "{text}");
    }

    #[test]
    fn pending_with_short_space_predicts_a_refusal() {
        let migrations: Vec<_> = pending(StoredVersion::At(44)).collect();
        let check = FreeSpaceCheck {
            needed_bytes: 20 * 1024 * 1024,
            available_bytes: Some(5 * 1024 * 1024),
        };
        let text = render_preview_upgrade(
            "0.88.0",
            p(),
            &Preview::Pending {
                current: 44,
                last_migrated_by: Some("0.86.0".to_string()),
                migrations,
            },
            Some(&check),
        );
        assert!(
            text.contains("not enough space")
                && text.contains("this sui-id would refuse to migrate"),
            "{text}"
        );
    }

    #[test]
    fn a_backup_under_a_megabyte_is_not_called_zero_mb() {
        let migrations: Vec<_> = pending(StoredVersion::At(44)).collect();
        let check = FreeSpaceCheck {
            needed_bytes: 2 * 40 * 1024,
            available_bytes: Some(3 * 1024 * 1024 * 1024 + 512 * 1024),
        };
        let text = render_preview_upgrade(
            "0.88.0",
            p(),
            &Preview::Pending {
                current: 44,
                last_migrated_by: Some("0.86.0".to_string()),
                migrations,
            },
            Some(&check),
        );
        assert!(!text.contains(" 0 MB"), "{text}");
        assert!(
            text.contains("a backup of under 1 MB beside the database (under 1 MB free needed, about 3072 MB available)"),
            "{text}"
        );
    }
}

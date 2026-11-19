use super::*;

fn argv(parts: &[&str]) -> Vec<String> {
    std::iter::once("sui-id".to_string())
        .chain(parts.iter().map(|s| s.to_string()))
        .collect()
}

fn parse(parts: &[&str]) -> Cli {
    Cli::try_parse_from(argv(parts)).unwrap_or_else(|e| panic!("{e}"))
}

// ── Every new leaf parses into the shape the dispatch expects ──────────

#[test]
fn backup_create_parses() {
    let cli = parse(&["backup", "create", "--to", "x.tar", "--encrypt"]);
    match cli.command {
        Some(Command::Backup(BackupArgs {
            command: Some(BackupCommand::Create { to, encrypt }),
            ..
        })) => {
            assert_eq!(to, PathBuf::from("x.tar"));
            assert!(encrypt);
        }
        other => panic!("expected Backup(Create), got {other:?}"),
    }
}

#[test]
fn admin_user_unlock_parses() {
    let cli = parse(&["admin", "user", "unlock", "--username", "alice"]);
    match cli.command {
        Some(Command::Admin(AdminArgs {
            command:
                AdminCommand::User(AdminUserArgs {
                    command: AdminUserCommand::Unlock { username },
                }),
        })) => assert_eq!(username, "alice"),
        other => panic!("expected Admin(User(Unlock)), got {other:?}"),
    }
}

// ── Every pre-148 alias still parses (RFC 148 §2/§3) ────────────────────

#[test]
fn backup_old_flat_form_parses() {
    let cli = parse(&["backup", "--to", "x.tar"]);
    match cli.command {
        Some(Command::Backup(BackupArgs {
            command: None,
            to: Some(to),
            ..
        })) => assert_eq!(to, PathBuf::from("x.tar")),
        other => panic!("expected Backup(flat), got {other:?}"),
    }
}

#[test]
fn restore_verify_backup_preview_upgrade_old_words_parse() {
    assert!(matches!(
        parse(&["restore", "--from", "x.tar"]).command,
        Some(Command::Restore { .. })
    ));
    assert!(matches!(
        parse(&["verify-backup", "--from", "x.tar"]).command,
        Some(Command::VerifyBackup { .. })
    ));
    assert!(matches!(
        parse(&["preview-upgrade"]).command,
        Some(Command::PreviewUpgrade)
    ));
}

#[test]
fn every_old_admin_subaction_word_parses() {
    let cases: &[&[&str]] = &[
        &["admin", "unlock-user", "--username", "a"],
        &["admin", "reset-mfa", "--username", "a", "--reason", "r"],
        &[
            "admin",
            "issue-recovery-link",
            "--username",
            "a",
            "--reason",
            "r",
        ],
        &["admin", "rotate-key"],
        &["admin", "rotate-metrics-token"],
        &["admin", "issue-registration-token"],
        &["admin", "revoke-registration-token", "--id", "x"],
        &["admin", "list-registration-tokens"],
        &["admin", "legacy-dynamic-candidates"],
    ];
    for case in cases {
        let cli = parse(case);
        assert!(
            matches!(cli.command, Some(Command::Admin(_))),
            "{case:?} did not parse as Admin(..)"
        );
    }
}

#[test]
fn old_top_level_mode_flags_still_parse() {
    let cli = parse(&["--print-sample-config"]);
    assert!(cli.print_sample_config);

    let cli = parse(&[
        "--dev",
        "--dev-bind",
        "127.0.0.1:9",
        "--dev-seed",
        "s.toml",
        "--dev-admin-password",
        "p",
        "--dev-client-secret",
        "s",
        "--dev-db",
        "d.sqlite",
    ]);
    assert!(cli.dev_flag);
    assert_eq!(cli.dev_bind.as_deref(), Some("127.0.0.1:9"));
    assert_eq!(cli.dev_seed, Some(PathBuf::from("s.toml")));
    assert_eq!(cli.dev_admin_password.as_deref(), Some("p"));
    assert_eq!(cli.dev_client_secret.as_deref(), Some("s"));
    assert_eq!(cli.dev_db, Some(PathBuf::from("d.sqlite")));
}

// ── RFC 107's own closure condition: a flag's value is never taken for a
// command, and a value that happens to spell a command name is never
// mistaken for one either -- real argument parsing, not a hand scanner,
// makes this a structural property rather than a list to keep in sync. ──

#[test]
fn a_flags_value_is_never_taken_for_a_command() {
    let cli = parse(&["--config", "backup"]);
    assert_eq!(cli.config, Some(PathBuf::from("backup")));
    assert!(
        cli.command.is_none(),
        "backup must be --config's value, not a command"
    );
}

#[test]
fn a_value_that_spells_a_command_name_is_never_mistaken_for_one() {
    let cli = parse(&["admin", "user", "unlock", "--username", "serve"]);
    match cli.command {
        Some(Command::Admin(AdminArgs {
            command:
                AdminCommand::User(AdminUserArgs {
                    command: AdminUserCommand::Unlock { username },
                }),
        })) => assert_eq!(username, "serve"),
        other => panic!("expected Admin(User(Unlock{{username: \"serve\"}})), got {other:?}"),
    }
}

#[test]
fn generate_cli_reference_is_hidden_but_parses() {
    let cli = parse(&["generate-cli-reference", "--check"]);
    assert!(matches!(
        cli.command,
        Some(Command::GenerateCliReference { check: true })
    ));
}

// ── Mutual exclusion the dispatch used to hand-check is now clap's job ──

#[test]
fn rotate_key_rejects_both_new_key_sources_at_once() {
    let err = Cli::try_parse_from(argv(&[
        "admin",
        "master-key",
        "rotate",
        "--generate-new-key",
        "--new-key",
        "x",
    ]))
    .expect_err("--generate-new-key and --new-key must conflict");
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn global_config_and_no_backup_work_after_any_subcommand() {
    let cli = parse(&[
        "admin",
        "user",
        "unlock",
        "--username",
        "a",
        "--config",
        "c.toml",
        "--no-backup",
    ]);
    assert_eq!(cli.config, Some(PathBuf::from("c.toml")));
    assert!(cli.no_backup);
}

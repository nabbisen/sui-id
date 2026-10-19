use super::*;

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

/// RFC 107: every flag `cli::VALUE_TAKING_FLAGS` names must have its
/// value skipped by `find_subcommand`, or that value is misread as
/// the subcommand -- the defect that made every value-taking
/// `--dev-*` flag fail with "unknown subcommand" before this fix.
#[test]
fn find_subcommand_skips_the_value_of_every_value_taking_flag() {
    for flag in cli::VALUE_TAKING_FLAGS {
        let args = argv(&["sui-id", flag, "some-value-not-a-subcommand"]);
        assert_eq!(
            find_subcommand(&args),
            None,
            "{flag}'s value must not be mistaken for a subcommand"
        );
    }
}

/// Mutation check for the test above: with the pre-RFC-107 three-flag
/// list, a flag outside it (`--dev-seed`) must fail this assertion --
/// otherwise the test could pass with an incomplete list.
#[test]
fn find_subcommand_test_actually_depends_on_the_flag_being_listed() {
    let args = argv(&["sui-id", "--not-a-real-flag", "some-value"]);
    assert_eq!(
        find_subcommand(&args),
        Some("some-value".to_string()),
        "a flag absent from VALUE_TAKING_FLAGS must still let its \
         next token be read as the subcommand -- confirms the test \
         above is exercising the list, not vacuously passing"
    );
}

/// RFC 107: every `--dev-*` flag that takes a value must actually be
/// honoured -- its value extracted, not silently defaulted and not
/// mistaken for a subcommand (the defect `find_subcommand_*` above
/// covers; this one is downstream of it and would not catch a
/// regression in `parse_dev_flags` alone, e.g. a flag name typo that
/// still compiles).
#[test]
fn dev_mode_honours_its_value_taking_flags() {
    let args = argv(&[
        "sui-id",
        "--dev",
        "--dev-bind",
        "10.0.0.5:9999",
        "--dev-seed",
        "/path/to/seed.toml",
        "--dev-admin-password",
        "overridden-admin-password",
        "--dev-client-secret",
        "overridden-client-secret",
        "--dev-db",
        "/path/to/dev.sqlite",
    ]);
    let flags = parse_dev_flags(&args);
    assert_eq!(flags.dev_bind, "10.0.0.5:9999", "--dev-bind must be read");
    assert_eq!(
        flags.dev_seed_path,
        Some(PathBuf::from("/path/to/seed.toml")),
        "--dev-seed must be read"
    );
    assert_eq!(
        flags.dev_admin_password.as_deref(),
        Some("overridden-admin-password"),
        "--dev-admin-password must be read"
    );
    assert_eq!(
        flags.dev_client_secret.as_deref(),
        Some("overridden-client-secret"),
        "--dev-client-secret must be read"
    );
    assert_eq!(
        flags.dev_db,
        Some(PathBuf::from("/path/to/dev.sqlite")),
        "--dev-db must be read"
    );
}

/// With none of the `--dev-*` flags given, every one of them must
/// fall back to its documented default rather than, say, an empty
/// string that happens to also satisfy the assertions above by
/// accident.
#[test]
fn dev_mode_flags_default_when_absent() {
    let args = argv(&["sui-id", "--dev"]);
    let flags = parse_dev_flags(&args);
    assert_eq!(flags.dev_bind, "127.0.0.1:8801");
    assert_eq!(flags.dev_seed_path, None);
    assert_eq!(flags.dev_admin_password, None);
    assert_eq!(flags.dev_client_secret, None);
    assert_eq!(flags.dev_db, None);
}

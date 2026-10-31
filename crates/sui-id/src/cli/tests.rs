use super::*;

/// The `SUBCOMMANDS:` block only -- `OPTIONS:` ends it. Scoping to this
/// block, not the whole help text, is half of RFC 107 fix round item 1's
/// fix: a name mentioned in USAGE examples or OPTIONS prose (e.g.
/// `--admin-username` mentions "admin") must not be able to satisfy the
/// binding test on that mention alone.
fn subcommands_block(text: &str) -> &str {
    let start = text.find("SUBCOMMANDS:\n").expect("SUBCOMMANDS header");
    let rest = &text[start..];
    let end = rest
        .find("\nOPTIONS:\n")
        .expect("OPTIONS header ends the block");
    &rest[..end]
}

/// True only when `name` is the entry's own leading token: exactly four
/// spaces of indent, then `name`, then whitespace or end of line. The
/// other half of item 1's fix -- `text.contains(name)` passed for
/// `backup` on `verify-backup`'s own entry, for `restore` on prose
/// mentioning it, and for `admin` on nothing structural at all (luck,
/// since every real `admin <subaction>` entry's own leading token
/// happens to be `admin`). A line with more than four spaces of indent
/// is a wrapped description continuation, not a new entry, and is
/// skipped.
fn has_top_level_entry(block: &str, name: &str) -> bool {
    for line in block.lines() {
        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        if rest.split_whitespace().next() == Some(name) {
            return true;
        }
    }
    false
}

/// True only when `name` is named on its own `admin <name>` entry line
/// (which may wrap its description to the next line, as
/// `issue-recovery-link` and the two newest subactions do) -- not a
/// substring anywhere, which is what let `rotate-metrics-token`'s own
/// previous removal (the review's mutation 3) go undetected by `admin`
/// name checks that only ever used `rotate-metrics-token` itself, never
/// this helper.
fn has_admin_entry(block: &str, name: &str) -> bool {
    for line in block.lines() {
        let Some(rest) = line.strip_prefix("    admin ") else {
            continue;
        };
        if rest.split_whitespace().next() == Some(name) {
            return true;
        }
    }
    false
}

/// RFC 107's binding test, direction 1: every name in each const must
/// have its own entry in the help text's `SUBCOMMANDS:` block -- not
/// merely appear as a substring anywhere in the text, which the
/// original version of this test checked and which three of the five
/// top-level names (`backup`, `restore`, `admin`) satisfied by
/// accident, through unrelated text, even with their own entry deleted
/// (RFC 107 fix round 2026-10-19, item 1).
#[test]
fn cli_bindings_cover_help_and_dispatch() {
    let text = help_text();
    let block = subcommands_block(&text);
    for name in TOP_LEVEL_SUBCOMMANDS {
        assert!(
            has_top_level_entry(block, name),
            "{name} is in TOP_LEVEL_SUBCOMMANDS but has no entry of its own \
             in the SUBCOMMANDS block"
        );
    }
    for name in ADMIN_SUBACTIONS {
        assert!(
            has_admin_entry(block, name),
            "{name} is in ADMIN_SUBACTIONS but has no `admin {name}` entry \
             of its own in the SUBCOMMANDS block"
        );
    }
}

/// RFC 107's binding test, direction 2: every name in `ADMIN_SUBACTIONS`
/// must dispatch to a real arm in `run_admin_subcommand`, not fall
/// through to the `unreachable!()` guard that catches a const entry
/// with no matching arm (the "arm with no entry" case the dispatch
/// names) -- caught here as a panic, which fails the test, since
/// `run_admin_subcommand`'s own string-literal `match` cannot be made
/// to fail to compile against a runtime `&[&str]` without restructuring
/// it into a boxed-future dispatch table, a complexity tax this plain
/// CLI dispatcher does not carry for the five lines a test already
/// costs.
///
/// **Every subaction must fail this call, but not all for the same
/// reason** (RFC 095 M3 stage 6 §3.1, measured rather than assumed).
/// Most are missing a required flag (`--username`, `--max-uses` is
/// optional so `issue-registration-token` fails on nothing it needs here
/// and is really failing on the config load below, same as
/// `legacy-dynamic-candidates`). `legacy-dynamic-candidates` has no
/// required flag at all; it fails because `Config::load("./sui-id.toml")`
/// finds nothing — `sui-id.toml` is gitignored and does not exist in a
/// fresh checkout, including the one this test runs against. Measured
/// directly: the error is `"failed to read config ./sui-id.toml: No such
/// file or directory"`, not a flag complaint. The assertion below only
/// ever checked that the dispatcher recognises the name, not *why* the
/// call failed, so this was always the right assertion — only the
/// doc comment and the `expect_err` caption claimed a narrower reason
/// than the code enforces. A fake required flag is not the fix: that
/// would change the command's interface to suit this test, backwards
/// from how a command should be designed.
#[tokio::test]
async fn every_admin_subaction_dispatches() {
    for name in ADMIN_SUBACTIONS {
        let args: Vec<String> = vec!["sui-id".into(), "admin".into(), (*name).into()];
        let err = run_admin_subcommand(&args).await.expect_err(
            "every admin subaction must fail this no-flags call -- on a missing required \
             flag for most, or (legacy-dynamic-candidates) on the absent ./sui-id.toml, \
             never by succeeding",
        );
        let msg = format!("{err:?}");
        assert!(
            !msg.contains("unknown admin subaction"),
            "{name} is in ADMIN_SUBACTIONS but the dispatcher does not \
             recognise it: {msg}"
        );
    }
}

/// RFC 107 fix round, item 2: closes the direction §2 of the returned
/// package disclosed as uncovered -- an arm added to
/// `run_admin_subcommand`'s own `match` without a matching
/// `ADMIN_SUBACTIONS` entry. The review's own precedent for doing this
/// textually rather than by restructuring the dispatcher is this
/// repository's G13 (`scripts/check-audit-matrix.sh`), which already
/// gates a different list against Rust source literals the same way,
/// in both directions. Textual, like G21's own checker states of
/// itself ("a textual scan, a proxy, not a proof"): reads this file's
/// own source via `include_str!`, isolates `run_admin_subcommand`'s
/// body by the next `pub(crate)` function after it, and asserts every
/// `Some("...")` literal pattern found there is in `ADMIN_SUBACTIONS`.
/// A stray `Some("...")` elsewhere in the body (there is none today)
/// would be a false positive; `every_admin_subaction_dispatches` above
/// is still the direction that matters more in practice, since this one
/// only fires if someone adds a working arm and simply forgets the
/// const line next to it.
#[test]
fn every_dispatched_admin_arm_is_in_the_const() {
    let src = include_str!("../cli.rs");
    let start = src
        .find("pub(crate) async fn run_admin_subcommand")
        .expect("run_admin_subcommand exists in cli.rs");
    let after = &src[start + 1..];
    let end = after
        .find("\npub(crate)")
        .expect("a later pub(crate) item ends this function's body");
    let body = &src[start..start + 1 + end];

    let mut rest = body;
    let mut found = Vec::new();
    while let Some(i) = rest.find("Some(\"") {
        let after_open = &rest[i + "Some(\"".len()..];
        let close = after_open
            .find('"')
            .expect("Some(\"...\" has a closing quote");
        found.push(&after_open[..close]);
        rest = &after_open[close..];
    }

    assert!(
        !found.is_empty(),
        "found no Some(\"...\") arms at all -- the scan's own boundaries are wrong"
    );
    for name in &found {
        assert!(
            ADMIN_SUBACTIONS.contains(name),
            "`Some(\"{name}\")` is a dispatch arm in run_admin_subcommand but \
             {name} is not in ADMIN_SUBACTIONS -- it will not appear in \
             --help or the unknown-subaction message"
        );
    }
    assert_eq!(
        found.len(),
        ADMIN_SUBACTIONS.len(),
        "found {} dispatch arms but ADMIN_SUBACTIONS has {} entries -- \
         counts must match for this to be a real bidirectional check, \
         not merely a one-way scan that happens to find a subset",
        found.len(),
        ADMIN_SUBACTIONS.len()
    );
}

/// Mutation check for the test above: a name genuinely absent from
/// both the const and the dispatcher must still produce the
/// "unknown admin subaction" error, confirming the assertion above
/// is discriminating rather than vacuous.
#[tokio::test]
async fn an_unlisted_admin_subaction_is_rejected() {
    let args: Vec<String> = vec![
        "sui-id".into(),
        "admin".into(),
        "not-a-real-subaction".into(),
    ];
    let err = run_admin_subcommand(&args).await.expect_err("unknown");
    assert!(format!("{err:?}").contains("unknown admin subaction"));
}

/// RFC 095 M3 stage 6 §5, made executable: the empty-case output must
/// state the incompleteness of the evidence, never a bare "there are
/// none" claim an operator could mistake for an all-clear. Tested
/// directly against [`render_legacy_dynamic_candidate_report`]'s return
/// value, not captured stdout, and with no database at all — the report
/// struct is plain data.
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

/// The control for the test above: a genuine candidate's own row renders
/// with the `legacy_unstamped_dynamic_candidate` label RFC 095 names,
/// and the non-empty case still carries its own "found, not existing"
/// qualifier (§5's second sentence, required on every run).
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

/// RFC 094 M2b wave 4 stage 4, C22 — the e2e assertion through the
/// layer that actually runs: there is no HTTP route for this command,
/// and no precedent in this crate for driving `run_admin_*` through a
/// spawned binary (`backup`/`restore`'s own e2e tests call the library
/// functions beneath the CLI directly, for the same reason) — so this
/// calls `run_admin_revoke_registration_token` itself, the real
/// argument-parsing and config/db-opening path, against a real on-disk
/// database and key file, the same construction
/// `tests/e2e/backup.rs::backup_then_restore_preserves_users_and_clients`
/// uses. Asserts the audit record, not only the revoked value.
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

    let args: Vec<String> = vec![
        "sui-id".into(),
        "admin".into(),
        "revoke-registration-token".into(),
        "--id".into(),
        id.to_string(),
        "--config".into(),
        config_path.to_string_lossy().into_owned(),
    ];
    run_admin_revoke_registration_token(&args)
        .await
        .expect("revoke");

    let key2 = sui_id::keyring::resolve(&key_path)
        .expect("resolve key")
        .key;
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
/// the conversion did not get. Added in review, for a reason worth
/// recording: `client_registration_token::create` is deliberately kept
/// for the seventeen fixtures that seed tokens, so the CLI has two
/// functions it could call and only one of them seals the record.
/// Reverting the one line in `run_admin_issue_registration_token` to the
/// kept wrapper left the **entire workspace suite green**. This test is
/// what makes that revert fail. Same construction as C22's test: the
/// real argument-parsing and config/db-opening path against an on-disk
/// database and key file, asserting the audit record and not only the
/// stored row.
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

    let args: Vec<String> = vec![
        "sui-id".into(),
        "admin".into(),
        "issue-registration-token".into(),
        "--config".into(),
        config_path.to_string_lossy().into_owned(),
    ];
    run_admin_issue_registration_token(&args)
        .await
        .expect("issue");

    let key2 = sui_id::keyring::resolve(&key_path)
        .expect("resolve key")
        .key;
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

/// This round's own dispatch, §2a/§3: builds two rows directly (no
/// database needed — [`render_registration_token_list`] is pure data
/// in, text out, same separation as
/// [`render_legacy_dynamic_candidate_report`]), one live and one
/// revoked, and checks every column the dispatch named: the id (the
/// whole point of this round), `max_uses = 0` rendering as `unlimited`
/// rather than the literal `0`, `expires_at = None` rendering as
/// `never`, and the revoked row's own revocation timestamp appearing.
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

/// §2a's explicit requirement: the raw token and its hash must never
/// reach stdout through this command. Checked against both rows' own
/// hash strings, which would otherwise be indistinguishable from a
/// column this command is allowed to print.
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

/// This round's §2b: `issue-registration-token`'s own output must
/// contain the row's id, not only the token, so an operator who
/// records the output can revoke later without the listing command
/// above or the database.
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

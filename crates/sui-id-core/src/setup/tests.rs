use super::*;
use sui_id_store::crypto::MasterKey;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

// ---------- generate_admin_password (RFC 077) ----------

#[test]
fn generated_password_is_24_alphanumeric_chars() {
    let pw = generate_admin_password();
    assert_eq!(pw.len(), 24);
    assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn generated_passwords_differ_across_calls() {
    let a = generate_admin_password();
    let b = generate_admin_password();
    assert_ne!(a.as_str(), b.as_str());
}

#[test]
fn generated_password_satisfies_standard_policy() {
    let pw = generate_admin_password();
    check_password_policy(&pw, SecurityLevel::Standard.password_min_len())
        .expect("generated password must pass Standard policy");
}

// ---------- create_initial_admin_headless (RFC 077) ----------

#[tokio::test]
async fn headless_setup_creates_admin_and_marks_initialized() {
    let db = fresh_db();
    let clock = crate::time::system_clock();

    let created = create_initial_admin_headless(
        &db,
        &clock,
        "first-admin",
        "a-long-enough-password",
        Some("First Admin"),
        Some("admin@example.com"),
    )
    .await
    .expect("headless setup");

    assert_eq!(created.username, "first-admin");
    assert!(state::is_initialized(&db).expect("state read"));

    // The headless path writes an ordinary credential row.
    credentials::get(&db, created.user_id).await.expect("cred");
}

/// Cycle I stage 4, added in review. The six S10 tests all live in
/// `sui-id-store`'s own command runner and call `complete_initial_setup`
/// directly, so none of them can see *this* layer choosing to call it.
/// Replacing that one call with the four separate bare writes it
/// replaced — recording nothing at all — left every S10 test green; only
/// RFC 115 D4's text-scanning credential allowlist noticed, and only
/// because the revert happened to reintroduce `credentials::upsert` into
/// this file. That protection is incidental. This test is not: it pins
/// the record of the most privileged operation this system performs to
/// the path that actually runs it.
#[tokio::test]
async fn headless_setup_records_the_event_through_the_sealed_command() {
    let db = fresh_db();
    let clock = crate::time::system_clock();

    let created = create_initial_admin_headless(
        &db,
        &clock,
        "audited-admin",
        "a-long-enough-password",
        None,
        None,
    )
    .await
    .expect("headless setup");

    let rows = sui_id_store::repos::audit::recent(&db, 50)
        .await
        .expect("audit list");
    let setup_rows: Vec<_> = rows
        .iter()
        .filter(|r| r.action == "admin.setup.completed")
        .collect();
    assert_eq!(
        setup_rows.len(),
        1,
        "exactly one setup event, committed with the writes"
    );
    let row = setup_rows[0];
    assert_eq!(
        row.target.as_deref(),
        Some(created.user_id.to_string().as_str()),
        "the event names the admin it created"
    );
    assert_eq!(
        row.actor, None,
        "first-run setup has no authorized actor (RFC 120 D5)"
    );
    assert!(
        row.note
            .as_deref()
            .unwrap_or("")
            .contains("authorized_by=filesystem"),
        "the headless path is authorized by the filesystem: {:?}",
        row.note
    );
    assert!(
        !rows
            .iter()
            .any(|r| r.action == "setup.create_initial_admin"),
        "the retired event name must not be emitted"
    );
}

/// The other half of the pair. Swapping the two constants the wrappers
/// pass left the whole workspace green except the test above — which
/// catches an inversion but not both wrappers being set to the same
/// value. Two tests, one per wrapper, is what actually pins the choice.
#[tokio::test]
async fn web_wizard_setup_records_that_a_setup_token_authorized_it() {
    let db = fresh_db();
    let clock = crate::time::system_clock();

    let created = create_initial_admin(
        &db,
        &clock,
        "expected-token",
        "expected-token",
        "wizard-admin",
        "a-long-enough-password",
        None,
        None,
    )
    .await
    .expect("web wizard setup");

    let rows = sui_id_store::repos::audit::recent(&db, 50)
        .await
        .expect("audit list");
    let row = rows
        .iter()
        .find(|r| r.action == "admin.setup.completed")
        .expect("a setup event");
    assert_eq!(
        row.target.as_deref(),
        Some(created.user_id.to_string().as_str())
    );
    assert_eq!(
        row.actor, None,
        "no authorized actor at first run (RFC 120 D5)"
    );
    assert!(
        row.note
            .as_deref()
            .unwrap_or("")
            .contains("authorized_by=setup_token"),
        "the wizard path is authorized by the setup token: {:?}",
        row.note
    );
}

#[tokio::test]
async fn headless_setup_fails_when_already_initialized() {
    let db = fresh_db();
    let clock = crate::time::system_clock();

    create_initial_admin_headless(
        &db,
        &clock,
        "first-admin",
        "a-long-enough-password",
        None,
        None,
    )
    .await
    .expect("first setup");

    let second = create_initial_admin_headless(
        &db,
        &clock,
        "second-admin",
        "another-long-password",
        None,
        None,
    )
    .await;
    assert!(matches!(second, Err(CoreError::AlreadyInitialized)));
}

#[tokio::test]
async fn headless_setup_enforces_standard_password_policy() {
    let db = fresh_db();
    let clock = crate::time::system_clock();

    // 8 chars passes Development but must fail here: setup is always Standard.
    let r = create_initial_admin_headless(&db, &clock, "first-admin", "changeme", None, None).await;
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
    assert!(!state::is_initialized(&db).expect("state read"));
}

#[tokio::test]
async fn web_wizard_path_still_requires_matching_token() {
    let db = fresh_db();
    let clock = crate::time::system_clock();

    let r = create_initial_admin(
        &db,
        &clock,
        "expected-token",
        "wrong-token",
        "first-admin",
        "a-long-enough-password",
        None,
        None,
    )
    .await;
    assert!(matches!(r, Err(CoreError::Forbidden)));

    let ok = create_initial_admin(
        &db,
        &clock,
        "expected-token",
        "expected-token",
        "first-admin",
        "a-long-enough-password",
        None,
        None,
    )
    .await;
    assert!(ok.is_ok());
    credentials::get(&db, ok.unwrap().user_id)
        .await
        .expect("cred");
}

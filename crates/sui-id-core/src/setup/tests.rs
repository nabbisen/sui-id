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

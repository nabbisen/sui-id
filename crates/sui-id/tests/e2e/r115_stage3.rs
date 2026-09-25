//! RFC 115 stage 3 — the provisioning throttle (D11), secrets that cannot be
//! printed (D5), and `must_change` gone (D12).
//!
//! The store owns the exemption predicate (`commands/tests/runner/
//! provisioning.rs`: a live account can never qualify). This file drives the
//! same behaviour through the web, and pins the two rules that are about the
//! *source*: no form field holds a secret in a plain `String`, and nothing in
//! production code mentions `must_change`.

use super::common::*;
use super::r102_stage1::sign_in;
use super::r103_stage1::NEW_PASSWORD;
use super::r103_stage3::admin;
use super::r115_stage2::{
    complete_with, created_id, get_as, issue_from_confirm, post_issue, production_sources, scalar,
    web_create,
};
use axum::http::StatusCode;
use sui_id_i18n::Locale;

const LOCALES: [Locale; 3] = [Locale::Ja, Locale::En, Locale::ZhHans];

async fn notes(state: &sui_id::AppState) -> Vec<String> {
    state
        .db
        .with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT note FROM audit_log WHERE action = 'user.recovery_link.issued' \
                 ORDER BY seq",
            )?;
            let rows = stmt
                .query_map([], |r| r.get::<_, Option<String>>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows.into_iter().flatten().collect())
        })
        .await
        .expect("notes")
}

// ── D11 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn r115_s3_bulk_creation_is_not_capped_at_five_an_hour() {
    // Before D11, the sixth user created in an hour could not be given a link.
    let a = admin().await;
    for i in 0..8 {
        let id = created_id(
            &web_create(
                &a,
                &format!("user{i}"),
                &format!("u{i}@test.invalid"),
                false,
                "",
            )
            .await,
        );
        let token = issue_from_confirm(&a, id).await;
        assert!(!token.is_empty(), "user {i} got a link");
    }
    let notes = notes(&a.state).await;
    assert_eq!(notes.len(), 8);
    assert!(
        notes.iter().all(|n| n.contains(" provisioning=1 ")),
        "every one is a provisioning link and says so: {notes:?}"
    );
}

#[tokio::test]
async fn r115_s3_provisioning_cannot_be_used_to_reach_an_existing_account_unthrottled() {
    let a = admin().await;
    // Activate one user so it is a live account.
    let dave = created_id(&web_create(&a, "dave", "dave@test.invalid", false, "").await);
    let token = issue_from_confirm(&a, dave).await;
    assert!(
        complete_with(&a.state, &token, NEW_PASSWORD)
            .await
            .completed()
    );
    // Five ordinary links for the live account are allowed, the sixth is not,
    // however many provisioning links were issued before them.
    for i in 0..5 {
        let r = post_issue(&a, dave).await;
        assert_eq!(r.status, StatusCode::OK, "ordinary link {i}: {}", r.body);
    }
    let sixth = post_issue(&a, dave).await;
    assert_eq!(
        sixth.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        sixth.body
    );
    // Creating another user still works: that is a different, provisioning count.
    let erin = created_id(&web_create(&a, "erin", "erin@test.invalid", false, "").await);
    assert!(!issue_from_confirm(&a, erin).await.is_empty());
}

#[tokio::test]
async fn r115_s3_an_administrator_created_on_the_web_is_not_exempt() {
    let a = admin().await;
    let bea = created_id(&web_create(&a, "bea", "bea@test.invalid", true, "").await);
    issue_from_confirm(&a, bea).await;
    let notes = notes(&a.state).await;
    assert_eq!(notes.len(), 1);
    assert!(
        !notes[0].contains("provisioning"),
        "an administrator target is an ordinary link: {}",
        notes[0]
    );
}

// ── the new-user form warns an administrator with no second factor ────

#[tokio::test]
async fn r115_s3_the_new_user_form_warns_an_administrator_who_cannot_issue_the_link() {
    let shows = |body: &str| {
        LOCALES
            .iter()
            .any(|l| body.contains(l.strings().users_create_needs_second_factor))
    };
    // The setup administrator has no second factor: the warning is on the form,
    // before the account exists.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let bare = super::r103_stage3::Admin {
        state,
        id: sui_id_shared::ids::UserId::new(),
        session,
    };
    let r = get_as(&bare, "/admin/users/new").await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(shows(&r.body), "the warning is shown");

    // An administrator with a second factor is not warned.
    let a = admin().await;
    let r = get_as(&a, "/admin/users/new").await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(
        !shows(&r.body),
        "no warning for an administrator who can issue"
    );
}

// ── D12 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn r115_s3_the_credentials_table_has_no_must_change_column() {
    let state = test_app();
    let columns: i64 = scalar(
        &state,
        "SELECT COUNT(*) FROM pragma_table_info('credentials') WHERE name = 'must_change'".into(),
    )
    .await;
    assert_eq!(columns, 0);
    // And a user activated the ordinary way still signs in.
    let a = admin().await;
    let id = created_id(&web_create(&a, "dave", "dave@test.invalid", false, "").await);
    let token = issue_from_confirm(&a, id).await;
    assert!(
        complete_with(&a.state, &token, NEW_PASSWORD)
            .await
            .completed()
    );
    assert!(!sign_in(&a.state, "dave", NEW_PASSWORD).await.is_empty());
}

#[test]
fn r115_s3_must_change_is_gone_from_production_code() {
    // The grep-proof: the column, `CredentialRow.must_change`, the three
    // function parameters and the claim in `cli.rs`'s doc comment. Migrations
    // are history and are not production code; they are not scanned here (and
    // the two that mention it are 0022, which added the CHECK, and 0043, which
    // drops the column).
    for (rel, text) in production_sources() {
        // The migration list names `0043_drop_credentials_must_change.sql`: a
        // file name, not a use of the column.
        if rel == "sui-id-store/src/migrations.rs" {
            continue;
        }
        assert!(
            !text.contains("must_change"),
            "{rel} still mentions must_change"
        );
    }
}

// ── D5: no form holds a secret in a plain String ──────────────────────

/// Fields that look like secrets by name but are not, or that are the named
/// D6 exception. Everything else must be a `SecretString`.
const NOT_A_SECRET: &[(&str, &str, &str)] = &[
    (
        "sui-id/src/http/handlers/dynamic_register.rs",
        "RegistrationRequest",
        "token_endpoint_auth_method",
    ),
    (
        "sui-id/src/http/handlers/oauth_token.rs",
        "IntrospectForm",
        "token_type_hint",
    ),
    (
        "sui-id/src/http/handlers/oauth_token.rs",
        "RevokeForm",
        "token_type_hint",
    ),
    (
        "sui-id/src/http/handlers/oidc.rs",
        "AuthorizeQuery",
        "code_challenge",
    ),
    (
        "sui-id/src/http/handlers/oidc.rs",
        "AuthorizeQuery",
        "code_challenge_method",
    ),
    (
        "sui-id/src/http/handlers/oidc.rs",
        "LogoutQuery",
        "id_token_hint",
    ),
    // D6: `--dev` seeds fixture users whose passwords it prints by design.
    ("sui-id/src/runtime/dev_mode.rs", "DevAdminToml", "password"),
    ("sui-id/src/runtime/dev_mode.rs", "DevUserToml", "password"),
    (
        "sui-id/src/runtime/dev_mode.rs",
        "DevClientToml",
        "client_secret",
    ),
    // Names of environment variables, not the secrets.
    (
        "sui-id/src/runtime/config.rs",
        "UserSourceConfig",
        "bind_password_env",
    ),
    (
        "sui-id/src/runtime/config.rs",
        "FederationProviderConfig",
        "client_secret_env",
    ),
];

fn is_secret_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "password",
        "secret",
        "token",
        "code",
        "verifier",
        "credential",
        "passphrase",
    ]
    .iter()
    .any(|k| n.contains(k))
}

/// `(struct name, field name)` for every `Debug + Deserialize` struct in
/// `text` whose field is a secret-looking name held in a plain string.
fn plain_string_secrets(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut derives = String::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("#[derive(") {
            derives = t.to_owned();
            continue;
        }
        if let Some(rest) = t
            .strip_prefix("pub struct ")
            .or_else(|| t.strip_prefix("struct "))
        {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            current =
                (t.ends_with('{') && derives.contains("Debug") && derives.contains("Deserialize"))
                    .then_some(name);
            derives.clear();
            continue;
        }
        if t == "}" {
            current = None;
            continue;
        }
        if let (Some(name), Some((field, ty))) =
            (&current, t.trim_start_matches("pub ").split_once(':'))
        {
            let ty = ty.trim().trim_end_matches(',');
            if (ty == "String" || ty == "Option<String>") && is_secret_name(field.trim()) {
                out.push((name.clone(), field.trim().to_owned()));
            }
        }
        if !t.starts_with("#[") && !t.is_empty() && !t.contains(':') && current.is_none() {
            derives.clear();
        }
    }
    out
}

#[test]
fn r115_s3_no_form_field_holds_a_secret_in_a_plain_string() {
    let mut offenders = Vec::new();
    for (rel, text) in production_sources() {
        if !rel.starts_with("sui-id/src/") {
            continue;
        }
        for (st, field) in plain_string_secrets(&text) {
            if !NOT_A_SECRET.contains(&(rel.as_str(), st.as_str(), field.as_str())) {
                offenders.push(format!("{rel}: {st}.{field}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a Debug + Deserialize struct holds a secret in a plain String; use \
         secrecy::SecretString (RFC 115 D5), or add it to NOT_A_SECRET with a reason: {offenders:#?}"
    );
}

#[test]
fn r115_s3_the_dead_dto_secrets_are_deleted() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sui-id-shared/src/api.rs"),
    )
    .expect("api.rs");
    for name in [
        "CreateInitialAdminRequest",
        "LoginRequest",
        "CreateUserRequest",
        "ResetPasswordRequest",
        "CreateClientResponse",
    ] {
        assert!(
            !text.contains(name),
            "{name} is dead and derived Debug over a secret"
        );
    }
}

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! RFC 095 M3 stage 4a — persist the three fields.
//!
//! Two layers, deliberately: migration 0049's own effect on a database
//! that predates it (raw SQL, no repo functions — the schema at that
//! point has none of today's `ClientRow` fields to call them with), and
//! the storage round-trip through the real repo functions on a database
//! that already has the column (`Database::open_in_memory`, which always
//! migrates to head).

use crate::Database;
use crate::crypto::MasterKey;
use crate::migrations;
use crate::models::{ClientRow, ConsentPolicy, RegistrationSource, TokenEndpointAuthMethod};
use crate::repos::clients;
use rusqlite::Connection;
use sui_id_shared::ids::ClientId;

fn conn_at(version: i32) -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run migrations");
    conn
}

/// Minimal `INSERT` matching the `clients` table's shape as of schema
/// version 48 — before migration 0049 added the three columns under test.
/// Only the columns this table has no default for are given explicit
/// values; the row depends on 0002's and 0025's defaults for the rest,
/// the same as a real pre-0049 row would.
fn insert_legacy_client(conn: &Connection, id: &str) {
    // confidential = 0: a schema CHECK (added after v48, carried forward)
    // requires a confidential row to have a `secret_hash`, which this
    // migration's own behaviour has nothing to do with -- kept out of the
    // way rather than given one it does not need.
    conn.execute(
        "INSERT INTO clients(id, name, confidential, redirect_uris, created_at, updated_at) \
         VALUES(?1, ?2, 0, '[]', datetime('now'), datetime('now'))",
        rusqlite::params![id, format!("legacy-{id}")],
    )
    .expect("seed legacy client row");
}

fn auth_method_columns(
    conn: &Connection,
    id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    conn.query_row(
        "SELECT token_endpoint_auth_method, grant_types, response_types FROM clients WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .expect("row exists")
}

/// RFC 095 M3 stage 4a §2/§6: the migration's own effect on data that
/// predates it. Two pre-existing rows, not one — "on every pre-existing
/// row" is the dispatch's own wording, and a bug that only showed up past
/// the first row would survive a single-row test.
#[test]
fn migration_0049_leaves_every_pre_existing_row_with_all_three_columns_null() {
    let mut conn = conn_at(48);
    insert_legacy_client(&conn, "c1");
    insert_legacy_client(&conn, "c2");

    migrations::run_up_to(&mut conn, 49).expect("apply 0049");

    for id in ["c1", "c2"] {
        let (method, grants, responses) = auth_method_columns(&conn, id);
        assert_eq!(
            method, None,
            "{id}: token_endpoint_auth_method must be NULL, not a default"
        );
        assert_eq!(grants, None, "{id}: grant_types must be NULL");
        assert_eq!(responses, None, "{id}: response_types must be NULL");
    }
}

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

fn sample_client(
    id: ClientId,
    token_endpoint_auth_method: Option<TokenEndpointAuthMethod>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
) -> ClientRow {
    let now = chrono::Utc::now();
    ClientRow {
        id,
        name: "test-client".into(),
        confidential: false,
        secret_hash: None,
        redirect_uris: vec!["https://example.com/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: ConsentPolicy::default(),
        registered_via: RegistrationSource::default(),
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        token_endpoint_auth_method,
        grant_types,
        response_types,
        created_at: now,
        updated_at: now,
    }
}

/// Half of the C1 assertion, made executable: writing `None` must read
/// back `None`, not a value the write path substituted.
///
/// **This is the *code-side* guard, and it is not the one a column
/// `DEFAULT` trips.** Measured 2026-10-22: adding
/// `DEFAULT 'client_secret_basic'` to migration `0049` leaves this test
/// green, because `create_within_tx` names all three columns in its
/// `INSERT` and SQLite applies a column default only to a column the
/// `INSERT` omits. The boundary C1 depends on is *"did the `INSERT` name
/// the column"*, not *"is the stored value NULL"*.
///
/// So the two halves are checked by two different tests, and both are
/// needed: a schema default is caught by
/// `migration_0049_leaves_every_pre_existing_row_with_all_three_columns_null`
/// above, and a code-side default — `….unwrap_or("client_secret_basic")`
/// in the write path — is caught here. Each was confirmed by mutation.
#[tokio::test]
async fn writing_none_for_all_three_reads_back_none() {
    let db = fresh_db();
    let id = ClientId::new();
    clients::create(&db, &sample_client(id, None, None, None))
        .await
        .expect("create");

    let row = clients::get(&db, id).await.expect("get");
    assert_eq!(row.token_endpoint_auth_method, None);
    assert_eq!(row.grant_types, None);
    assert_eq!(row.response_types, None);
}

#[tokio::test]
async fn token_endpoint_auth_method_round_trips() {
    let db = fresh_db();
    let id = ClientId::new();
    clients::create(
        &db,
        &sample_client(
            id,
            Some(TokenEndpointAuthMethod::ClientSecretPost),
            None,
            None,
        ),
    )
    .await
    .expect("create");

    let row = clients::get(&db, id).await.expect("get");
    assert_eq!(
        row.token_endpoint_auth_method,
        Some(TokenEndpointAuthMethod::ClientSecretPost)
    );
}

#[tokio::test]
async fn grant_types_round_trips() {
    let db = fresh_db();
    let id = ClientId::new();
    let grants = vec![
        "authorization_code".to_string(),
        "refresh_token".to_string(),
    ];
    clients::create(&db, &sample_client(id, None, Some(grants.clone()), None))
        .await
        .expect("create");

    let row = clients::get(&db, id).await.expect("get");
    assert_eq!(row.grant_types, Some(grants));
}

#[tokio::test]
async fn response_types_round_trips() {
    let db = fresh_db();
    let id = ClientId::new();
    let responses = vec!["code".to_string()];
    clients::create(&db, &sample_client(id, None, None, Some(responses.clone())))
        .await
        .expect("create");

    let row = clients::get(&db, id).await.expect("get");
    assert_eq!(row.response_types, Some(responses));
}

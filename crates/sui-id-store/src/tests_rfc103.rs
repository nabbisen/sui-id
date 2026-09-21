#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::panic
)]
//! Tests for RFC 103 stage 3 — migration 0041, the recovery-link columns on
//! `password_reset_tokens`.
//!
//! The command-level behaviour (U37, the D3 invalidations, `origin`) is
//! tested in `commands/tests/runner/recovery.rs`; this file is the schema
//! itself: what the migration does to existing rows and what the CHECKs and
//! the foreign key refuse.

#[cfg(test)]
mod recovery_link_schema_tests {
    use crate::migrations;
    use rusqlite::Connection;

    /// `id`, `issued_via`, `issued_by`, `revoked_at`, `consumed_at`.
    type TokenColumns = (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    fn conn_at(version: i32) -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable FK");
        migrations::run_up_to(&mut conn, version).expect("run migrations");
        conn
    }

    fn add_user(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, \
                               created_at, updated_at, user_uuid, failed_login_count) \
             VALUES(?1, ?1, 0, 0, 0, datetime('now'), datetime('now'), ?2, 0)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert user");
    }

    fn insert_token(
        conn: &Connection,
        id: &str,
        user: &str,
        via: &str,
        by: Option<&str>,
    ) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO password_reset_tokens(id, user_id, token_hash, issued_at, expires_at, \
                                               issued_via, issued_by) \
             VALUES(?1, ?2, CAST(?1 AS BLOB), datetime('now'), datetime('now', '+30 minutes'), ?3, ?4)",
            rusqlite::params![id, user, via, by],
        )
    }

    #[test]
    fn migration_0041_keeps_existing_tokens_as_email_origin_and_unrevoked() {
        let conn = conn_at(40);
        add_user(&conn, "u1");
        conn.execute_batch(
            "INSERT INTO password_reset_tokens(id, user_id, token_hash, issued_at, expires_at) \
             VALUES('t-live', 'u1', X'01', datetime('now'), datetime('now', '+30 minutes')); \
             INSERT INTO password_reset_tokens(id, user_id, token_hash, issued_at, expires_at, \
                                               consumed_at) \
             VALUES('t-used', 'u1', X'02', datetime('now'), datetime('now', '+30 minutes'), \
                    datetime('now'));",
        )
        .expect("seed pre-migration tokens");

        let tx_sql = migrations::sql_for_version(41);
        conn.execute_batch(tx_sql).expect("apply 0041");

        let rows: Vec<TokenColumns> = conn
            .prepare(
                "SELECT id, issued_via, issued_by, revoked_at, consumed_at \
                 FROM password_reset_tokens ORDER BY id",
            )
            .expect("prepare")
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        assert_eq!(rows.len(), 2, "no row lost");
        for (id, via, by, revoked, _) in &rows {
            assert_eq!(via, "email", "{id}: pre-existing tokens are email-origin");
            assert!(by.is_none(), "{id}: no issuing administrator");
            assert!(revoked.is_none(), "{id}: not revoked");
        }
        assert!(rows[0].4.is_none(), "t-live is still unconsumed");
        assert!(rows[1].4.is_some(), "t-used is still consumed");
    }

    #[test]
    fn issued_by_is_present_exactly_when_the_origin_is_web() {
        let conn = conn_at(41);
        add_user(&conn, "admin");
        add_user(&conn, "user");

        insert_token(&conn, "ok-email", "user", "email", None).expect("email, no issuer");
        insert_token(&conn, "ok-cli", "user", "cli", None).expect("cli, no issuer");
        insert_token(&conn, "ok-web", "user", "web", Some("admin")).expect("web with issuer");

        for (id, via, by) in [
            ("bad-web-anonymous", "web", None),
            ("bad-email-issuer", "email", Some("admin")),
            ("bad-cli-issuer", "cli", Some("admin")),
            ("bad-origin", "carrier-pigeon", None),
        ] {
            let why = insert_token(&conn, id, "user", via, by)
                .expect_err(&format!("{id}: must be refused"))
                .to_string();
            assert!(
                why.contains("CHECK constraint failed"),
                "{id}: refused by a CHECK, not for some other reason: {why}"
            );
        }
    }

    #[test]
    fn a_web_token_needs_an_existing_issuing_administrator() {
        let conn = conn_at(41);
        add_user(&conn, "user");
        let why = insert_token(&conn, "t", "user", "web", Some("nobody"))
            .expect_err("an issuer that does not exist")
            .to_string();
        assert!(why.contains("FOREIGN KEY constraint failed"), "{why}");
    }

    #[test]
    fn deleting_the_issuing_administrator_takes_their_web_tokens_not_others() {
        let conn = conn_at(41);
        add_user(&conn, "admin");
        add_user(&conn, "user");
        insert_token(&conn, "by-admin", "user", "web", Some("admin")).expect("web");
        insert_token(&conn, "by-mail", "user", "email", None).expect("email");
        insert_token(&conn, "by-cli", "user", "cli", None).expect("cli");

        conn.execute("DELETE FROM users WHERE id = 'admin'", [])
            .expect("delete the issuing admin");

        let left: Vec<String> = conn
            .prepare("SELECT id FROM password_reset_tokens ORDER BY id")
            .expect("prepare")
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        assert_eq!(
            left,
            ["by-cli", "by-mail"],
            "cascade leaves no CHECK-violating row"
        );
    }

    #[test]
    fn the_throttle_indexes_exist() {
        let conn = conn_at(41);
        for name in [
            "idx_password_reset_tokens_issued_by",
            "idx_password_reset_tokens_cli",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [name],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(n, 1, "{name}");
        }
    }
}

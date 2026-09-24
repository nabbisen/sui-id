#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::panic
)]
//! Tests for RFC 115 stage 3 — migrations 0042 (`password_reset_tokens.
//! provisioning`, D11) and 0043 (`credentials.must_change` dropped, D12).

#[cfg(test)]
mod schema_tests {
    use crate::migrations;
    use rusqlite::Connection;

    fn conn_at(version: i32) -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable FK");
        migrations::run_up_to(&mut conn, version).expect("run migrations");
        conn
    }

    #[test]
    fn drop_column_works_on_the_pinned_sqlite_against_the_real_schema() {
        // Verified BEFORE the migration was written (RFC 115 handoff, stage 3):
        // `credentials.must_change` carries a column CHECK from migration
        // 0022, and SQLite refuses to drop a column that another part of the
        // schema references. A CHECK attached to the column itself is fine.
        let conn = conn_at(41);
        let version: String = conn
            .query_row("SELECT sqlite_version()", [], |r| r.get(0))
            .expect("version");
        eprintln!("bundled SQLite {version}");
        conn.execute(
            "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, created_at, \
                               updated_at, user_uuid, failed_login_count) \
             VALUES('u1','u1',0,0,0,datetime('now'),datetime('now'),?1,0)",
            [uuid::Uuid::new_v4().to_string()],
        )
        .expect("user");
        conn.execute(
            "INSERT INTO credentials(user_id, password_hash, must_change, updated_at) \
             VALUES('u1','hash',1,datetime('now'))",
            [],
        )
        .expect("credential");
        conn.execute("ALTER TABLE credentials DROP COLUMN must_change", [])
            .expect("DROP COLUMN must_change");
        let hash: String = conn
            .query_row("SELECT password_hash FROM credentials", [], |r| r.get(0))
            .expect("row survives");
        assert_eq!(hash, "hash");
    }

    fn add_user(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, created_at, \
                               updated_at, user_uuid, failed_login_count) \
             VALUES(?1,?1,0,0,0,datetime('now'),datetime('now'),?2,0)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .expect("user");
    }

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        conn.prepare(&format!("PRAGMA table_info({table})"))
            .expect("prepare")
            .query_map([], |r| r.get::<_, String>(1))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows")
    }

    #[test]
    fn migration_0042_keeps_every_existing_token_ordinary_and_refuses_other_values() {
        let conn = conn_at(41);
        add_user(&conn, "u1");
        conn.execute(
            "INSERT INTO password_reset_tokens(id, user_id, token_hash, issued_at, expires_at) \
             VALUES('t1','u1',X'01',datetime('now'),datetime('now','+30 minutes'))",
            [],
        )
        .expect("seed pre-migration token");

        conn.execute_batch(migrations::sql_for_version(42))
            .expect("apply 0042");

        let flag: i64 = conn
            .query_row(
                "SELECT provisioning FROM password_reset_tokens WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .expect("read");
        assert_eq!(flag, 0, "an existing link is an ordinary link");
        let bad = conn.execute(
            "INSERT INTO password_reset_tokens(id, user_id, token_hash, issued_at, expires_at, \
                                               provisioning) \
             VALUES('t2','u1',X'02',datetime('now'),datetime('now','+30 minutes'),2)",
            [],
        );
        assert!(bad.is_err(), "the CHECK refuses a value other than 0 and 1");
    }

    #[test]
    fn migration_0043_drops_the_column_and_keeps_the_credential() {
        let conn = conn_at(42);
        add_user(&conn, "u1");
        conn.execute(
            "INSERT INTO credentials(user_id, password_hash, must_change, updated_at) \
             VALUES('u1','the-hash',1,'2026-01-01T00:00:00Z')",
            [],
        )
        .expect("seed a flagged credential");
        assert!(columns(&conn, "credentials").contains(&"must_change".to_owned()));

        conn.execute_batch(migrations::sql_for_version(43))
            .expect("apply 0043");

        assert_eq!(
            columns(&conn, "credentials"),
            vec!["user_id", "password_hash", "updated_at"],
            "exactly the columns that are read"
        );
        let (hash, updated): (String, String) = conn
            .query_row(
                "SELECT password_hash, updated_at FROM credentials WHERE user_id='u1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("row survives");
        assert_eq!(
            (hash.as_str(), updated.as_str()),
            ("the-hash", "2026-01-01T00:00:00Z")
        );
    }

    #[test]
    fn a_fresh_database_has_no_must_change_column() {
        let conn = conn_at(migrations::MAX_SCHEMA_VERSION);
        assert!(!columns(&conn, "credentials").contains(&"must_change".to_owned()));
        assert!(columns(&conn, "password_reset_tokens").contains(&"provisioning".to_owned()));
    }
}

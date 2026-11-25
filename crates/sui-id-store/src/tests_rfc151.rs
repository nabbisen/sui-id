#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Tests for RFC 151 — migration 0055, `server_settings.password_min_len`.
//!
//! The command-level behaviour (S11) is tested in
//! `commands/tests/runner/server_settings.rs`; this file is the schema
//! itself: what the migration leaves an upgrade with, and what the
//! `CHECK` refuses, pinned against the named floor/ceiling constants
//! the command and HTTP layers use (a migration's own literals cannot
//! themselves derive from a Rust constant).

use crate::migrations;
use rusqlite::Connection;

fn conn_at_55() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_up_to(&mut conn, 55).expect("run migrations");
    conn
}

#[test]
fn an_upgrade_defaults_to_12_so_nothing_changes() {
    let conn = conn_at_55();
    let v: i64 = conn
        .query_row(
            "SELECT password_min_len FROM server_settings WHERE id = 'singleton'",
            [],
            |r| r.get(0),
        )
        .expect("read");
    assert_eq!(v, crate::commands::PASSWORD_MIN_LEN_FLOOR);
}

#[test]
fn the_check_refuses_one_below_the_named_floor_and_accepts_the_floor_itself() {
    let conn = conn_at_55();
    let floor = crate::commands::PASSWORD_MIN_LEN_FLOOR;
    let err = conn
        .execute(
            "UPDATE server_settings SET password_min_len = ?1 WHERE id = 'singleton'",
            rusqlite::params![floor - 1],
        )
        .expect_err("below the floor must be refused");
    assert!(err.to_string().contains("CHECK constraint failed"), "{err}");
    conn.execute(
        "UPDATE server_settings SET password_min_len = ?1 WHERE id = 'singleton'",
        rusqlite::params![floor],
    )
    .expect("exactly the floor is accepted");
}

#[test]
fn the_check_refuses_one_above_the_named_ceiling_and_accepts_the_ceiling_itself() {
    let conn = conn_at_55();
    let ceiling = crate::commands::PASSWORD_MIN_LEN_CEILING;
    let err = conn
        .execute(
            "UPDATE server_settings SET password_min_len = ?1 WHERE id = 'singleton'",
            rusqlite::params![ceiling + 1],
        )
        .expect_err("above the ceiling must be refused");
    assert!(err.to_string().contains("CHECK constraint failed"), "{err}");
    conn.execute(
        "UPDATE server_settings SET password_min_len = ?1 WHERE id = 'singleton'",
        rusqlite::params![ceiling],
    )
    .expect("exactly the ceiling is accepted");
}

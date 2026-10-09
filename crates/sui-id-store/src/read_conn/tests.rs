use crate::errors::StoreError;
use crate::{Database, crypto::MasterKey};

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

#[tokio::test]
async fn a_select_is_prepared_and_read() {
    let db = open_test_db();
    let n: i64 = db
        .with_read(|read| {
            let mut stmt = read.prepare("SELECT 1 + 1")?;
            Ok(stmt.query_row([], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn query_map_returns_every_row() {
    let db = open_test_db();
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TABLE _t (x INTEGER); \
             INSERT INTO _t VALUES (1); \
             INSERT INTO _t VALUES (2); \
             INSERT INTO _t VALUES (3);",
        )?;
        Ok(())
    })
    .await
    .unwrap();

    let rows: Vec<i64> = db
        .with_read(|read| {
            let mut stmt = read.prepare("SELECT x FROM _t ORDER BY x ASC")?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    assert_eq!(rows, vec![1, 2, 3]);
}

/// RFC 094's 2026-08-12 amendment, item 2 of the stage 0 dispatch: a
/// statement that passes Rust's own type check (`prepare` takes any `&str`)
/// but is not read-only must be refused by the interrogation, not by the
/// type. `DELETE` is the plainest write SQLite has; `sqlite3_stmt_readonly`
/// is specified to return false for it, so this is the running check firing
/// on a real statement, not a contrived one.
#[tokio::test]
async fn the_interrogation_refuses_a_non_read_only_statement() {
    let db = open_test_db();
    db.with_conn(|conn| {
        conn.execute_batch("CREATE TABLE _t (x INTEGER);")?;
        Ok(())
    })
    .await
    .unwrap();

    let result = db
        .with_read(|read| {
            let _ = read.prepare("DELETE FROM _t")?;
            Ok(())
        })
        .await;
    assert!(matches!(result, Err(StoreError::NotReadOnly { .. })));
}

/// RFC 094's 2026-08-26 amendment is a feature assertion, not a runtime
/// check this module can perform (see the module doc) — but `ReadConn`'s
/// own `PRAGMA` refusal is a real, running check this module does own, and
/// it fires unconditionally, before `sqlite3_stmt_readonly` is ever asked.
#[tokio::test]
async fn pragma_is_refused_unconditionally() {
    let db = open_test_db();
    let result = db
        .with_read(|read| {
            let _ = read.prepare("PRAGMA table_info(sui_meta)")?;
            Ok(())
        })
        .await;
    assert!(matches!(result, Err(StoreError::PragmaRefused { .. })));
}

#[tokio::test]
async fn pragma_refusal_is_case_and_whitespace_insensitive() {
    let db = open_test_db();
    for sql in [
        "pragma foreign_keys",
        "  PrAgMa foreign_keys",
        "\tPragma  foreign_keys=ON",
    ] {
        let result = db
            .with_read(move |read| {
                let _ = read.prepare(sql)?;
                Ok(())
            })
            .await;
        assert!(
            matches!(result, Err(StoreError::PragmaRefused { .. })),
            "sql: {sql:?}"
        );
    }
}

/// `pragma_table_info('x')` invoked as a table-valued function inside an
/// ordinary `SELECT` is not a `PRAGMA` statement to SQLite's own parser, and
/// must not be refused as one — the refusal is keyed on the statement's own
/// first token, not on the substring "pragma" appearing anywhere in it.
#[tokio::test]
async fn a_select_naming_a_pragma_function_is_not_refused() {
    let db = open_test_db();
    // `sui_meta` already exists via migrations run by `open_in_memory`;
    // no setup needed beyond that.
    let n: i64 = db
        .with_read(|read| {
            let mut stmt = read.prepare("SELECT COUNT(*) FROM pragma_table_info('sui_meta')")?;
            Ok(stmt.query_row([], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert!(n > 0);
}

/// A string too short to contain any refused keyword at all must not panic
/// the length check — `leading_keyword`'s extraction uses `str::find` over
/// the whole (possibly empty) trimmed string, not fixed-length indexing,
/// specifically for this.
#[tokio::test]
async fn a_very_short_statement_does_not_panic_the_pragma_check() {
    let db = open_test_db();
    let err = db
        .with_read(|read| Ok(read.prepare("x").is_err()))
        .await
        .unwrap();
    assert!(
        err,
        "\"x\" is not valid SQL and must fail to prepare, not panic"
    );
}

#[tokio::test]
async fn with_read_sync_matches_with_read() {
    let db = open_test_db();
    let n: i64 = db
        .with_read_sync(|read| {
            let mut stmt = read.prepare("SELECT 41 + 1")?;
            Ok(stmt.query_row([], |r| r.get(0))?)
        })
        .unwrap();
    assert_eq!(n, 42);
}

// ── Stage 0 fix: the side-effecting "read-only" statement class ───────────

/// RFC 096-B1 stage 0 fix, item 1-2: every member of the derived membership
/// list (SQLite's own documentation for `sqlite3_stmt_readonly`, not just the
/// dispatch's seven-item probe — `END` is `COMMIT`'s synonym and is in this
/// crate's own list even though the dispatch's probe didn't name it) is
/// refused at `prepare`, naming the matched keyword in the error.
#[tokio::test]
async fn each_non_content_effect_keyword_is_refused_by_keyword() {
    let db = open_test_db();
    for (sql, expected_keyword) in [
        ("BEGIN", "BEGIN"),
        ("BEGIN DEFERRED TRANSACTION", "BEGIN"),
        ("BEGIN IMMEDIATE", "BEGIN"),
        ("COMMIT", "COMMIT"),
        ("END", "END"),
        ("END TRANSACTION", "END"),
        ("ROLLBACK", "ROLLBACK"),
        ("SAVEPOINT sp1", "SAVEPOINT"),
        ("RELEASE sp1", "RELEASE"),
        (
            "ATTACH DATABASE '/tmp/does-not-matter.db' AS evil",
            "ATTACH",
        ),
        ("DETACH evil", "DETACH"),
    ] {
        let result = db
            .with_read(move |read| {
                let _ = read.prepare(sql)?;
                Ok(())
            })
            .await;
        let keyword = match result {
            Err(StoreError::NonContentEffectRefused { keyword, .. }) => Some(keyword),
            _ => None,
        };
        assert_eq!(keyword.as_deref(), Some(expected_keyword), "sql: {sql:?}");
    }
}

/// RFC 096-B1 stage 0 fix, item 1: `BEGIN IMMEDIATE`/`BEGIN EXCLUSIVE` are
/// refused the same way plain `BEGIN` is, because the keyword match compares
/// only the leading word (`BEGIN`), not the full phrase — confirmed they are
/// also independently refused by `sqlite3_stmt_readonly` (`NotReadOnly`)
/// below via `begin_immediate_and_exclusive_are_also_rejected_by_readonly`,
/// so this is belt-and-suspenders, not the only thing stopping them.
#[tokio::test]
async fn begin_immediate_and_exclusive_are_refused_by_the_keyword_list() {
    let db = open_test_db();
    for sql in ["BEGIN IMMEDIATE", "BEGIN EXCLUSIVE"] {
        let result = db
            .with_read(move |read| {
                let _ = read.prepare(sql)?;
                Ok(())
            })
            .await;
        let keyword = match result {
            Err(StoreError::NonContentEffectRefused { keyword, .. }) => Some(keyword),
            _ => None,
        };
        assert_eq!(keyword.as_deref(), Some("BEGIN"), "sql: {sql:?}");
    }
}

/// Independent of the keyword list above: `sqlite3_stmt_readonly` itself
/// already returns `false` for `BEGIN IMMEDIATE`/`BEGIN EXCLUSIVE` (checked
/// directly against the vendored `rusqlite`), confirmed here by bypassing
/// the keyword match and calling `rusqlite::Connection::prepare` directly —
/// so even if the keyword list above were ever narrowed to exclude `BEGIN`
/// outright, these two would still be refused by `NotReadOnly`.
#[tokio::test]
async fn begin_immediate_and_exclusive_are_also_rejected_by_readonly() {
    let db = open_test_db();
    for sql in ["BEGIN IMMEDIATE", "BEGIN EXCLUSIVE"] {
        let is_readonly: bool = db
            .with_conn(move |conn| Ok(conn.prepare(sql)?.readonly()))
            .await
            .unwrap();
        assert!(!is_readonly, "sql: {sql:?}");
    }
}

/// RFC 096-B1 stage 0 fix, item 1: `EXPLAIN` and `EXPLAIN QUERY PLAN` are
/// refused unconditionally, including when wrapping an otherwise-harmless
/// `SELECT` — not just when wrapping a refused keyword.
#[tokio::test]
async fn explain_is_refused_unconditionally() {
    let db = open_test_db();
    for sql in [
        "EXPLAIN SELECT 1",
        "EXPLAIN QUERY PLAN SELECT 1",
        "EXPLAIN ATTACH DATABASE '/tmp/does-not-matter.db' AS evil",
    ] {
        let result = db
            .with_read(move |read| {
                let _ = read.prepare(sql)?;
                Ok(())
            })
            .await;
        assert!(
            matches!(result, Err(StoreError::ExplainRefused { .. })),
            "sql: {sql:?}"
        );
    }
}

/// RFC 096-B1 stage 0 fix, item 3: a leading SQL comment must not defeat the
/// keyword check, for either the new class or the pre-existing `PRAGMA`
/// refusal — `/* x */ PRAGMA optimize` is the concrete case this crate found
/// live (module doc): `PRAGMA optimize` reports read-only and can write
/// `ANALYZE` statistics, and the un-hardened check let the comment hide it.
#[tokio::test]
async fn a_leading_comment_does_not_defeat_the_keyword_check() {
    let db = open_test_db();
    for (sql, is_pragma) in [
        (
            "/* x */ ATTACH DATABASE '/tmp/does-not-matter.db' AS evil",
            false,
        ),
        (
            "-- line comment\nATTACH DATABASE '/tmp/does-not-matter.db' AS evil",
            false,
        ),
        ("/* x */ PRAGMA optimize", true),
        ("-- line comment\nPRAGMA optimize", true),
    ] {
        let result = db
            .with_read(move |read| {
                let _ = read.prepare(sql)?;
                Ok(())
            })
            .await;
        if is_pragma {
            assert!(
                matches!(result, Err(StoreError::PragmaRefused { .. })),
                "sql: {sql:?}, got {result:?}"
            );
        } else {
            assert!(
                matches!(result, Err(StoreError::NonContentEffectRefused { .. })),
                "sql: {sql:?}, got {result:?}"
            );
        }
    }
}

/// SQL block comments do not nest — `/* a /* b */ c */` is a terminated
/// comment `/* a /* b */` followed by bare text `c */`, not a balanced pair.
/// `skip_leading_trivia` must match that (stop at the *first* `*/`), not a
/// naive balanced-nesting scan, or it would skip past `BEGIN` below and
/// treat it as still inside the comment.
#[tokio::test]
async fn block_comments_do_not_nest() {
    let db = open_test_db();
    let result = db
        .with_read(|read| {
            let _ = read.prepare("/* a /* b */ BEGIN")?;
            Ok(())
        })
        .await;
    assert!(matches!(
        result,
        Err(StoreError::NonContentEffectRefused { .. })
    ));
}

/// RFC 096-B1 stage 0 fix, item 4: the trap the dispatch named explicitly —
/// a prepare-only assertion would have passed even while `ATTACH` was still
/// exploitable on execution (stage 0's own review proved exactly that). This
/// prepares *and runs* the statement through `with_read`, and asserts the
/// file `ATTACH` would have created does not exist afterward, not just that
/// `prepare` returned an error.
#[tokio::test]
async fn attach_is_refused_and_creates_no_file_even_if_executed() {
    let db = open_test_db();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("evil.db");
    let sql = format!("ATTACH DATABASE '{}' AS evil", target.display());

    let result = db
        .with_read(move |read| {
            let mut stmt = read.prepare(&sql)?;
            // If `prepare` had somehow returned `Ok`, this would be the
            // execution step the dispatch's own first draft never reached —
            // run it too, not just the type check.
            let _ = stmt.query_row([], |_| Ok(()));
            Ok(())
        })
        .await;
    assert!(matches!(
        result,
        Err(StoreError::NonContentEffectRefused { .. })
    ));
    assert!(
        !target.exists(),
        "ATTACH must not create a file, prepared or executed"
    );
}

//! `ReadConn` — a typed read-only handle (RFC 096-B1 stage 0, closing RFC 094
//! `:89-93`'s `ReadConn` prerequisite).
//!
//! `Database::with_conn` hands out `&rusqlite::Connection`, and
//! `Connection::execute` takes `&self` — so a write is reachable from every
//! `with_conn` call site with nothing in the type to stop it (measured in
//! `read-path-audit-result-2026-10-09.md`). `ReadConn` closes that for the
//! call sites that are read-only in fact: it wraps the same `&Connection`
//! `with_conn` already hands out, but exposes no `execute`/`execute_batch`,
//! and the only way to obtain one at all is [`crate::db::Database::with_read`]
//! / `with_read_sync`, which construct it from a connection they already had
//! — nothing new is reachable through it that `with_conn` didn't already
//! reach.
//!
//! **What stops a caller reaching the inner `Connection`.** `ReadConn`'s
//! field is private and [`ReadConn::new`] is `pub(crate)`: nothing outside
//! this crate can construct one, and nothing — inside or outside this crate —
//! can read `conn` back out, because no accessor returns it. A caller only
//! ever sees `&ReadConn`, and `&ReadConn`'s own API (`prepare`, see below) has
//! no path back to `&Connection`.
//!
//! **The per-statement interrogation (RFC 094, 2026-08-12 amendment).** Every
//! statement reaches SQLite only through [`ReadConn::prepare`], which refuses
//! anything `sqlite3_stmt_readonly` does not call read-only
//! (`rusqlite::Statement::readonly`), and refuses, unconditionally and by
//! leading keyword, every statement `sqlite3_stmt_readonly` is documented to
//! report read-only despite the statement having an effect:
//!
//! - **`PRAGMA`** — `rusqlite`'s own source documents this one as unreliable
//!   (`raw_statement.rs`, directly above the FFI call: "does not work for
//!   PRAGMA"), confirmed concretely: `PRAGMA optimize` reports read-only
//!   despite being able to write `ANALYZE` statistics to the database.
//! - **`BEGIN`, `COMMIT`, `END`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`, `ATTACH`,
//!   `DETACH`** — SQLite's own documentation for `sqlite3_stmt_readonly`
//!   names transaction control and `ATTACH`/`DETACH` explicitly as reporting
//!   read-only because they don't change database *file content*, even
//!   though they change connection, transaction, or (for `ATTACH`)
//!   filesystem state. The keyword match below compares only the leading
//!   word, so `BEGIN IMMEDIATE`/`BEGIN EXCLUSIVE` are refused by matching
//!   `BEGIN` the same as plain `BEGIN` is — redundantly with
//!   `sqlite3_stmt_readonly`, which already returns `false` for both
//!   (checked directly), but not relying on that distinction to matter.
//!   `END` is `COMMIT`'s synonym in SQLite's own grammar and is refused for
//!   the same reason, even though it wasn't named in this fix's own
//!   dispatch.
//! - **`EXPLAIN`, `EXPLAIN QUERY PLAN`** — reports the same
//!   `sqlite3_stmt_readonly` value as the statement it wraps (SQLite's own
//!   documentation), so `EXPLAIN ATTACH ...` reports read-only too. Checked
//!   directly that this is not independently exploitable — executing an
//!   `EXPLAIN`-wrapped statement only lists its bytecode and never runs it,
//!   confirmed by `EXPLAIN ATTACH DATABASE '<path>' AS x` not creating
//!   `<path>` — but it defeats a first-keyword check for the statement it
//!   wraps, so it is refused rather than relied on to stay harmless by an
//!   accident of `EXPLAIN`'s own behaviour that nothing guarantees.
//!
//! **A leading SQL comment (`-- ...` or `/* ... */`) defeats a naive
//! "first N bytes" keyword check** — checked directly, and found to be live
//! against the original `PRAGMA`-only check too: `/* x */ PRAGMA optimize`
//! reports read-only and would have reached execution unrefused. The leading-
//! keyword extraction below skips comments and whitespace, repeatedly,
//! before comparing — the same hardened check PRAGMA now shares with the rest
//! of this list, not a separate PRAGMA-only shortcut.
//!
//! Every refusal is an error, not a panic: a future edit that accidentally
//! routes any of the above through a `ReadConn`-typed call site should fail
//! that one request, not take down the process.
//!
//! **The feature assertion (RFC 094, 2026-08-26 amendment)** — that
//! `rusqlite`'s `functions`, `vtab` and `load_extension` features stay
//! disabled — is not this module's job to enforce at runtime: statement-level
//! `readonly()` cannot see a side-effecting application function or virtual
//! table regardless of how carefully this module checks it, so the control
//! has to live where the feature itself would surface — see
//! `crates/sui-id-store/tests/compile_fail/rusqlite_*_feature_must_stay_off.rs`.

use crate::errors::{StoreError, StoreResult};
use rusqlite::Connection;

/// A read-only handle to a SQLite connection. See the module doc for what
/// this does and does not guarantee, and why.
pub struct ReadConn<'a> {
    conn: &'a Connection,
}

impl<'a> ReadConn<'a> {
    /// Only [`crate::db::Database::with_read`] / `with_read_sync` call this —
    /// both already hold the same `&Connection` `with_conn` would have handed
    /// the caller, so this constructs no new access, only a narrower view of
    /// access that already existed.
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Prepare a statement for reading. Refused before SQLite ever sees it if
    /// its leading keyword is `PRAGMA`, `EXPLAIN`, or one of the
    /// transaction-control/connection-config keywords (unconditionally, see
    /// module doc); refused after `rusqlite` prepares it if
    /// `sqlite3_stmt_readonly` does not call it read-only. None of these
    /// refusals has fired against any of this crate's own read call sites —
    /// see the mutation evidence in the stage 0 package and its fix — so
    /// today this is a continuous check with nothing to catch, not a
    /// behaviour change.
    pub fn prepare(&self, sql: &str) -> StoreResult<ReadStatement<'a>> {
        let keyword = leading_keyword(sql);
        if keyword.eq_ignore_ascii_case("PRAGMA") {
            return Err(StoreError::PragmaRefused {
                sql: sql.to_owned(),
            });
        }
        if keyword.eq_ignore_ascii_case("EXPLAIN") {
            return Err(StoreError::ExplainRefused {
                sql: sql.to_owned(),
            });
        }
        if NON_CONTENT_EFFECT_KEYWORDS
            .iter()
            .any(|k| keyword.eq_ignore_ascii_case(k))
        {
            return Err(StoreError::NonContentEffectRefused {
                keyword: keyword.to_ascii_uppercase(),
                sql: sql.to_owned(),
            });
        }
        let stmt = self.conn.prepare(sql)?;
        if !stmt.readonly() {
            return Err(StoreError::NotReadOnly {
                sql: sql.to_owned(),
            });
        }
        Ok(ReadStatement { stmt })
    }
}

/// A statement prepared through [`ReadConn::prepare`]: already interrogated
/// and known read-only. Offers `query_row` / `query_map`, the two shapes
/// every read call site in this crate uses, and nothing execute-shaped.
pub struct ReadStatement<'a> {
    stmt: rusqlite::Statement<'a>,
}

impl ReadStatement<'_> {
    /// Mirrors `rusqlite::Statement::query_row` exactly (same signature, same
    /// error type) so converting a call site is a type change, not a
    /// behaviour change.
    pub fn query_row<T, P, F>(&mut self, params: P, f: F) -> rusqlite::Result<T>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        self.stmt.query_row(params, f)
    }

    /// Mirrors `rusqlite::Statement::query_map` exactly, same reason as
    /// `query_row` above.
    pub fn query_map<T, P, F>(
        &mut self,
        params: P,
        f: F,
    ) -> rusqlite::Result<rusqlite::MappedRows<'_, F>>
    where
        P: rusqlite::Params,
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        self.stmt.query_map(params, f)
    }
}

/// Transaction-control and connection-config keywords `sqlite3_stmt_readonly`
/// reports read-only despite their effect (see the module doc). `PRAGMA` and
/// `EXPLAIN` are handled separately in `prepare()` since each gets its own
/// error variant.
const NON_CONTENT_EFFECT_KEYWORDS: &[&str] = &[
    "BEGIN",
    "COMMIT",
    "END",
    "ROLLBACK",
    "SAVEPOINT",
    "RELEASE",
    "ATTACH",
    "DETACH",
];

/// Skip leading whitespace and SQL comments (`-- ...` to end of line, or
/// `/* ... */`), repeatedly, so a comment cannot be used to hide a refused
/// keyword from [`leading_keyword`] — checked directly to matter: without
/// this, `/* x */ PRAGMA optimize` reached `sqlite3_stmt_readonly` unrefused
/// (see the module doc). An unterminated `/* ...` with no closing `*/` is not
/// valid SQL either way, so it is returned as-is rather than treated as "the
/// rest of the string is a comment" — `prepare()` rejects it on its own.
fn skip_leading_trivia(sql: &str) -> &str {
    let mut rest = sql;
    loop {
        let trimmed = rest.trim_start();
        if let Some(after_dashes) = trimmed.strip_prefix("--") {
            rest = match after_dashes.find('\n') {
                Some(newline) => &after_dashes[newline + 1..],
                None => "",
            };
        } else if let Some(after_open) = trimmed.strip_prefix("/*") {
            match after_open.find("*/") {
                Some(close) => rest = &after_open[close + 2..],
                None => return trimmed,
            }
        } else {
            return trimmed;
        }
    }
}

/// The statement's leading keyword: the longest run of ASCII letters after
/// skipping leading trivia. A whole-word extraction rather than a
/// fixed-length slice, so e.g. an identifier beginning `ENDOFSOMETHING`
/// cannot be mistaken for the keyword `END` — every top-level SQLite
/// statement begins with a reserved keyword, never a bare identifier, so
/// this never has to disambiguate a real collision, but a whole-word compare
/// is the correct primitive regardless of keyword length (`END` is 3 bytes,
/// `SAVEPOINT` is 9 — the fixed 6-byte slice this replaced only ever worked
/// for `PRAGMA` by coincidence).
fn leading_keyword(sql: &str) -> &str {
    let trimmed = skip_leading_trivia(sql);
    let end = trimmed
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(trimmed.len());
    &trimmed[..end]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

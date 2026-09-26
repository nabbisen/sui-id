//! Opening the database, or refusing to (RFC 112 D5).
//!
//! `Database::open` refuses a database this build does not understand (RFC 112
//! stage 1). This module is what the **operator** sees of that refusal, and it is
//! the one place that decides it: `startup::prepare` and every CLI subcommand that
//! opens the database call [`open`], so there is one line, one exit code and one
//! `tracing` event, not eight wrappers that each bury the cause under
//! `Error: opening database … Caused by:`.
//!
//! The line goes to **stderr, first and alone**, names the path, both versions
//! and the release that stamped the database when it recorded one, says the route
//! back and what **not** to do, and contains "schema" and "migrat" so the
//! `journalctl | grep -i migrat` that `deployment.md` teaches finds it. The exit
//! code is [`EXIT_DATABASE_REFUSED`] for **both** refusal variants: a code that
//! covers one of them covers nothing, because `RestartPreventExitStatus=65` is set
//! once.
//!
//! The exit-code convention this establishes is deliberately two lines long:
//! `0` success; `65` the database is not one this build can use; `1` everything
//! else.

use anyhow::Context;
use std::path::Path;
use sui_id_store::crypto::MasterKey;
use sui_id_store::{Database, StoreError};

/// `EX_DATAERR` (`sysexits.h`): the input data was incorrect. It collides with
/// neither the shell's reserved codes (126-128, 128+signal) nor systemd's own
/// (200-242).
pub const EXIT_DATABASE_REFUSED: u8 = 65;

/// The database was refused. Carried through `anyhow` so that every caller's `?`
/// works and `main` alone turns it into the line and the exit code.
#[derive(Debug)]
pub struct DatabaseRefused {
    line: String,
}

impl DatabaseRefused {
    /// The single line the operator reads.
    pub fn line(&self) -> &str {
        &self.line
    }
}

impl std::fmt::Display for DatabaseRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.line)
    }
}

impl std::error::Error for DatabaseRefused {}

/// Open the database at `path`, or refuse with [`DatabaseRefused`]. Every other
/// failure keeps its ordinary shape and exit code `1`.
pub fn open(path: &Path, key: MasterKey) -> anyhow::Result<Database> {
    match Database::open(path, key) {
        Ok(db) => Ok(db),
        Err(err) => match refusal(path, &err) {
            Some(refused) => Err(refused.into()),
            None => Err(anyhow::Error::new(err)).context("opening database"),
        },
    }
}

/// Refuse now, without opening the database for writing, if it is one this build
/// cannot use. For an operation that prints something before it opens (`admin
/// rotate-key` prints a summary and asks for confirmation first): the refusal must
/// still be the first line on stderr, and a database that will be refused should
/// not be described, confirmed and then refused.
pub fn check(path: &Path) -> anyhow::Result<()> {
    match sui_id_store::migrations::check_database_file(path) {
        Ok(()) => Ok(()),
        Err(err) => match refusal(path, &err) {
            Some(refused) => Err(refused.into()),
            None => Err(anyhow::Error::new(err)).context("checking the database"),
        },
    }
}

/// If `err` is one of the two schema refusals, log it once and build the line.
fn refusal(path: &Path, err: &StoreError) -> Option<DatabaseRefused> {
    let stamped_by = || sui_id_store::migrations::last_migrated_by_file(path);
    let line = match err {
        StoreError::SchemaTooNew { found, supported } => {
            let stamped_by = stamped_by();
            tracing::error!(
                refusal = "schema_too_new",
                found = *found,
                supported = *supported,
                db_path = %path.display(),
                last_migrated_by = stamped_by.as_deref().unwrap_or(""),
                "refusing to use the database: its schema is newer than this build understands"
            );
            too_new_line(path, *found, *supported, stamped_by.as_deref())
        }
        StoreError::SchemaVersionInvalid { tables, detail } => {
            tracing::error!(
                refusal = "schema_version_invalid",
                tables = *tables,
                detail = detail.as_str(),
                db_path = %path.display(),
                "refusing to use the database: its schema version cannot be trusted"
            );
            invalid_line(path, *tables, detail)
        }
        _ => return None,
    };
    Some(DatabaseRefused { line })
}

const RESTORE: &str = "sui-id restore --config <config> --from <backup.tar> --force";

/// The line for a database that is newer than this build.
pub fn too_new_line(path: &Path, found: i64, supported: i32, stamped_by: Option<&str>) -> String {
    let (stamp, run) = match stamped_by {
        Some(v) => (
            format!(" (it was last migrated by sui-id {v})"),
            format!("sui-id {v} or newer"),
        ),
        None => (String::new(), "a newer sui-id".to_owned()),
    };
    one_line(&format!(
        "sui-id: refusing to run: the database at {} is at schema version {found}, \
         but this sui-id {} understands up to {supported}{stamp}. \
         Run {run}, or restore the pre-upgrade backup with \"{RESTORE}\". \
         Nothing was changed. Do not edit or delete the database, and do not run migrations by hand.",
        path.display(),
        env!("CARGO_PKG_VERSION"),
    ))
}

/// The line for a database whose schema version cannot be trusted.
pub fn invalid_line(path: &Path, tables: usize, detail: &str) -> String {
    one_line(&format!(
        "sui-id: refusing to run: the schema version recorded in the database at {} is unreadable \
         ({detail}; {tables} application table(s)), so no migrations were run. \
         Nothing was changed. Restore the most recent backup with \"{RESTORE}\", \
         and keep this file for inspection. Do not edit or delete the database.",
        path.display(),
    ))
}

/// One line, whatever a path or a stored value contained: a control character
/// (a newline in a path, an escape in a stored value) is replaced, so nothing in
/// the message can break the "first and alone" property.
fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "database/tests.rs"]
mod tests;

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
use sui_id_store::backup::{self, FreeSpaceCheck};
use sui_id_store::crypto::MasterKey;
use sui_id_store::migrations::{MAX_SCHEMA_VERSION, Preview};
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

/// RFC 142 stage 5 D4: whether [`open`] may take an automatic snapshot
/// before a migration it is about to run. There is no default — every
/// caller must name one, so a new call site cannot silently skip the
/// decision (the compiler enforces it: this is a required parameter, not
/// an optional one, a global, or an environment variable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreMigrationBackup {
    /// Take the snapshot if a migration will actually run.
    Take,
    /// `--no-backup`: migrate without one, but still say so.
    Declined,
}

/// `--no-backup` is a boolean flag like any other (it is never in
/// `VALUE_TAKING_FLAGS`): its presence anywhere in `args` declines the
/// automatic pre-migration snapshot for this invocation. The one place
/// that reads the flag, so every caller — the server's own start and
/// every subcommand that reaches [`open`] — asks it the same way.
pub fn backup_policy_from_args(args: &[String]) -> PreMigrationBackup {
    if args.iter().any(|a| a == "--no-backup") {
        PreMigrationBackup::Declined
    } else {
        PreMigrationBackup::Take
    }
}

/// A refusal that is not about the database's schema (RFC 142 stage 5 §5) —
/// not enough free space for the pre-migration snapshot, or the snapshot
/// itself failed to write. Carried through `anyhow` the same way
/// [`DatabaseRefused`] is, but with exit code **1**, not 65: this is a
/// disk problem, not "the database is not one this build can use." A
/// restart loop under systemd is harmless here, because the check that
/// refused wrote nothing.
#[derive(Debug)]
pub struct PreMigrationBackupRefused {
    line: String,
}

impl PreMigrationBackupRefused {
    pub fn line(&self) -> &str {
        &self.line
    }
}

impl std::fmt::Display for PreMigrationBackupRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.line)
    }
}

impl std::error::Error for PreMigrationBackupRefused {}

/// Open the database at `path`, or refuse with [`DatabaseRefused`]. Every other
/// failure keeps its ordinary shape and exit code `1`.
///
/// RFC 142 stage 5: before the open that may migrate, this takes an
/// automatic pre-migration snapshot — unless `backup` is
/// [`PreMigrationBackup::Declined`], or no migration will actually run.
/// Sequence: the existing schema refusal first (via [`sui_id_store::migrations::preview`],
/// the same reader [`preview`] uses — a database this build refuses gets no
/// backup and the same line as ever); only [`Preview::Pending`] takes a
/// backup; free space is checked before any byte is written; the snapshot
/// is taken; only then does this call `Database::open`, which migrates.
///
/// **A race, and why it is acceptable:** another process may migrate the
/// database between the `preview` read above and the `Database::open`
/// below. Then the snapshot is of the pre-migration state, as intended, or
/// of an already-migrated one — an unneeded but harmless extra file.
/// Neither loses data.
pub fn open(path: &Path, key: MasterKey, backup: PreMigrationBackup) -> anyhow::Result<Database> {
    let outcome = match sui_id_store::migrations::preview(path) {
        Ok(outcome) => outcome,
        Err(err) => {
            let store_err = StoreError::from(err);
            return match refusal(path, &store_err) {
                Some(refused) => Err(refused.into()),
                None => Err(anyhow::Error::new(store_err)).context("opening database"),
            };
        }
    };

    if let Preview::Pending { current, .. } = outcome {
        let to = MAX_SCHEMA_VERSION;
        match backup {
            PreMigrationBackup::Declined => {
                eprintln!(
                    "sui-id: --no-backup: migrating schema {current} → {to} without a backup."
                );
                tracing::warn!(
                    from = current,
                    to,
                    "migrating without a pre-migration backup (--no-backup)"
                );
            }
            PreMigrationBackup::Take => {
                let check = measure_free_space(path);
                if check.is_short() {
                    let line = short_space_line(path, &check);
                    tracing::error!(
                        db_path = %path.display(),
                        needed_bytes = check.needed_bytes,
                        available_bytes = check.available_bytes.unwrap_or(0),
                        "refusing to migrate: not enough free space for the pre-migration backup"
                    );
                    return Err(PreMigrationBackupRefused { line }.into());
                }
                match backup::pre_migration_snapshot(path, current, to) {
                    Ok(snapshot_path) => {
                        eprintln!(
                            "sui-id: backed up the database to {} before migrating schema {current} \
                             → {to}. To undo the upgrade, stop sui-id and see \"Rollback\" in \
                             docs/src/guides/upgrade.md.",
                            snapshot_path.display()
                        );
                        tracing::info!(
                            pre_migration_backup = %snapshot_path.display(),
                            from = current,
                            to,
                            "pre-migration backup taken"
                        );
                    }
                    Err(e) => {
                        let line = backup_write_failed_line(path, &e.to_string());
                        tracing::error!(
                            db_path = %path.display(),
                            error = %e,
                            "refusing to migrate: the pre-migration backup could not be written"
                        );
                        return Err(PreMigrationBackupRefused { line }.into());
                    }
                }
            }
        }
    }

    match Database::open(path, key) {
        Ok(db) => Ok(db),
        Err(err) => match refusal(path, &err) {
            Some(refused) => Err(refused.into()),
            None => Err(anyhow::Error::new(err)).context("opening database"),
        },
    }
}

/// A size for an operator-facing line. sui-id databases are often smaller
/// than a megabyte, and a whole-megabyte figure then reads "about 0 MB",
/// which says nothing true; below 1 MB this says "under 1 MB" instead.
/// Above it, "about N MB", where `round_up` rounds a requirement up (never
/// understate what is needed) and an availability down (never overstate
/// what is there).
fn size_for_operator(bytes: u64, round_up: bool) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes < MB {
        return "under 1 MB".to_owned();
    }
    let n = if round_up {
        bytes.div_ceil(MB)
    } else {
        bytes / MB
    };
    format!("about {n} MB")
}

/// The real measurement [`open`] uses, unless a test has overridden it
/// (§8's "free space short" test: injected through this seam, never by
/// filling a disk). **Thread-local**, not process-global: Rust's test
/// runner runs `#[test]` functions concurrently on separate threads, and a
/// process-global override would leak into every other test's real
/// `open()` call racing on another thread. Nothing in the production build
/// can reach it at all (`test-support` is off by default).
fn measure_free_space(path: &Path) -> FreeSpaceCheck {
    #[cfg(feature = "test-support")]
    if let Some(check) = FREE_SPACE_OVERRIDE.with(|cell| *cell.borrow()) {
        return check;
    }
    backup::check_free_space(path)
}

#[cfg(feature = "test-support")]
thread_local! {
    static FREE_SPACE_OVERRIDE: std::cell::RefCell<Option<FreeSpaceCheck>> =
        const { std::cell::RefCell::new(None) };
}

/// Test-only seam (RFC 142 stage 5 §8): force [`open`]'s free-space check to
/// a given value, for the rest of this *thread*, without touching any real
/// filesystem. `None` clears the override. Only exists under
/// `test-support`.
#[cfg(feature = "test-support")]
pub fn set_free_space_override_for_test(check: Option<FreeSpaceCheck>) {
    FREE_SPACE_OVERRIDE.with(|cell| *cell.borrow_mut() = check);
}

/// The refusal line for §5's free-space check.
fn short_space_line(path: &Path, check: &FreeSpaceCheck) -> String {
    one_line(&format!(
        "sui-id: refusing to migrate: the pre-upgrade backup of {} needs {} free beside it \
         (room for the backup and for the migration after it), and {} is available. Nothing was \
         changed. Free some space, or start with --no-backup if you have taken a backup yourself.",
        path.display(),
        size_for_operator(check.needed_bytes, true),
        size_for_operator(check.available_bytes.unwrap_or(0), false),
    ))
}

/// The refusal line for a pre-migration snapshot that failed to write, for
/// any reason other than free space (which [`short_space_line`] covers).
fn backup_write_failed_line(path: &Path, cause: &str) -> String {
    one_line(&format!(
        "sui-id: refusing to migrate: could not write the pre-upgrade backup for {}: {cause}. \
         Nothing was changed. Start with --no-backup if you have taken a backup yourself, or fix \
         the problem and try again.",
        path.display(),
    ))
}

/// The line `preview-upgrade` prints in its `Pending` case, before "Nothing
/// was changed." (§6): predicts exactly what [`open`] will decide, using
/// the same [`FreeSpaceCheck`] — so the two can never disagree about the
/// numbers.
pub fn backup_prediction_line(check: &FreeSpaceCheck) -> String {
    let backup_size = size_for_operator(check.snapshot_bytes(), true);
    let needed = size_for_operator(check.needed_bytes, true);
    match check.available_bytes {
        None => format!(
            "Before migrating, this sui-id writes a backup of {backup_size} beside the \
             database (free space not checked on this platform) unless started with --no-backup."
        ),
        Some(avail) if check.is_short() => format!(
            "Before migrating, this sui-id writes a backup of {backup_size} beside the \
             database, but it needs {needed} free and {} is available: not enough space, so this \
             sui-id would refuse to migrate unless started with --no-backup.",
            size_for_operator(avail, false)
        ),
        Some(avail) => format!(
            "Before migrating, this sui-id writes a backup of {backup_size} beside the \
             database ({needed} free needed, {} available) unless started with --no-backup.",
            size_for_operator(avail, false)
        ),
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

/// Preview what this build would do to the database at `path`, without
/// writing anything and without resolving a master key (RFC 142 stage 4). A
/// refused database produces exactly the same [`DatabaseRefused`] — same
/// line, same `tracing` event, same exit code — as [`check`] and [`open`],
/// because both go through [`refusal`].
pub fn preview(path: &Path) -> anyhow::Result<sui_id_store::migrations::Preview> {
    match sui_id_store::migrations::preview(path) {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            let store_err = StoreError::from(err);
            match refusal(path, &store_err) {
                Some(refused) => Err(refused.into()),
                None => Err(anyhow::Error::new(store_err)).context("previewing the upgrade"),
            }
        }
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

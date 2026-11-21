//! RFC 100 stage 4: startup recovery of the preparation prefix
//! (`crash-matrix.md` rows 1-7 and 11). Runs before the database is
//! opened for normal use -- `database::open_checked` calls it first, and
//! `admin rotate-key` calls it explicitly, since it resolves its key
//! through `keyring::resolve_checked` directly rather than through
//! `open_checked`.
//!
//! **Decision (2026-11-21): clean and start, never resume.** A rotation
//! that stopped before the database was resealed is cleaned -- every
//! leftover verified against the authenticated journal, removed, and
//! named -- and the service continues on the active (old) key. To
//! rotate, the operator runs the command again.
//!
//! **Decision (2026-11-21): a verified pre-rotation backup is removed.**
//! Keeping it would leave an untracked second copy of the still-active
//! key once the journal that recorded its disposition is gone (M5).
//!
//! **Rows 8-10 are not here.** They read the database as already
//! resealed (K07 committed) or activation already run, and they are
//! this stage's own scoping decision: a workspace beside a database the
//! active key does not open fails closed instead (row 11's outcome),
//! safe because nothing is removed that might be the only key the
//! database opens with. Stage 6 builds those rows, recovery first
//! within it.

use crate::key_adapter::KeyFileOps;
use crate::rotate::journal::{self, JournalBody};
use crate::rotate::preconditions::RotationPaths;
use anyhow::Context;
use std::path::PathBuf;
use sui_id_store::crypto::{self, KeyRole, MasterKey};
use sui_id_store::master_key_check::{self, Inspection};

/// What [`recover`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovered {
    /// No workspace -- row 1. Normal startup; nothing touched.
    Nothing,
    /// A leftover workspace was verified and cleaned. `removed` is
    /// exactly what *this* run removed, in the order it removed them --
    /// a re-run after a crash mid-cleanup removes only what is still
    /// there, so this can be shorter than a first, uninterrupted run.
    /// `active_fingerprint` is the key sui-id continues on, for the
    /// report -- never the key itself.
    Cleaned {
        removed: Vec<PathBuf>,
        active_fingerprint: String,
    },
}

fn fail_closed(paths: &RotationPaths, detail: &str) -> anyhow::Error {
    // The detail is a sentence of its own, wherever it was worded.
    let mut chars = detail.chars();
    let detail: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    };
    anyhow::anyhow!(
        "refusing to open the database: a key rotation stopped part-way, and \
         what it left does not check out. {detail} Nothing was changed. Keep \
         {}, {} and, if they exist, {} and {} exactly as they are: do not \
         remove or edit them by hand, since one of them may be needed to \
         recover.",
        paths.workspace.display(),
        paths.active.display(),
        paths.next.display(),
        paths.backup.display(),
    )
}

/// RFC 100 stage 4 §3: classify and, if safe, clean a leftover
/// preparation workspace. Every file operation goes through `adapter`.
/// Any failed check fails closed: nothing is changed, and the error
/// names what was found and which check failed.
pub fn recover(
    adapter: &dyn KeyFileOps,
    key_file: &std::path::Path,
    db_path: &std::path::Path,
) -> anyhow::Result<Recovered> {
    let paths = RotationPaths::for_key_file(key_file);

    // Row 1: no workspace at all.
    let workspace_meta = match adapter.metadata("recovery.workspace", &paths.workspace) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Recovered::Nothing),
        Err(e) => {
            return Err(
                anyhow::Error::new(e).context(format!("checking {}", paths.workspace.display()))
            );
        }
    };

    // Step 2: the workspace itself.
    if workspace_meta.is_symlink {
        return Err(fail_closed(
            &paths,
            &format!(
                "{} is a symlink, not a directory.",
                paths.workspace.display()
            ),
        ));
    }
    if !workspace_meta.is_dir {
        return Err(fail_closed(
            &paths,
            &format!("{} is not a directory.", paths.workspace.display()),
        ));
    }
    if workspace_meta.mode != 0o700 {
        return Err(fail_closed(
            &paths,
            &format!(
                "{} has mode {:o}, not 0700.",
                paths.workspace.display(),
                workspace_meta.mode
            ),
        ));
    }
    let my_uid = rustix::process::geteuid().as_raw();
    if workspace_meta.uid != my_uid {
        return Err(fail_closed(
            &paths,
            &format!(
                "{} is owned by uid {}, not the uid this sui-id runs as ({my_uid}).",
                paths.workspace.display(),
                workspace_meta.uid
            ),
        ));
    }

    // Step 3: the active key.
    // The keyring's own reader, so recovery and startup can never disagree
    // about where the active key came from.
    let resolved = crate::keyring::read_existing_impl(adapter, &paths.active)
        .map_err(|e| fail_closed(&paths, &format!("{e:#}.")))?;
    let active_key = match resolved {
        Some(r) if r.origin == crate::keyring::KeyOrigin::KeyFile => r.key,
        Some(_) => {
            return Err(fail_closed(
                &paths,
                "The master key comes from the environment, not from the key file, and a \
                 rotation only ever runs on a key file's key.",
            ));
        }
        None => {
            return Err(fail_closed(
                &paths,
                &format!("There is no key at {}.", paths.active.display()),
            ));
        }
    };
    let conn = sui_id_store::migrations::open_preview_connection(db_path).with_context(|| {
        format!(
            "opening {} read-only to check the master key",
            db_path.display()
        )
    })?;
    let inspection = master_key_check::inspect(&conn).with_context(|| {
        format!(
            "reading the master key check state of {}",
            db_path.display()
        )
    })?;
    let sentinel = match inspection {
        Inspection::Sentinel(row) => row,
        // No sentinel at all beside an existing workspace is not a state
        // rows 1-7 can produce (the sentinel predates stage 3's own
        // first write); treat it the same as a key that does not open
        // one -- fail closed rather than guess which row this is.
        Inspection::SealedNoSentinel { .. } | Inspection::EmptyNoSentinel | Inspection::NoFile => {
            return Err(fail_closed(
                &paths,
                "the database has no master key sentinel yet, which a rotation workspace should \
                 never be able to produce.",
            ));
        }
    };
    if !master_key_check::sentinel_opens(&active_key, &sentinel.sealed) {
        return Err(fail_closed(
            &paths,
            &format!(
                "the key at {} does not open the database's master key sentinel -- the database \
                 may already be sealed under a different key (fingerprint {}).",
                paths.active.display(),
                sentinel.key_fingerprint
            ),
        ));
    }
    let active_fingerprint = crypto::fingerprint(&active_key);

    // List the workspace once; every later step classifies from this.
    let entries = adapter
        .list_dir("recovery.workspace.list", &paths.workspace)
        .with_context(|| format!("listing {}", paths.workspace.display()))?;

    // Every entry, whatever its name, must be a plain file: a directory
    // or a symlink is nothing a rotation writes, and checking now, before
    // anything is removed, keeps "nothing was changed" true when it fails.
    for entry in &entries {
        let meta = adapter
            .metadata("recovery.workspace.entry", entry)
            .with_context(|| format!("checking {}", entry.display()))?;
        if meta.is_symlink || meta.is_dir {
            return Err(fail_closed(
                &paths,
                &format!(
                    "{} is a symlink or a directory, not a file a rotation writes.",
                    entry.display()
                ),
            ));
        }
    }

    let published = paths.workspace.join("intent.json");
    let has_intent_json = entries.iter().any(|p| p == &published);

    let body = if !has_intent_json {
        // Step 4, row 2: every entry must be a bound partial-intent
        // temporary -- nothing has ever been authenticated here yet, so
        // nothing names which temporaries are legitimate except the two
        // that can exist before any journal is published.
        for entry in &entries {
            let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !matches!(name, "intent.write" | "update.write") {
                return Err(fail_closed(
                    &paths,
                    &format!(
                        "{} holds an unexpected entry, {}, before any rotation journal was ever \
                         published.",
                        paths.workspace.display(),
                        entry.display()
                    ),
                ));
            }
        }
        // No journal binds a published artifact yet, so none may exist
        // (M3): a `next` or `backup` here is not this rotation's, and is
        // never removed unverified.
        for (artifact, label) in [
            (&paths.next, "next key"),
            (&paths.backup, "pre-rotation backup"),
        ] {
            match adapter.metadata("recovery.row2.artifact", artifact) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(fail_closed(
                        &paths,
                        &format!(
                            "A {label} exists at {}, but no rotation journal was ever published to \
                             bind it.",
                            artifact.display()
                        ),
                    ));
                }
                Err(e) => {
                    return Err(
                        anyhow::Error::new(e).context(format!("checking {}", artifact.display()))
                    );
                }
            }
        }
        None
    } else {
        // Step 5: `intent.json` is published -- authenticate it with the
        // active key and bind it to this exact deployment.
        let (body, role) = journal::read(adapter, &paths.workspace, &active_key)
            .map_err(|e| fail_closed(&paths, &format!("{e:#}.")))?;
        if role != KeyRole::Old {
            return Err(fail_closed(
                &paths,
                "the rotation journal authenticates the active key as the *new* role, which \
                 means activation may already have started -- that is a later row this sui-id \
                 does not recover yet.",
            ));
        }
        if body.old_fingerprint != active_fingerprint {
            return Err(fail_closed(
                &paths,
                &format!(
                    "the journal's old key fingerprint ({}) does not match the active key's \
                     ({active_fingerprint}).",
                    body.old_fingerprint
                ),
            ));
        }
        check_bound_paths(&paths, &body)?;

        let bound_temps = [
            &paths.intent_temp,
            &paths.update_temp,
            &paths.next_temp,
            &paths.backup_temp,
        ];
        for entry in &entries {
            if entry == &published {
                continue;
            }
            if !bound_temps.contains(&entry) {
                return Err(fail_closed(
                    &paths,
                    &format!(
                        "{} holds an entry the journal does not bind, {}.",
                        paths.workspace.display(),
                        entry.display()
                    ),
                ));
            }
        }
        Some(body)
    };

    // Step 6: the published artifacts outside the workspace.
    if let Some(body) = &body {
        check_published_artifact(
            adapter,
            &paths.next,
            body.expected_uid,
            body.expected_mode,
            &body.new_fingerprint,
            "next key",
        )
        .map_err(|e| fail_closed(&paths, &format!("{e:#}.")))?;
        check_published_artifact(
            adapter,
            &paths.backup,
            body.expected_uid,
            body.expected_mode,
            &active_fingerprint,
            "pre-rotation backup (a copy of the active key)",
        )
        .map_err(|e| fail_closed(&paths, &format!("{e:#}.")))?;
    }

    // Step 7: clean, in the dispatch's exact order. Every removal
    // tolerates `NotFound`, so a re-run after a crash mid-cleanup
    // finishes the job instead of failing on what an earlier run already
    // removed.
    let mut removed = Vec::new();
    let mut remove = |adapter: &dyn KeyFileOps, point: &str, path: &std::path::Path| {
        match adapter.remove(point, path) {
            Ok(()) => removed.push(path.to_path_buf()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::Error::new(e).context(format!("removing {}", path.display())));
            }
        }
        Ok(())
    };

    // (a) Published artifacts first, while the journal that binds them
    // still exists.
    remove(adapter, "recovery.clean.backup", &paths.backup)?;
    remove(adapter, "recovery.clean.next", &paths.next)?;
    crate::keyring::sync_parent_dir_impl(adapter, "recovery.clean.sync_active_dir", &paths.active)
        .context("syncing the key file's directory after cleanup")?;

    // (b) Every bound temporary inside the workspace.
    remove(adapter, "recovery.clean.next_temp", &paths.next_temp)?;
    remove(adapter, "recovery.clean.backup_temp", &paths.backup_temp)?;
    remove(adapter, "recovery.clean.update_temp", &paths.update_temp)?;
    remove(adapter, "recovery.clean.intent_temp", &paths.intent_temp)?;
    crate::keyring::sync_parent_dir_impl(
        adapter,
        "recovery.clean.sync_workspace_b",
        &paths.intent_temp,
    )
    .context("syncing the workspace after removing its temporaries")?;

    // (c) `intent.json`, after (b) is durable -- never in the same sync
    // (a power loss after this step then leaves an empty workspace,
    // never row 2 with a `next.write` in it, which step 4 would refuse).
    remove(adapter, "recovery.clean.intent_json", &published)?;
    crate::keyring::sync_parent_dir_impl(adapter, "recovery.clean.sync_workspace_c", &published)
        .context("syncing the workspace after removing the journal")?;

    // (d) The workspace itself.
    match adapter.remove_dir("recovery.clean.workspace", &paths.workspace) {
        Ok(()) => removed.push(paths.workspace.clone()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(anyhow::Error::new(e).context("removing the workspace")),
    }
    crate::keyring::sync_parent_dir_impl(adapter, "recovery.clean.sync_parent", &paths.workspace)
        .context("syncing the workspace's parent directory after removing it")?;

    Ok(Recovered::Cleaned {
        removed,
        active_fingerprint,
    })
}

/// Every one of a journal's eight bound paths must equal what this exact
/// key file derives -- RFC 100's own rule, "act only on paths the
/// journal binds." A journal sealed under the right keys but naming a
/// different workspace or key file is not this deployment's.
fn check_bound_paths(paths: &RotationPaths, body: &JournalBody) -> anyhow::Result<()> {
    let expected = [
        ("active_path", &body.active_path, &paths.active),
        ("workspace_path", &body.workspace_path, &paths.workspace),
        ("next_path", &body.next_path, &paths.next),
        ("backup_path", &body.backup_path, &paths.backup),
        (
            "intent_temp_path",
            &body.intent_temp_path,
            &paths.intent_temp,
        ),
        (
            "update_temp_path",
            &body.update_temp_path,
            &paths.update_temp,
        ),
        ("next_temp_path", &body.next_temp_path, &paths.next_temp),
        (
            "backup_temp_path",
            &body.backup_temp_path,
            &paths.backup_temp,
        ),
    ];
    for (field, bound, actual) in expected {
        if bound.as_str() != actual.to_string_lossy() {
            anyhow::bail!(
                "the journal's {field} ({bound}) does not match this deployment's own ({})",
                actual.display()
            );
        }
    }
    Ok(())
}

/// If `path` exists, it must be a regular file, not a symlink, with
/// `expected_uid`/`expected_mode`, decoding to a key whose fingerprint
/// is `expected_fingerprint`. Absent is fine -- not every row has
/// published every artifact yet.
fn check_published_artifact(
    adapter: &dyn KeyFileOps,
    path: &std::path::Path,
    expected_uid: u32,
    expected_mode: u32,
    expected_fingerprint: &str,
    label: &str,
) -> anyhow::Result<()> {
    let meta = match adapter.metadata("recovery.artifact.metadata", path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow::Error::new(e).context(format!("checking {}", path.display()))),
    };
    if meta.is_symlink {
        anyhow::bail!(
            "the {label} at {} is a symlink, not a regular file",
            path.display()
        );
    }
    if meta.uid != expected_uid || meta.mode != expected_mode {
        anyhow::bail!(
            "the {label} at {} has owner/mode {}:{:o}, not the bound {}:{:o}",
            path.display(),
            meta.uid,
            meta.mode,
            expected_uid,
            expected_mode
        );
    }
    let key =
        read_key_file(adapter, path).with_context(|| format!("reading {}", path.display()))?;
    let fingerprint = crypto::fingerprint(&key);
    if fingerprint != expected_fingerprint {
        anyhow::bail!(
            "the {label} at {} has fingerprint {fingerprint}; it should be {expected_fingerprint}",
            path.display()
        );
    }
    Ok(())
}

fn read_key_file(adapter: &dyn KeyFileOps, path: &std::path::Path) -> anyhow::Result<MasterKey> {
    let bytes = adapter.read("recovery.read_key", path)?;
    let s = String::from_utf8(bytes)
        .with_context(|| format!("{} is not valid UTF-8", path.display()))?;
    MasterKey::from_base64(s.trim()).with_context(|| format!("{} is malformed", path.display()))
}

/// RFC 100 stage 4 §3's own report, for the caller to print or log --
/// never from inside `recover` itself, so a server start and a CLI
/// command can place it differently (the server's log, at warn; a CLI
/// command's stderr, first). Empty for [`Recovered::Nothing`]: the usual
/// case, and silence is the right report for it.
pub fn report(recovered: &Recovered) -> Vec<String> {
    match recovered {
        Recovered::Nothing => Vec::new(),
        Recovered::Cleaned {
            removed,
            active_fingerprint,
        } => {
            let mut lines = vec![format!(
                "a stopped rotation was found and cleaned; sui-id continues on the active key \
                 (fingerprint {active_fingerprint})"
            )];
            for path in removed {
                lines.push(format!("  removed {}", path.display()));
            }
            lines.push("to rotate again, run `sui-id admin master-key rotate`.".to_owned());
            lines
        }
    }
}

/// Runs [`recover`] and reports the result on both channels the
/// dispatch names: `tracing::warn!` (the server's log) and `eprintln!`
/// (a CLI command's stderr, before anything else it prints -- true as
/// long as this runs before that command's own first print, which every
/// call site here does). [`Recovered::Nothing`], the usual case, reports
/// nothing on either channel.
pub fn recover_and_report(
    adapter: &dyn KeyFileOps,
    key_file: &std::path::Path,
    db_path: &std::path::Path,
) -> anyhow::Result<Recovered> {
    let recovered = recover(adapter, key_file, db_path)?;
    for line in report(&recovered) {
        tracing::warn!("{line}");
        eprintln!("{line}");
    }
    Ok(recovered)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "recovery/tests.rs"]
mod tests;

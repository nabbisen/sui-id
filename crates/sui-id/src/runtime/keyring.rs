//! Master key acquisition.
//!
//! Resolution order:
//!
//! 1. `SUI_ID_MASTER_KEY` environment variable: a base64-encoded 32-byte
//!    value. Most operationally clean for container deployments.
//! 2. The `key_file` from the config: created on first run with permissions
//!    `0600` and a fresh random key. Subsequent starts read it as-is.
//!
//! The key never lands in the TOML config file. Loss of the key means loss
//! of all sealed columns; the operator is responsible for backup.

use crate::key_adapter::{self, KeyFileOps};
use anyhow::{Context, Result};
use std::path::Path;
use sui_id_store::crypto::MasterKey;
use sui_id_store::master_key_check::{self, Inspection, KeyCheckDecision};

const ENV_VAR: &str = "SUI_ID_MASTER_KEY";

pub struct ResolvedKey {
    pub key: MasterKey,
    pub origin: KeyOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOrigin {
    Env,
    KeyFile,
    GeneratedNew,
}

pub fn resolve(key_file: &Path) -> Result<ResolvedKey> {
    resolve_impl(key_adapter::default_adapter().as_ref(), key_file)
}

/// RFC 100 stage 0: every key-file operation goes through the adapter —
/// `resolve`'s own public signature is unchanged, using [`Real`] by
/// default; tests reach this directly with [`key_adapter::simulated::Simulated`]
/// or a [`key_adapter::failpoint::Failpoint`].
///
/// [`Real`]: key_adapter::Real
pub fn resolve_impl(adapter: &dyn KeyFileOps, key_file: &Path) -> Result<ResolvedKey> {
    match read_existing_impl(adapter, key_file)? {
        Some(resolved) => Ok(resolved),
        None => {
            // Generate a new key, persist with restrictive permissions, and
            // tell the caller. The startup wrapper logs a prominent notice.
            let new_key = MasterKey::generate();
            write_key_file_impl(adapter, "keyring.generate", key_file, &new_key.to_base64())?;
            Ok(ResolvedKey {
                key: new_key,
                origin: KeyOrigin::GeneratedNew,
            })
        }
    }
}

/// The environment-or-file read half of [`resolve_impl`], without the
/// generate-on-missing fallback — RFC 100 stage 1's [`resolve_checked_impl`]
/// needs to know whether a key is *present* before it is safe to decide
/// whether generating one would be safe, which `resolve_impl`'s original,
/// fused behaviour could not answer.
pub(crate) fn read_existing_impl(
    adapter: &dyn KeyFileOps,
    key_file: &Path,
) -> Result<Option<ResolvedKey>> {
    if let Ok(env_value) = std::env::var(ENV_VAR) {
        let key = MasterKey::from_base64(env_value.trim()).with_context(|| {
            format!("environment variable {ENV_VAR} did not contain a valid 32-byte base64 key")
        })?;
        // The variable is left set: nothing here removes it. sui-id spawns
        // no child process outside tests, so none inherits it. Operators
        // are expected to scope it tightly in the deployment system.
        return Ok(Some(ResolvedKey {
            key,
            origin: KeyOrigin::Env,
        }));
    }
    match adapter.read("keyring.resolve.read", key_file) {
        Ok(bytes) => {
            let s = String::from_utf8(bytes)
                .with_context(|| format!("key file {} is not valid UTF-8", key_file.display()))?;
            let key = MasterKey::from_base64(s.trim())
                .with_context(|| format!("key file {} is malformed", key_file.display()))?;
            Ok(Some(ResolvedKey {
                key,
                origin: KeyOrigin::KeyFile,
            }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e))
            .with_context(|| format!("failed to read key file {}", key_file.display())),
    }
}

/// RFC 100 stage 1: the one function every caller uses to get a master
/// key before opening the database — `startup::prepare`, every simple
/// `cli.rs` command (through [`crate::database::open_checked`]), and
/// rotation's old-key load directly. Reads the database read-only
/// first, decides from its state and whether a key is present whether
/// generating one is safe, verifies an existing key against the
/// sentinel or one sealed row when either exists, and refuses — before
/// any migration, writing nothing — when the key is wrong, or missing
/// over data that needs it. `key_adapter::resolve(` and `::resolve_impl(`
/// no longer have any caller outside this module and its own tests;
/// this function is the replacement, named differently so the "only
/// path" grep (`key_adapter::tests::nothing_touches_a_key_file_outside_the_adapter`'s
/// sibling for this stage, `tests::resolve_is_only_called_from_the_keyring_module`)
/// has one name to look for.
pub fn resolve_checked(key_file: &Path, db_path: &Path) -> Result<ResolvedKey> {
    resolve_checked_impl(key_adapter::default_adapter().as_ref(), key_file, db_path)
}

/// [`resolve_checked`]'s body, parameterized over the adapter the same
/// way [`resolve_impl`] is — a test reaches this directly with
/// [`key_adapter::simulated::Simulated`].
///
/// The `expect`s below each state an invariant [`master_key_check::decide`]'s
/// own match already guarantees (a given [`KeyCheckDecision`] variant
/// always pairs with the `Inspection`/key-presence combination its own
/// arm matched on) — a real panic here would mean `decide` and this
/// function's `match` have drifted apart, not a reachable runtime state.
#[allow(clippy::expect_used)]
pub fn resolve_checked_impl(
    adapter: &dyn KeyFileOps,
    key_file: &Path,
    db_path: &Path,
) -> Result<ResolvedKey> {
    let existing = read_existing_impl(adapter, key_file)?;
    let inspection = inspect_database(db_path)?;
    let decision = master_key_check::decide(existing.is_some(), &inspection);
    match decision {
        KeyCheckDecision::UseThenWriteSentinel => {
            Ok(existing.expect("UseThenWriteSentinel implies a key was read"))
        }
        KeyCheckDecision::GenerateThenWriteSentinel => {
            let new_key = MasterKey::generate();
            write_key_file_impl(adapter, "keyring.generate", key_file, &new_key.to_base64())?;
            Ok(ResolvedKey {
                key: new_key,
                origin: KeyOrigin::GeneratedNew,
            })
        }
        KeyCheckDecision::VerifyAgainstSentinel => {
            let resolved = existing.expect("VerifyAgainstSentinel implies a key was read");
            let Inspection::Sentinel(row) = &inspection else {
                unreachable!("VerifyAgainstSentinel only pairs with Inspection::Sentinel")
            };
            if master_key_check::sentinel_opens(&resolved.key, &row.sealed) {
                Ok(resolved)
            } else {
                Err(crate::database::DatabaseRefused::new(wrong_key_line(
                    key_file,
                    &resolved.key,
                    Some(&row.key_fingerprint),
                ))
                .into())
            }
        }
        KeyCheckDecision::VerifyOneSealedRowThenWriteSentinel => {
            let resolved =
                existing.expect("VerifyOneSealedRowThenWriteSentinel implies a key was read");
            let Inspection::SealedNoSentinel { sample, aad } = &inspection else {
                unreachable!(
                    "VerifyOneSealedRowThenWriteSentinel only pairs with Inspection::SealedNoSentinel"
                )
            };
            if sui_id_store::crypto::open(&resolved.key, sample, aad).is_ok() {
                Ok(resolved)
            } else {
                Err(crate::database::DatabaseRefused::new(wrong_key_line(
                    key_file,
                    &resolved.key,
                    None,
                ))
                .into())
            }
        }
        KeyCheckDecision::RefuseMissingKeyOverSentinel => {
            let Inspection::Sentinel(row) = &inspection else {
                unreachable!("RefuseMissingKeyOverSentinel only pairs with Inspection::Sentinel")
            };
            Err(crate::database::DatabaseRefused::new(missing_key_line(
                key_file,
                Some(&row.key_fingerprint),
            ))
            .into())
        }
        KeyCheckDecision::RefuseMissingKeyOverSealedRows => {
            Err(crate::database::DatabaseRefused::new(missing_key_line(key_file, None)).into())
        }
    }
}

/// Read the database read-only, before any migration, and say what it
/// says about whether it has ever been sealed — [`Inspection::NoFile`]
/// if the path does not exist at all (the one fact
/// [`sui_id_store::master_key_check::inspect`] cannot answer itself; it
/// needs a connection to ask).
fn inspect_database(db_path: &Path) -> Result<Inspection> {
    if !db_path.exists() {
        return Ok(Inspection::NoFile);
    }
    let conn = sui_id_store::migrations::open_preview_connection(db_path).with_context(|| {
        format!(
            "opening {} read-only to check the master key",
            db_path.display()
        )
    })?;
    master_key_check::inspect(&conn).with_context(|| {
        format!(
            "reading the master key check state of {}",
            db_path.display()
        )
    })
}

/// The refusal line for a key that does not open what it should —
/// RFC 100's own example (`implementation.md`'s dispatch): names the
/// candidate key's fingerprint always, and the sentinel's stored
/// fingerprint when one exists to name (`None` in the upgrade case,
/// where the key failed against one sealed row rather than a sentinel).
fn wrong_key_line(
    key_file: &Path,
    candidate: &MasterKey,
    stored_fingerprint: Option<&str>,
) -> String {
    let candidate_fp = sui_id_store::crypto::fingerprint(candidate);
    match stored_fingerprint {
        Some(stored_fp) => format!(
            "the master key in {} (fingerprint {candidate_fp}) does not open this database, \
             whose sentinel was written under fingerprint {stored_fp}; restore the matching key \
             file, or set {ENV_VAR} to it.",
            key_file.display(),
        ),
        None => format!(
            "the master key in {} (fingerprint {candidate_fp}) does not open this database, \
             which holds data sealed under another master key; restore the matching key file, \
             or set {ENV_VAR} to it.",
            key_file.display(),
        ),
    }
}

/// The refusal line for a missing key over data that needs one —
/// RFC 100's own example: names the sentinel's stored fingerprint when
/// one exists (`None` in the upgrade case, where there is no sentinel
/// yet to name one from).
fn missing_key_line(key_file: &Path, stored_fingerprint: Option<&str>) -> String {
    match stored_fingerprint {
        Some(stored_fp) => format!(
            "no master key file at {}, but this database holds data sealed under a master key \
             (fingerprint {stored_fp}). Generating a new key would make it unreadable. Restore \
             the key file.",
            key_file.display(),
        ),
        None => format!(
            "no master key file at {}, but this database holds data sealed under a master key \
             (no sentinel has been written yet, so its fingerprint cannot be named). Generating \
             a new key would make it unreadable. Restore the key file.",
            key_file.display(),
        ),
    }
}

/// Create `path` with owner-only permissions, atomically — `create_new`
/// so the file is never briefly world-readable between creation and a
/// later `chmod`. `pub` (not `pub(crate)`): the binary's `cli.rs` is a
/// separate crate from this library target, so the rotation command's
/// use of this to stage the new master key needs full visibility, not
/// just crate-local.
///
/// `0.91.0` stage 1, part A: returns only after the file's own bytes and
/// its directory entry are both synced to disk — before this, the file
/// was merely `write_all`'d, which a power loss can still lose (the data
/// may sit in the page cache, and the directory entry that names it may
/// not have reached disk at all). Every caller (rotation's staging write,
/// and `resolve`'s first-start generation below) gets the same guarantee.
pub fn write_key_file(path: &Path, body: &str) -> Result<()> {
    write_key_file_impl(
        key_adapter::default_adapter().as_ref(),
        "rotate.stage",
        path,
        body,
    )
}

/// RFC 100 stage 0: [`write_key_file`]'s real body, parameterized over the
/// adapter so a test can plug in [`key_adapter::simulated::Simulated`] or
/// a [`key_adapter::failpoint::Failpoint`] without changing the public
/// signature production code calls. `point_prefix` names this call site
/// for the adapter (`"rotate.stage"` for rotation's staging write,
/// `"keyring.generate"` for first-run generation) — each of the four
/// operations below gets its own point, `"{point_prefix}.<op>"`.
pub fn write_key_file_impl(
    adapter: &dyn KeyFileOps,
    point_prefix: &str,
    path: &Path,
    body: &str,
) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    adapter
        .create_new(&format!("{point_prefix}.create_new"), path)
        .with_context(|| format!("failed to create key file {}", path.display()))?;
    let mut data = body.as_bytes().to_vec();
    data.push(b'\n');
    adapter
        .write_all(&format!("{point_prefix}.write"), path, &data)
        .with_context(|| format!("failed to write key file {}", path.display()))?;
    adapter
        .sync_file(&format!("{point_prefix}.sync_file"), path)
        .with_context(|| format!("failed to sync key file {} to disk", path.display()))?;
    sync_parent_dir_impl(adapter, &format!("{point_prefix}.sync_dir"), path)
        .with_context(|| format!("failed to sync the directory holding {}", path.display()))?;
    Ok(())
}

/// Fsync the directory holding `path`, so a create or rename inside it
/// (the directory *entry*, not the file's own bytes — `File::sync_all`
/// on the file covers only the latter) survives a crash. `pub`, for the
/// same cross-target reason as [`write_key_file`]: the rotation command
/// in the binary calls this after its own renames, which happen in
/// `cli.rs`, not here.
pub fn sync_parent_dir(path: &Path) -> Result<()> {
    sync_parent_dir_impl(
        key_adapter::default_adapter().as_ref(),
        "rotate.sync_dir",
        path,
    )
}

/// RFC 100 stage 0: [`sync_parent_dir`]'s real body, parameterized over
/// the adapter. `point` names this call site — callers that sync the
/// same directory more than once per run (rotation's two renames) pass a
/// distinct point each time.
pub fn sync_parent_dir_impl(adapter: &dyn KeyFileOps, point: &str, path: &Path) -> Result<()> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    adapter
        .sync_dir(point, parent)
        .with_context(|| format!("failed to open directory {} to sync it", parent.display()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "keyring/tests.rs"]
mod tests;

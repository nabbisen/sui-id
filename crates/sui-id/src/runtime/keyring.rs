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
    if let Ok(env_value) = std::env::var(ENV_VAR) {
        let key = MasterKey::from_base64(env_value.trim()).with_context(|| {
            format!("environment variable {ENV_VAR} did not contain a valid 32-byte base64 key")
        })?;
        // The variable is left set: nothing here removes it. sui-id spawns
        // no child process outside tests, so none inherits it. Operators
        // are expected to scope it tightly in the deployment system.
        return Ok(ResolvedKey {
            key,
            origin: KeyOrigin::Env,
        });
    }
    match adapter.read("keyring.resolve.read", key_file) {
        Ok(bytes) => {
            let s = String::from_utf8(bytes)
                .with_context(|| format!("key file {} is not valid UTF-8", key_file.display()))?;
            let key = MasterKey::from_base64(s.trim())
                .with_context(|| format!("key file {} is malformed", key_file.display()))?;
            Ok(ResolvedKey {
                key,
                origin: KeyOrigin::KeyFile,
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Generate a new key, persist with restrictive permissions, and
            // tell the caller. The startup wrapper logs a prominent notice.
            let new_key = MasterKey::generate();
            write_key_file_impl(adapter, "keyring.generate", key_file, &new_key.to_base64())?;
            Ok(ResolvedKey {
                key: new_key,
                origin: KeyOrigin::GeneratedNew,
            })
        }
        Err(e) => Err(anyhow::Error::new(e))
            .with_context(|| format!("failed to read key file {}", key_file.display())),
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

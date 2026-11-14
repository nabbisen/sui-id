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
    if key_file.exists() {
        let s = std::fs::read_to_string(key_file)
            .with_context(|| format!("failed to read key file {}", key_file.display()))?;
        let key = MasterKey::from_base64(s.trim())
            .with_context(|| format!("key file {} is malformed", key_file.display()))?;
        return Ok(ResolvedKey {
            key,
            origin: KeyOrigin::KeyFile,
        });
    }
    // Generate a new key, persist with restrictive permissions, and tell the
    // caller. The startup wrapper logs a prominent notice.
    let new_key = MasterKey::generate();
    write_key_file(key_file, &new_key.to_base64())?;
    Ok(ResolvedKey {
        key: new_key,
        origin: KeyOrigin::GeneratedNew,
    })
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
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to create key file {}", path.display()))?;
    f.write_all(body.as_bytes())?;
    f.write_all(b"\n")?;
    f.sync_all()
        .with_context(|| format!("failed to sync key file {} to disk", path.display()))?;
    sync_parent_dir(path)
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
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let dir = std::fs::File::open(parent)
        .with_context(|| format!("failed to open directory {} to sync it", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync directory {}", parent.display()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "keyring/tests.rs"]
mod tests;

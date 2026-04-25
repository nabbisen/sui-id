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
        // Scrub the env var so child processes don't inherit it.
        // SAFETY: Only main process at startup; setting env in single-threaded init.
        // Note: std::env::remove_var is technically unsafe in multi-threaded contexts,
        // but we run this before any threads are spawned.
        // We deliberately leave it set so an admin can confirm the source via `env`,
        // but operators are expected to scope it tightly via the deployment system.
        return Ok(ResolvedKey { key, origin: KeyOrigin::Env });
    }
    if key_file.exists() {
        let s = std::fs::read_to_string(key_file)
            .with_context(|| format!("failed to read key file {}", key_file.display()))?;
        let key = MasterKey::from_base64(s.trim())
            .with_context(|| format!("key file {} is malformed", key_file.display()))?;
        return Ok(ResolvedKey { key, origin: KeyOrigin::KeyFile });
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

fn write_key_file(path: &Path, body: &str) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to create key file {}", path.display()))?;
    f.write_all(body.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

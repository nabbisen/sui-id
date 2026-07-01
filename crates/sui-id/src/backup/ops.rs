//! Backup, restore, and verify operations, plus internal crypto/fs helpers.
//! Crypto and filesystem helpers live here (rather than in separate files)
//! to keep the cross-file import surface minimal.

//! Backup and restore helpers.
//!
//! ## Backup file layout
//!
//! A plain (unencrypted) backup is a POSIX ustar tarball of three
//! entries:
//!
//!   * `MANIFEST.json` — provenance metadata (sui-id version, schema
//!     version, source hostname, issuer, creation timestamp).
//!     Operators read this without needing the master key, and
//!     `restore` consults it before clobbering anything.
//!   * `sui-id.sqlite` — a SQLite-consistent snapshot of the database
//!     produced via `VACUUM INTO`. Safe to take while sui-id is
//!     running.
//!   * `sui-id.key`    — a verbatim copy of the master key file.
//!
//! Plain backups have file mode 0600 — they contain the master key.
//!
//! ## Encrypted backup
//!
//! With `--encrypt` (or programmatically, [`BackupOptions::passphrase`]),
//! the tarball above is wrapped in an encrypted envelope:
//!
//! ```text
//!   magic(8)    "SUIDIDBK"
//!   version(4)  big-endian u32, currently 1
//!   salt(16)    Argon2id input
//!   nonce(24)   XChaCha20-Poly1305 nonce
//!   ciphertext  XChaCha20-Poly1305 over the inner tar
//!   tag(16)     Poly1305 authentication tag (appended by the AEAD)
//! ```
//!
//! Key derivation: Argon2id over the operator's passphrase with
//! conservative parameters (m_cost=64 MiB, t_cost=3, p_cost=1). The
//! 32-byte derived key is used directly as the AEAD key. The salt is
//! random per backup and stored in the envelope; the nonce is also
//! random per backup.
//!
//! Operators who want to ship a backup over a transport they don't
//! trust (cloud storage, email, removable media) should always use
//! the encrypted form. The plain form is fine for backups that stay
//! on the same trust boundary as the host (a local disk, the same
//! VPC), where the master key being inline is not an issue.
//!
//! ## Restore safety
//!
//! `restore` refuses to clobber an existing database or key file
//! without `--force`. It also reads the manifest first and refuses
//! a backup whose `format_version` is newer than this build knows,
//! or whose `schema_version` is newer than the latest migration this
//! build can run. Both are reversible operator failures: rebuild
//! with the right binary version.

use crate::config::Config;
use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use getrandom;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const ENTRY_MANIFEST: &str = "MANIFEST.json";
const ENTRY_DB: &str = "sui-id.sqlite";
const ENTRY_KEY: &str = "sui-id.key";

/// On-disk format version for the MANIFEST and the encrypted
/// envelope. Bumped when the layout changes in a way that older
/// restores can't read.
use super::tar::{read_tar, write_tar_entry, write_tar_terminator};
use super::types::{BackupOptions, Manifest, RestoreOptions, VerifyReport};
const FORMAT_VERSION: u32 = 1; // mirrors backup::FORMAT_VERSION
const ENCRYPTED_MAGIC: &[u8; 8] = b"SUIDIDBK";
const ARGON2_M_COST_KIB: u32 = 64 * 1024;
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 1;

pub fn run_backup(cfg: &Config, dest: &Path, opts: &BackupOptions) -> Result<()> {
    if dest.exists() {
        bail!("refusing to overwrite existing file {}", dest.display());
    }

    if !cfg.storage.db_path.exists() {
        bail!(
            "configured database does not exist at {}",
            cfg.storage.db_path.display()
        );
    }
    if !cfg.storage.key_file.exists() {
        bail!(
            "configured key file does not exist at {}",
            cfg.storage.key_file.display()
        );
    }

    // Step 1: snapshot via VACUUM INTO.
    let snapshot_dir = tempfile_dir()?;
    let snapshot_path = snapshot_dir.join(ENTRY_DB);
    {
        let conn = rusqlite::Connection::open(&cfg.storage.db_path)
            .context("opening source database for snapshot")?;
        let target = snapshot_path
            .to_str()
            .context("snapshot path must be valid UTF-8")?;
        let quoted = target.replace('\'', "''");
        conn.execute_batch(&format!("VACUUM INTO '{quoted}'"))
            .context("VACUUM INTO failed")?;
    }
    let db_bytes = std::fs::read(&snapshot_path).context("reading database snapshot")?;
    let key_bytes = std::fs::read(&cfg.storage.key_file).context("reading master key file")?;

    // Step 2: read schema_version from the snapshot for the manifest.
    let schema_version: i64 = {
        let conn = rusqlite::Connection::open(&snapshot_path)
            .context("reopening snapshot to read schema_version")?;
        conn.query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| {
                let s: String = r.get(0)?;
                Ok(s.parse::<i64>().unwrap_or(0))
            },
        )
        .unwrap_or(0)
    };

    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        sui_id_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
        created_at: chrono::Utc::now().to_rfc3339(),
        hostname: hostname_or_unknown(),
        issuer: cfg.server.issuer.clone(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).context("serialising MANIFEST")?;

    // Step 3: build the inner tar in memory so we can optionally
    // encrypt it as one blob.
    let mut tar_buf = Vec::with_capacity(db_bytes.len() + key_bytes.len() + 4096);
    write_tar_entry(&mut tar_buf, ENTRY_MANIFEST, &manifest_bytes)?;
    write_tar_entry(&mut tar_buf, ENTRY_DB, &db_bytes)?;
    write_tar_entry(&mut tar_buf, ENTRY_KEY, &key_bytes)?;
    write_tar_terminator(&mut tar_buf)?;

    // Step 4: write to dest, encrypted or plain.
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let mut out = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(dest)
        .with_context(|| format!("creating backup file {}", dest.display()))?;

    if let Some(passphrase) = opts.passphrase.as_deref() {
        let envelope = encrypt_envelope(passphrase, &tar_buf)?;
        out.write_all(&envelope)?;
    } else {
        out.write_all(&tar_buf)?;
    }
    out.sync_all().ok();

    // Best-effort cleanup.
    let _ = std::fs::remove_file(&snapshot_path);
    let _ = std::fs::remove_dir(&snapshot_dir);

    Ok(())
}

/// Restore a backup tarball into the configured storage paths.
pub fn run_restore(cfg: &Config, src: &Path, opts: &RestoreOptions) -> Result<()> {
    if !src.exists() {
        bail!("backup file {} does not exist", src.display());
    }
    let bytes = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let (_, manifest, db_bytes, key_bytes) = parse_backup(&bytes, opts.passphrase.as_deref())?;

    // Manifest checks.
    check_manifest_compatibility(&manifest)?;

    if !opts.force {
        if cfg.storage.db_path.exists() {
            bail!(
                "refusing to overwrite existing database at {} (pass --force to override)",
                cfg.storage.db_path.display()
            );
        }
        if cfg.storage.key_file.exists() {
            bail!(
                "refusing to overwrite existing key file at {} (pass --force to override)",
                cfg.storage.key_file.display()
            );
        }
    }

    if let Some(parent) = cfg.storage.db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    if let Some(parent) = cfg.storage.key_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    write_atomic(&cfg.storage.db_path, &db_bytes, 0o600)?;
    write_atomic(&cfg.storage.key_file, &key_bytes, 0o600)?;
    Ok(())
}

/// Read a backup file and report what's inside, without writing
/// anything. Useful before a real restore — see `sui-id verify-backup`.
pub fn run_verify(src: &Path, passphrase: Option<&str>) -> Result<VerifyReport> {
    if !src.exists() {
        bail!("backup file {} does not exist", src.display());
    }
    let bytes = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let encrypted = is_encrypted(&bytes);
    let (tar_bytes_len, manifest, db_bytes, key_bytes) = parse_backup(&bytes, passphrase)?;
    // Run a SQLite integrity check on the inner database. This catches
    // a corrupted snapshot before the operator commits to the restore.
    {
        let dir = tempfile_dir()?;
        let temp_db = dir.join("verify.sqlite");
        std::fs::write(&temp_db, &db_bytes).context("staging snapshot for integrity check")?;
        let conn =
            rusqlite::Connection::open(&temp_db).context("opening snapshot for integrity check")?;
        let result: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .context("running integrity_check")?;
        let _ = std::fs::remove_file(&temp_db);
        let _ = std::fs::remove_dir(&dir);
        if result != "ok" {
            bail!("SQLite integrity_check failed: {result}");
        }
    }
    Ok(VerifyReport {
        manifest,
        encrypted,
        tar_bytes: tar_bytes_len,
        db_bytes: db_bytes.len(),
        key_present: !key_bytes.is_empty(),
    })
}

// ---------- internals ---------------------------------------------

fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[..8] == ENCRYPTED_MAGIC
}

/// Common reader for both plain and encrypted backups.
/// Returns: (tar_byte_len, manifest, db_bytes, key_bytes).
fn parse_backup(
    bytes: &[u8],
    passphrase: Option<&str>,
) -> Result<(usize, Manifest, Vec<u8>, Vec<u8>)> {
    let tar_bytes: Vec<u8> = if is_encrypted(bytes) {
        let pass = passphrase
            .context("this backup is encrypted; supply --decrypt and provide the passphrase")?;
        decrypt_envelope(pass, bytes)?
    } else {
        if passphrase.is_some() {
            // Operator passed --decrypt but the file is plain. Almost
            // certainly a misuse — refuse rather than silently ignore.
            bail!("backup file is not encrypted, but a passphrase was provided");
        }
        bytes.to_vec()
    };
    let entries = read_tar(&tar_bytes)?;
    let manifest_bytes = entries
        .iter()
        .find(|(name, _)| name == ENTRY_MANIFEST)
        .map(|(_, b)| b.as_slice());
    let manifest = match manifest_bytes {
        Some(b) => serde_json::from_slice::<Manifest>(b).context("parsing MANIFEST.json")?,
        None => {
            // Backups created before v0.13.0 don't have a manifest.
            // Fabricate a permissive one so they still restore. The
            // schema_version is unknown; we mark it 0 so the
            // compatibility check stays out of the way.
            Manifest {
                format_version: 0,
                sui_id_version: "<pre-0.13>".into(),
                schema_version: 0,
                created_at: "".into(),
                hostname: "".into(),
                issuer: "".into(),
            }
        }
    };
    let db_bytes = entries
        .iter()
        .find(|(name, _)| name == ENTRY_DB)
        .map(|(_, b)| b.clone())
        .with_context(|| format!("backup is missing {ENTRY_DB} entry"))?;
    let key_bytes = entries
        .iter()
        .find(|(name, _)| name == ENTRY_KEY)
        .map(|(_, b)| b.clone())
        .with_context(|| format!("backup is missing {ENTRY_KEY} entry"))?;
    Ok((tar_bytes.len(), manifest, db_bytes, key_bytes))
}

fn check_manifest_compatibility(m: &Manifest) -> Result<()> {
    // Future format versions: refuse — we wouldn't know how to read
    // the inner data even if everything else looked fine.
    if m.format_version > FORMAT_VERSION {
        bail!(
            "backup format_version {} is newer than this build supports ({}). \
             Restore on a newer sui-id or downgrade the backup.",
            m.format_version,
            FORMAT_VERSION
        );
    }
    // Future schema versions: refuse — migrations only go forward, so
    // a backup from a newer build cannot be opened by this one.
    let our_max_schema = sui_id_store::migrations::MAX_SCHEMA_VERSION as i64;
    if m.schema_version > our_max_schema {
        bail!(
            "backup schema_version {} is newer than this build supports (max {}). \
             Use a newer sui-id binary to restore this backup.",
            m.schema_version,
            our_max_schema
        );
    }
    Ok(())
}

fn encrypt_envelope(passphrase: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).expect("system RNG unavailable");
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).expect("system RNG unavailable");
    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow::anyhow!("encryption failed"))?;

    let mut out = Vec::with_capacity(8 + 4 + 16 + 24 + ciphertext.len());
    out.extend_from_slice(ENCRYPTED_MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn decrypt_envelope(passphrase: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    const HEADER_LEN: usize = 8 + 4 + 16 + 24;
    if bytes.len() < HEADER_LEN + 16 {
        bail!("encrypted backup truncated (header missing)");
    }
    let (magic, rest) = bytes.split_at(8);
    if magic != ENCRYPTED_MAGIC {
        bail!("encrypted backup magic mismatch");
    }
    let (version_bytes, rest) = rest.split_at(4);
    let version = u32::from_be_bytes(version_bytes.try_into().unwrap());
    if version != FORMAT_VERSION {
        bail!(
            "encrypted backup envelope version {} is not supported (this build supports {})",
            version,
            FORMAT_VERSION
        );
    }
    let (salt, rest) = rest.split_at(16);
    let (nonce, ciphertext) = rest.split_at(24);
    let key = derive_key(passphrase, salt)?;
    let cipher = XChaCha20Poly1305::new(key.as_slice().into());
    let plaintext = cipher
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|_| {
            anyhow::anyhow!(
                "could not decrypt backup — wrong passphrase, or the file has been tampered with"
            )
        })?;
    Ok(plaintext)
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    use argon2::{Algorithm, Argon2, Params, Version};
    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| anyhow::anyhow!("argon2 derive: {e}"))?;
    Ok(out)
}

fn hostname_or_unknown() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            // Fallback: read /etc/hostname. Best-effort — used for the
            // operator's eye, never for a security decision.
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "<unknown>".into())
}

fn write_atomic(target: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let tmp = target.with_extension("restoring");
    if tmp.exists() {
        std::fs::remove_file(&tmp).ok();
    }
    {
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(mode)
            .open(&tmp)
            .with_context(|| format!("creating temp file {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, target)
        .with_context(|| format!("renaming temp file into {}", target.display()))?;
    Ok(())
}

fn tempfile_dir() -> Result<PathBuf> {
    // Per-call directory: process id + nanosecond timestamp + a tiny
    // bit of randomness. Concurrent callers (cron + manual run, or
    // parallel test threads) must not share a directory because the
    // snapshot filename inside is fixed; collision = VACUUM INTO
    // refuses to write into an existing file.
    use std::time::{SystemTime, UNIX_EPOCH};

    let base = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut rand_byte = [0u8; 4];
    getrandom::fill(&mut rand_byte).expect("system RNG unavailable");
    let suffix = u32::from_le_bytes(rand_byte);
    let unique = format!(
        "sui-id-backup-{}-{}-{:08x}",
        std::process::id(),
        nanos,
        suffix
    );
    let dir = base.join(unique);
    std::fs::create_dir_all(&dir).context("creating temp dir for snapshot")?;
    Ok(dir)
}

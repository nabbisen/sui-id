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
use anyhow::{bail, Context, Result};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
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
pub const FORMAT_VERSION: u32 = 1;

/// Magic bytes at the head of an encrypted envelope. Lets `restore`
/// distinguish encrypted from plain at a single read of the first 8
/// bytes, without the operator having to remember which kind they
/// supplied.
const ENCRYPTED_MAGIC: &[u8; 8] = b"SUIDIDBK";

/// Argon2id parameters for passphrase → AEAD key derivation.
/// 64 MiB / 3 iterations / 1 thread is well above the 19 MiB minimum
/// recommended by OWASP for password storage and well below anything
/// that would push backup creation past a couple of seconds on
/// reasonable hardware.
const ARGON2_M_COST_KIB: u32 = 64 * 1024;
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 1;

/// Provenance metadata written into every backup. `restore` consults
/// `format_version` and `schema_version` before doing anything
/// destructive; everything else is for the operator to read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub sui_id_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub hostname: String,
    pub issuer: String,
}

#[derive(Debug, Default, Clone)]
pub struct BackupOptions {
    /// When `Some`, the backup is encrypted under a key derived from
    /// the passphrase. When `None`, a plain tarball is produced.
    pub passphrase: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct RestoreOptions {
    pub force: bool,
    /// Required when the backup file is encrypted. Optional otherwise
    /// (a plain tarball is accepted with `passphrase = None`).
    pub passphrase: Option<String>,
}

/// Result of `verify-backup` — purely informational.
#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub manifest: Manifest,
    pub encrypted: bool,
    /// Total bytes of the tar (post-decrypt if encrypted).
    pub tar_bytes: usize,
    /// Bytes of the inner SQLite snapshot.
    pub db_bytes: usize,
    /// Whether the master key entry is present.
    pub key_present: bool,
}

/// Produce a backup tarball at `dest`.
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
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .context("serialising MANIFEST")?;

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
    let (tar_bytes_len, manifest, db_bytes, key_bytes) =
        parse_backup(&bytes, passphrase)?;
    // Run a SQLite integrity check on the inner database. This catches
    // a corrupted snapshot before the operator commits to the restore.
    {
        let dir = tempfile_dir()?;
        let temp_db = dir.join("verify.sqlite");
        std::fs::write(&temp_db, &db_bytes).context("staging snapshot for integrity check")?;
        let conn = rusqlite::Connection::open(&temp_db)
            .context("opening snapshot for integrity check")?;
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
        let pass = passphrase.context(
            "this backup is encrypted; supply --decrypt and provide the passphrase",
        )?;
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
    OsRng.fill_bytes(&mut salt);
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
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
    OsRng.fill_bytes(&mut rand_byte);
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

// ---------- minimal POSIX ustar tar writer / reader ----------------------
// The `tar` crate is a perfectly good dependency, but for two files we can
// stay zero-dep and keep the audit surface small. Format reference:
// https://www.gnu.org/software/tar/manual/html_node/Standard.html

const BLOCK: usize = 512;

fn write_tar_entry<W: Write>(out: &mut W, name: &str, bytes: &[u8]) -> Result<()> {
    if name.len() >= 100 {
        bail!("tar entry name too long: {name}");
    }
    let mut header = [0u8; BLOCK];
    // name (offset 0, 100 bytes)
    header[..name.len()].copy_from_slice(name.as_bytes());
    // mode (100, 8 bytes, octal ASCII, NUL-terminated). 0600.
    write_octal(&mut header[100..108], 0o600);
    // uid, gid (108, 8 bytes each). 0.
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    // size (124, 12 bytes octal)
    write_octal(&mut header[124..136], bytes.len() as u64);
    // mtime (136, 12 bytes octal) — 0 is acceptable for an archive.
    write_octal(&mut header[136..148], 0);
    // chksum (148, 8 bytes) — fill with spaces for the checksum
    // computation, then overwrite with the result.
    for b in &mut header[148..156] {
        *b = b' ';
    }
    // typeflag (156, 1 byte) — '0' = regular file.
    header[156] = b'0';
    // linkname (157, 100 bytes) zero-filled.
    // magic (257, 6) "ustar\0"
    header[257..263].copy_from_slice(b"ustar\0");
    // version (263, 2)
    header[263..265].copy_from_slice(b"00");
    // uname/gname (265, 32 each) — leave empty.
    // devmajor/devminor (329, 8 each) — 0.
    write_octal(&mut header[329..337], 0);
    write_octal(&mut header[337..345], 0);

    let chksum: u32 = header.iter().map(|&b| b as u32).sum();
    // 6 octal digits, NUL, space.
    let s = format!("{chksum:06o}\0 ");
    let bytes_chk = s.as_bytes();
    header[148..148 + bytes_chk.len()].copy_from_slice(bytes_chk);

    out.write_all(&header)?;
    out.write_all(bytes)?;
    let pad = (BLOCK - (bytes.len() % BLOCK)) % BLOCK;
    if pad > 0 {
        out.write_all(&vec![0u8; pad])?;
    }
    Ok(())
}

fn write_tar_terminator<W: Write>(out: &mut W) -> Result<()> {
    out.write_all(&[0u8; BLOCK * 2])?;
    Ok(())
}

fn write_octal(buf: &mut [u8], mut value: u64) {
    let n = buf.len();
    // We write `n-1` octal digits, NUL-terminated.
    for i in (0..n - 1).rev() {
        buf[i] = b'0' + (value & 0o7) as u8;
        value >>= 3;
    }
    buf[n - 1] = 0;
}

fn read_tar(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    let mut idx = 0;
    while idx + BLOCK <= bytes.len() {
        let header = &bytes[idx..idx + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = std::str::from_utf8(&header[..name_end])
            .context("tar entry name is not UTF-8")?
            .to_owned();
        let size = read_octal(&header[124..136])?;
        idx += BLOCK;
        if idx + (size as usize) > bytes.len() {
            bail!("truncated tar entry for {name}");
        }
        let body = bytes[idx..idx + size as usize].to_vec();
        out.push((name, body));
        // Advance past the rounded-up data area.
        let padded = ((size as usize) + BLOCK - 1) / BLOCK * BLOCK;
        idx += padded;
    }
    if out.is_empty() {
        bail!("tar archive contains no entries");
    }
    Ok(out)
}

fn read_octal(buf: &[u8]) -> Result<u64> {
    let mut v = 0u64;
    for &b in buf {
        if b == 0 || b == b' ' {
            break;
        }
        if !(b'0'..=b'7').contains(&b) {
            bail!("invalid octal digit in tar header");
        }
        v = v * 8 + (b - b'0') as u64;
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use tempfile::TempDir;

    fn fake_files(dir: &Path) -> (PathBuf, PathBuf) {
        let db = dir.join("sui-id.sqlite");
        let key = dir.join("sui-id.key");
        // For the round-trip test we don't need a real SQLite file; the
        // tar pipe doesn't care. The end-to-end backup() function does
        // need a real SQLite file, exercised separately.
        std::fs::write(&db, b"sqlite-fake-bytes").unwrap();
        std::fs::write(&key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        (db, key)
    }

    #[test]
    fn tar_round_trip_two_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let dest = tmp.path().join("out.tar");
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&dest)
                .unwrap();
            write_tar_entry(&mut f, "a", b"hello").unwrap();
            write_tar_entry(&mut f, "b", b"world!!!").unwrap();
            write_tar_terminator(&mut f).unwrap();
        }
        let mut bytes = Vec::new();
        File::open(&dest).unwrap().read_to_end(&mut bytes).unwrap();
        let entries = read_tar(&bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "a");
        assert_eq!(entries[0].1, b"hello");
        assert_eq!(entries[1].0, "b");
        assert_eq!(entries[1].1, b"world!!!");
    }

    #[test]
    fn restore_refuses_to_overwrite_without_force() {
        let tmp = TempDir::new().expect("tempdir");
        let (db, key) = fake_files(tmp.path());
        let cfg = Config {
            server: crate::config::ServerConfig {
                listen_addr: "127.0.0.1:0".into(),
                issuer: "https://x".into(),
                cookie_secure: false,
                trusted_proxies: Vec::new(),
            },
            storage: crate::config::StorageConfig {
                db_path: db.clone(),
                key_file: key.clone(),
            },
            tokens: crate::config::TokensConfig::default(),
            log: crate::config::LogConfig::default(),
            security: crate::config::SecurityConfig::default(),
        };
        let backup_path = tmp.path().join("backup.tar");
        // Build a backup tar by hand — bypass run_backup since fake_files
        // didn't create a real SQLite file.
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&backup_path)
                .unwrap();
            write_tar_entry(&mut f, ENTRY_DB, b"db-bytes").unwrap();
            write_tar_entry(&mut f, ENTRY_KEY, b"key-bytes").unwrap();
            write_tar_terminator(&mut f).unwrap();
        }
        // db & key already exist, so restore must refuse.
        let r = run_restore(&cfg, &backup_path, &RestoreOptions { force: false, passphrase: None });
        assert!(r.is_err(), "expected refusal to overwrite without --force");
        // With --force, it succeeds.
        run_restore(&cfg, &backup_path, &RestoreOptions { force: true, passphrase: None }).expect("force restore");
        assert_eq!(std::fs::read(&db).unwrap(), b"db-bytes");
        assert_eq!(std::fs::read(&key).unwrap(), b"key-bytes");
    }

    #[test]
    fn restore_creates_files_when_destinations_dont_exist() {
        let tmp = TempDir::new().expect("tempdir");
        let cfg = Config {
            server: crate::config::ServerConfig {
                listen_addr: "127.0.0.1:0".into(),
                issuer: "https://x".into(),
                cookie_secure: false,
                trusted_proxies: Vec::new(),
            },
            storage: crate::config::StorageConfig {
                db_path: tmp.path().join("subdir").join("sui-id.sqlite"),
                key_file: tmp.path().join("subdir").join("sui-id.key"),
            },
            tokens: crate::config::TokensConfig::default(),
            log: crate::config::LogConfig::default(),
            security: crate::config::SecurityConfig::default(),
        };
        let backup_path = tmp.path().join("backup.tar");
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&backup_path)
                .unwrap();
            write_tar_entry(&mut f, ENTRY_DB, b"db-bytes").unwrap();
            write_tar_entry(&mut f, ENTRY_KEY, b"key-bytes").unwrap();
            write_tar_terminator(&mut f).unwrap();
        }
        run_restore(&cfg, &backup_path, &RestoreOptions { force: false, passphrase: None }).expect("restore");
        assert!(cfg.storage.db_path.exists());
        assert!(cfg.storage.key_file.exists());
    }

    #[test]
    fn run_backup_round_trip_via_real_sqlite() {
        let tmp = TempDir::new().expect("tempdir");
        let db = tmp.path().join("source.sqlite");
        let key = tmp.path().join("source.key");
        // Real SQLite file.
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE t (k TEXT PRIMARY KEY); INSERT INTO t VALUES ('hello');",
            )
            .unwrap();
        }
        std::fs::write(&key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        let cfg = Config {
            server: crate::config::ServerConfig {
                listen_addr: "127.0.0.1:0".into(),
                issuer: "https://x".into(),
                cookie_secure: false,
                trusted_proxies: Vec::new(),
            },
            storage: crate::config::StorageConfig {
                db_path: db.clone(),
                key_file: key.clone(),
            },
            tokens: crate::config::TokensConfig::default(),
            log: crate::config::LogConfig::default(),
            security: crate::config::SecurityConfig::default(),
        };
        let dest = tmp.path().join("backup.tar");
        run_backup(&cfg, &dest, &BackupOptions::default()).expect("backup");
        assert!(dest.exists());
        // Verify mode 0600.
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // Restore into a fresh location and check the SQLite file is queryable.
        let cfg2 = Config {
            server: cfg.server.clone(),
            storage: crate::config::StorageConfig {
                db_path: tmp.path().join("restored.sqlite"),
                key_file: tmp.path().join("restored.key"),
            },
            tokens: cfg.tokens.clone(),
            log: cfg.log.clone(),
            security: cfg.security.clone(),
        };
        run_restore(&cfg2, &dest, &RestoreOptions { force: false, passphrase: None }).expect("restore");
        let conn = rusqlite::Connection::open(&cfg2.storage.db_path).unwrap();
        let v: String = conn
            .query_row("SELECT k FROM t LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
        // Key file restored byte-for-byte.
        let restored_key = std::fs::read(&cfg2.storage.key_file).unwrap();
        assert_eq!(restored_key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
    }

    // ---------- v0.13.0 additions: encryption, manifest, verify ----------

    fn make_real_sqlite_db(dir: &Path) -> (PathBuf, PathBuf) {
        let db = dir.join("sui-id.sqlite");
        let key = dir.join("sui-id.key");
        // Real SQLite with a sui_meta row so the manifest can read
        // schema_version. Mimics the post-migration state.
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE sui_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL); \
             INSERT INTO sui_meta(key, value) VALUES('schema_version', '5'); \
             CREATE TABLE t(k TEXT); INSERT INTO t VALUES('hello');",
        )
        .unwrap();
        std::fs::write(&key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        (db, key)
    }

    fn fake_cfg(_dir: &Path, db: PathBuf, key: PathBuf) -> Config {
        Config {
            server: crate::config::ServerConfig {
                listen_addr: "127.0.0.1:0".into(),
                issuer: "https://idp.test".into(),
                cookie_secure: false,
                trusted_proxies: Vec::new(),
            },
            storage: crate::config::StorageConfig { db_path: db, key_file: key },
            tokens: crate::config::TokensConfig::default(),
            log: crate::config::LogConfig::default(),
            security: crate::config::SecurityConfig::default(),
        }
    }

    #[test]
    fn manifest_present_in_plain_backup() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar");
        run_backup(&cfg, &dest, &BackupOptions::default()).unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let entries = read_tar(&bytes).unwrap();
        let m_bytes = entries
            .iter()
            .find(|(n, _)| n == ENTRY_MANIFEST)
            .map(|(_, b)| b)
            .expect("MANIFEST.json present");
        let m: Manifest = serde_json::from_slice(m_bytes).unwrap();
        assert_eq!(m.format_version, FORMAT_VERSION);
        assert_eq!(m.schema_version, 5);
        assert_eq!(m.issuer, "https://idp.test");
        assert!(!m.created_at.is_empty());
    }

    #[test]
    fn encrypted_backup_round_trips_with_correct_passphrase() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        run_backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("hunter2-correct-horse".into()),
            },
        )
        .unwrap();

        // Restore into a fresh location.
        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        run_restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: Some("hunter2-correct-horse".into()),
            },
        )
        .unwrap();
        let conn = rusqlite::Connection::open(&cfg2.storage.db_path).unwrap();
        let v: String = conn.query_row("SELECT k FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "hello");
    }

    #[test]
    fn encrypted_backup_rejects_wrong_passphrase() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        run_backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("right-pass".into()),
            },
        )
        .unwrap();

        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = run_restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: Some("wrong-pass".into()),
            },
        );
        assert!(r.is_err());
        // Failure should not have written the destination files.
        assert!(!cfg2.storage.db_path.exists());
        assert!(!cfg2.storage.key_file.exists());
    }

    #[test]
    fn restore_of_encrypted_without_passphrase_errors() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        run_backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("p".into()),
            },
        )
        .unwrap();

        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = run_restore(
            &cfg2,
            &dest,
            &RestoreOptions { force: false, passphrase: None },
        );
        let msg = format!("{}", r.unwrap_err().chain().next().unwrap());
        assert!(
            msg.contains("encrypted"),
            "error should mention encryption; got: {msg}"
        );
    }

    #[test]
    fn restore_of_plain_with_passphrase_errors() {
        // A plain tarball + --decrypt is almost certainly an
        // operator misuse. Refuse rather than silently ignore.
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar");
        run_backup(&cfg, &dest, &BackupOptions::default()).unwrap();

        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = run_restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: Some("anything".into()),
            },
        );
        assert!(r.is_err());
    }

    #[test]
    fn verify_reports_manifest_and_runs_integrity_check() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar");
        run_backup(&cfg, &dest, &BackupOptions::default()).unwrap();
        let report = run_verify(&dest, None).expect("verify");
        assert!(!report.encrypted);
        assert_eq!(report.manifest.schema_version, 5);
        assert_eq!(report.manifest.format_version, FORMAT_VERSION);
        assert!(report.key_present);
        assert!(report.db_bytes > 0);
    }

    #[test]
    fn verify_works_on_encrypted_backup_with_passphrase() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        run_backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("p".into()),
            },
        )
        .unwrap();
        let report = run_verify(&dest, Some("p")).expect("verify");
        assert!(report.encrypted);
        assert_eq!(report.manifest.schema_version, 5);
    }

    #[test]
    fn restore_refuses_backup_with_too_new_schema_version() {
        // Hand-craft a manifest with a future schema_version.
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.tar");
        let manifest = Manifest {
            format_version: FORMAT_VERSION,
            sui_id_version: "future".into(),
            schema_version: 9999,
            created_at: "2099-01-01T00:00:00Z".into(),
            hostname: "x".into(),
            issuer: "x".into(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();

        // A real SQLite file so restore doesn't trip on integrity.
        let inner_db = tmp.path().join("inner.sqlite");
        let conn = rusqlite::Connection::open(&inner_db).unwrap();
        conn.execute_batch("CREATE TABLE t(k TEXT)").unwrap();
        let db_bytes = std::fs::read(&inner_db).unwrap();

        let mut tar_buf = Vec::new();
        write_tar_entry(&mut tar_buf, ENTRY_MANIFEST, &manifest_bytes).unwrap();
        write_tar_entry(&mut tar_buf, ENTRY_DB, &db_bytes).unwrap();
        write_tar_entry(&mut tar_buf, ENTRY_KEY, b"key").unwrap();
        write_tar_terminator(&mut tar_buf).unwrap();
        std::fs::write(&dest, &tar_buf).unwrap();

        let cfg = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = run_restore(
            &cfg,
            &dest,
            &RestoreOptions { force: false, passphrase: None },
        );
        let msg = format!("{}", r.unwrap_err().chain().next().unwrap());
        assert!(msg.contains("schema_version"), "got: {msg}");
    }
}

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

use super::types::BackupError;
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
use super::FORMAT_VERSION;
use super::tar::{read_tar, write_tar_entry, write_tar_terminator};
use super::types::{BackupOptions, Manifest, RestoreOptions, VerifyReport};

type Result<T> = std::result::Result<T, BackupError>;
const ENCRYPTED_MAGIC: &[u8; 8] = b"SUIDIDBK";
const ARGON2_M_COST_KIB: u32 = 64 * 1024;
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 1;

/// Back up the database at `db_path` and the master key at `key_file` into
/// `dest`. `issuer` is recorded in the manifest for the operator.
pub fn run_backup(
    db_path: &Path,
    key_file: &Path,
    issuer: &str,
    dest: &Path,
    opts: &BackupOptions,
) -> Result<()> {
    if dest.exists() {
        return Err(BackupError::DestinationExists(dest.to_path_buf()));
    }

    if !db_path.exists() {
        return Err(BackupError::DatabaseMissing(db_path.to_path_buf()));
    }
    if !key_file.exists() {
        return Err(BackupError::KeyFileMissing(key_file.to_path_buf()));
    }

    // Step 1: snapshot via VACUUM INTO.
    let snapshot_dir = tempfile_dir()?;
    let snapshot_path = snapshot_dir.join(ENTRY_DB);
    {
        let conn = rusqlite::Connection::open(db_path).map_err(BackupError::OpenSourceDatabase)?;
        let target = snapshot_path
            .to_str()
            .ok_or(BackupError::SnapshotPathNotUtf8)?;
        let quoted = target.replace('\'', "''");
        conn.execute_batch(&format!("VACUUM INTO '{quoted}'"))
            .map_err(BackupError::VacuumInto)?;
    }
    let db_bytes = std::fs::read(&snapshot_path).map_err(BackupError::ReadSnapshot)?;
    let key_bytes = std::fs::read(key_file).map_err(BackupError::ReadKeyFile)?;

    // Step 2: read schema_version from the snapshot for the manifest.
    let schema_version: i64 = {
        let conn =
            rusqlite::Connection::open(&snapshot_path).map_err(BackupError::ReopenSnapshot)?;
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
        issuer: issuer.to_owned(),
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(BackupError::SerializeManifest)?;

    // Step 3: build the inner tar in memory so we can optionally
    // encrypt it as one blob.
    let mut tar_buf = Vec::with_capacity(db_bytes.len() + key_bytes.len() + 4096);
    write_tar_entry(&mut tar_buf, ENTRY_MANIFEST, &manifest_bytes)?;
    write_tar_entry(&mut tar_buf, ENTRY_DB, &db_bytes)?;
    write_tar_entry(&mut tar_buf, ENTRY_KEY, &key_bytes)?;
    write_tar_terminator(&mut tar_buf)?;

    // Step 4: write to dest, encrypted or plain.
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    let mut out = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(dest)
        .map_err(|source| BackupError::CreateBackupFile {
            path: dest.to_path_buf(),
            source,
        })?;

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

/// Restore a backup tarball into `db_path` and `key_file`.
pub fn run_restore(
    db_path: &Path,
    key_file: &Path,
    src: &Path,
    opts: &RestoreOptions,
) -> Result<()> {
    if !src.exists() {
        return Err(BackupError::SourceMissing(src.to_path_buf()));
    }
    let bytes = read_source(src)?;
    let (_, manifest, db_bytes, key_bytes) = parse_backup(&bytes, opts.passphrase.as_deref())?;

    // Manifest checks.
    check_manifest_compatibility(&manifest)?;

    if !opts.force {
        if db_path.exists() {
            return Err(BackupError::DatabaseExists(db_path.to_path_buf()));
        }
        if key_file.exists() {
            return Err(BackupError::KeyFileExists(key_file.to_path_buf()));
        }
    }

    if let Some(parent) = db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    if let Some(parent) = key_file.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }

    write_atomic(db_path, &db_bytes, 0o600)?;
    write_atomic(key_file, &key_bytes, 0o600)?;
    Ok(())
}

/// Read a backup file and report what's inside, without writing
/// anything. Useful before a real restore — see `sui-id verify-backup`.
pub fn run_verify(src: &Path, passphrase: Option<&str>) -> Result<VerifyReport> {
    if !src.exists() {
        return Err(BackupError::SourceMissing(src.to_path_buf()));
    }
    let bytes = read_source(src)?;
    let encrypted = is_encrypted(&bytes);
    let (tar_bytes_len, manifest, db_bytes, key_bytes) = parse_backup(&bytes, passphrase)?;
    // Run a SQLite integrity check on the inner database. This catches
    // a corrupted snapshot before the operator commits to the restore.
    {
        let dir = tempfile_dir()?;
        let temp_db = dir.join("verify.sqlite");
        std::fs::write(&temp_db, &db_bytes).map_err(BackupError::StageSnapshot)?;
        let conn = rusqlite::Connection::open(&temp_db).map_err(BackupError::OpenStagedSnapshot)?;
        let result: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(BackupError::RunIntegrityCheck)?;
        let _ = std::fs::remove_file(&temp_db);
        let _ = std::fs::remove_dir(&dir);
        if result != "ok" {
            return Err(BackupError::IntegrityCheckFailed(result));
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

fn read_source(src: &Path) -> Result<Vec<u8>> {
    std::fs::read(src).map_err(|source| BackupError::ReadSource {
        path: src.to_path_buf(),
        source,
    })
}

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
        let pass = passphrase.ok_or(BackupError::PassphraseRequired)?;
        decrypt_envelope(pass, bytes)?
    } else {
        if passphrase.is_some() {
            // Operator passed --decrypt but the file is plain. Almost
            // certainly a misuse — refuse rather than silently ignore.
            return Err(BackupError::PassphraseForPlainBackup);
        }
        bytes.to_vec()
    };
    let entries = read_tar(&tar_bytes)?;
    let manifest_bytes = entries
        .iter()
        .find(|(name, _)| name == ENTRY_MANIFEST)
        .map(|(_, b)| b.as_slice());
    let manifest = match manifest_bytes {
        Some(b) => serde_json::from_slice::<Manifest>(b).map_err(BackupError::ParseManifest)?,
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
        .ok_or(BackupError::MissingDatabaseEntry)?;
    let key_bytes = entries
        .iter()
        .find(|(name, _)| name == ENTRY_KEY)
        .map(|(_, b)| b.clone())
        .ok_or(BackupError::MissingKeyEntry)?;
    Ok((tar_bytes.len(), manifest, db_bytes, key_bytes))
}

fn check_manifest_compatibility(m: &Manifest) -> Result<()> {
    // Future format versions: refuse — we wouldn't know how to read
    // the inner data even if everything else looked fine.
    if m.format_version > FORMAT_VERSION {
        return Err(BackupError::FormatVersionTooNew {
            found: m.format_version,
            supported: FORMAT_VERSION,
        });
    }
    // Future schema versions: refuse — migrations only go forward, so
    // a backup from a newer build cannot be opened by this one.
    let our_max_schema = crate::migrations::MAX_SCHEMA_VERSION as i64;
    if m.schema_version > our_max_schema {
        return Err(BackupError::SchemaVersionTooNew {
            found: m.schema_version,
            max: our_max_schema,
        });
    }
    Ok(())
}

fn encrypt_envelope(passphrase: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    // A broken OS CSPRNG means we cannot mint a salt/nonce safe to use for
    // encryption; continuing with non-random output would be a real
    // vulnerability, so this must panic rather than degrade.
    let mut salt = [0u8; 16];
    #[allow(clippy::expect_used)]
    getrandom::fill(&mut salt).expect("system RNG unavailable");
    let mut nonce = [0u8; 24];
    #[allow(clippy::expect_used)]
    getrandom::fill(&mut nonce).expect("system RNG unavailable");
    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let xnonce = <&XNonce>::from(&nonce);
    let ciphertext = cipher
        .encrypt(xnonce, plaintext)
        .map_err(|_| BackupError::Encrypt)?;

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
        return Err(BackupError::EnvelopeTruncated);
    }
    let (magic, rest) = bytes.split_at(8);
    if magic != ENCRYPTED_MAGIC {
        return Err(BackupError::EnvelopeMagicMismatch);
    }
    let (version_bytes, rest) = rest.split_at(4);
    // split_at(4) guarantees version_bytes.len() == 4, so this cannot fail.
    #[allow(clippy::unwrap_used)]
    let version = u32::from_be_bytes(version_bytes.try_into().unwrap());
    if version != FORMAT_VERSION {
        return Err(BackupError::EnvelopeVersionUnsupported {
            found: version,
            supported: FORMAT_VERSION,
        });
    }
    let (salt, rest) = rest.split_at(16);
    let (nonce, ciphertext) = rest.split_at(24);
    let key = derive_key(passphrase, salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let xnonce = <&XNonce>::try_from(nonce).map_err(|_| BackupError::EnvelopeNonceLength)?;
    let plaintext = cipher
        .decrypt(xnonce, ciphertext)
        .map_err(|_| BackupError::Decrypt)?;
    Ok(plaintext)
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    use argon2::{Algorithm, Argon2, Params, Version};
    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| BackupError::Argon2Params(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| BackupError::Argon2Derive(e.to_string()))?;
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
            .map_err(|source| BackupError::CreateTempFile {
                path: tmp.clone(),
                source,
            })?;
        f.write_all(bytes)?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, target).map_err(|source| BackupError::RenameTempFile {
        path: target.to_path_buf(),
        source,
    })?;
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
    // A broken OS CSPRNG here is a symptom of a broken system; this suffix
    // only needs to avoid collisions, but there is nothing safe to fall
    // back to, so this must panic rather than silently degrade.
    #[allow(clippy::expect_used)]
    getrandom::fill(&mut rand_byte).expect("system RNG unavailable");
    let suffix = u32::from_le_bytes(rand_byte);
    let unique = format!(
        "sui-id-backup-{}-{}-{:08x}",
        std::process::id(),
        nanos,
        suffix
    );
    let dir = base.join(unique);
    std::fs::create_dir_all(&dir).map_err(BackupError::CreateTempDir)?;
    Ok(dir)
}

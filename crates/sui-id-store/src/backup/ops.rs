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

    // Step 2: read schema_version from the snapshot for the manifest, with the
    // one reader every caller uses (RFC 112 D4). It used to be `unwrap_or(0)`
    // twice, so a snapshot whose version could not be read was stamped `0`,
    // passed `restore`'s check, and was re-run from 0001 on the next open. A
    // snapshot that is **newer** than this build is recorded truthfully: taking
    // a backup of a database this binary cannot open is the operator's way out.
    let schema_version: i64 = {
        let conn = rusqlite::Connection::open_with_flags(
            &snapshot_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(BackupError::ReopenSnapshot)?;
        match crate::migrations::read_stored_version(&conn) {
            Ok(v) => v.number(),
            Err(crate::migrations::SchemaError::Read(e)) => {
                return Err(BackupError::ReopenSnapshot(e));
            }
            Err(e) => return Err(BackupError::SchemaVersionUnreadable(e.to_string())),
        }
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

    // Manifest checks, then the database's own version (RFC 112 D4).
    check_manifest_compatibility(&manifest)?;
    check_snapshot_schema(&db_bytes)?;

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
    // `verify` fails where `restore` would fail on the database's own schema
    // version (RFC 112 D4). The manifest and format-version checks stay
    // restore-only until RFC 106 F2 brings the rest of them across.
    check_snapshot_schema(&db_bytes)?;
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

// ---------- RFC 142 stage 5: the automatic pre-migration backup ---

/// Free-space measurement for the pre-migration snapshot (§5). `needed` is
/// already doubled: one multiple of the database's own size (main file plus
/// `-wal`, if any) for the snapshot itself, one of headroom for the
/// migrations that follow it, since a table rebuild can briefly double a
/// table. `available` is `None` when it could not be measured on this
/// platform — never by itself a reason to refuse (§5: take the snapshot
/// anyway, and say it was not checked).
#[derive(Debug, Clone, Copy)]
pub struct FreeSpaceCheck {
    pub needed_bytes: u64,
    pub available_bytes: Option<u64>,
}

impl FreeSpaceCheck {
    /// Only a *measured* shortfall refuses. `available_bytes: None` is not
    /// short — it is unmeasurable, which `database::open` takes as "proceed,
    /// and say so."
    pub fn is_short(&self) -> bool {
        matches!(self.available_bytes, Some(avail) if avail < self.needed_bytes)
    }

    /// The size of the snapshot itself: half of [`Self::needed_bytes`],
    /// which is deliberately doubled for migration headroom. Operator lines
    /// name the two separately; a line that called `needed_bytes` the
    /// backup's size would state it at twice its real size.
    pub fn snapshot_bytes(&self) -> u64 {
        self.needed_bytes / 2
    }
}

/// Bytes to whole megabytes, **rounded down**. Operator-facing lines do not
/// use this: they go through `sui_id::database`'s own formatter, which says
/// "under 1 MB" rather than "0 MB" and rounds a requirement up.
pub fn to_mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

/// Measure free space for a pre-migration snapshot of the database at
/// `db_path`. Reads only file sizes and the filesystem's own `statvfs`;
/// opens nothing.
pub fn check_free_space(db_path: &Path) -> FreeSpaceCheck {
    let db_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let wal_size = {
        let mut wal = db_path.as_os_str().to_owned();
        wal.push("-wal");
        std::fs::metadata(wal).map(|m| m.len()).unwrap_or(0)
    };
    let needed_bytes = (db_size + wal_size).saturating_mul(2);
    FreeSpaceCheck {
        needed_bytes,
        available_bytes: available_bytes_beside(db_path),
    }
}

#[cfg(unix)]
fn available_bytes_beside(db_path: &Path) -> Option<u64> {
    let dir = db_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stat = rustix::fs::statvfs(dir).ok()?;
    Some(stat.f_bavail.saturating_mul(stat.f_frsize))
}

#[cfg(not(unix))]
fn available_bytes_beside(_db_path: &Path) -> Option<u64> {
    // No safe, already-locked `statvfs` equivalent for non-Unix platforms;
    // `check_free_space` reports this as unmeasurable, which is not a
    // refusal (§5).
    None
}

/// The pre-migration snapshot's name: derived from the database's own file
/// name, in its own directory, so an operator with several can see which
/// one to restore without opening any (RFC 142 stage 5 §4).
pub fn pre_migration_snapshot_path(db_path: &Path, from: i64, to: i32) -> PathBuf {
    pre_migration_snapshot_path_at(db_path, from, to, chrono::Utc::now())
}

/// [`pre_migration_snapshot_path`] for a given instant, so the name's one
/// format can be computed for a time other than now. The name has
/// one-second resolution: a test that predicts it must cover every second
/// its call might land in, not only the second it read.
pub fn pre_migration_snapshot_path_at(
    db_path: &Path,
    from: i64,
    to: i32,
    at: chrono::DateTime<chrono::Utc>,
) -> PathBuf {
    let file_name = db_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("database");
    let stamp = at.format("%Y%m%dT%H%M%SZ");
    db_path.with_file_name(format!(
        "{file_name}.pre-upgrade-schema-{from}-to-{to}-{stamp}.sqlite"
    ))
}

/// Take the automatic pre-migration snapshot: a plain (unencrypted)
/// `VACUUM INTO` beside the database, not `run_backup`'s encrypted path, so
/// a restore needs only `sqlite3` (D4). Measured before building (§4):
/// `VACUUM INTO` works from a read-only source connection and includes
/// committed WAL frames, so the source is never opened for writing — this
/// reuses [`crate::migrations::open_preview_connection`], which already
/// avoids leaving `-shm`/`-wal` behind and already percent-encodes the
/// path. `VACUUM INTO` also accepts a pre-created empty file and keeps its
/// mode, so the destination is created `0600` *before* any bytes are
/// written to it — never world-readable, not even for an instant — and
/// `create_new` means an existing file of the same name is never
/// overwritten: this returns an error instead, and the caller removes
/// nothing (the existing file was never touched).
///
/// On a failure *after* creation (the `VACUUM INTO` itself), the partial
/// file this call created is removed before returning.
pub fn pre_migration_snapshot(db_path: &Path, from: i64, to: i32) -> Result<PathBuf> {
    let dest = pre_migration_snapshot_path(db_path, from, to);
    create_snapshot_file(&dest)?;

    // Every failure from here on removes the file this call just created —
    // not only a `VACUUM INTO` failure. The source connection can fail too
    // (RFC 142 stage 5 §3's documented race: the database can vanish or
    // become unreadable between the free-space check and this call), and a
    // partial file left behind either way is exactly what §3 point 4
    // forbids.
    match vacuum_into(db_path, &dest) {
        Ok(()) => Ok(dest),
        Err(e) => {
            let _ = std::fs::remove_file(&dest);
            Err(e)
        }
    }
}

fn vacuum_into(db_path: &Path, dest: &Path) -> Result<()> {
    let conn = crate::migrations::open_preview_connection(db_path)
        .map_err(BackupError::OpenSourceDatabase)?;
    let target = dest.to_str().ok_or(BackupError::SnapshotPathNotUtf8)?;
    let quoted = target.replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{quoted}'"))
        .map_err(BackupError::VacuumInto)
}

/// `create_new` so an existing file of the same name is refused, not
/// overwritten (§4). Unix gets mode `0600` at creation, atomically with
/// `O_CREAT | O_EXCL` — never a window where the file exists world-
/// readable. A non-Unix build still gets `create_new`'s collision safety;
/// it just cannot also set the mode here (no safe, already-locked API for
/// it), so the file is created with the platform's own default permissions.
#[cfg(unix)]
fn create_snapshot_file(dest: &Path) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(dest)
        .map(|_| ())
        .map_err(|source| BackupError::CreateBackupFile {
            path: dest.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn create_snapshot_file(dest: &Path) -> Result<()> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dest)
        .map(|_| ())
        .map_err(|source| BackupError::CreateBackupFile {
            path: dest.to_path_buf(),
            source,
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

/// Read and judge the schema version of the database bytes inside a backup, with
/// the same reader and rule the server uses on its own database (RFC 112 D4).
/// The manifest's `schema_version` is the archive's own claim; a pre-0.13 archive
/// has none and is fabricated as `0`. This reads the database itself, so neither
/// a lying nor a missing manifest can carry a too-new or unreadable database past
/// `restore` or `verify`.
fn check_snapshot_schema(db_bytes: &[u8]) -> Result<()> {
    let dir = tempfile_dir()?;
    let temp_db = dir.join("schema-check.sqlite");
    let result = (|| {
        std::fs::write(&temp_db, db_bytes).map_err(BackupError::StageSnapshot)?;
        let conn = rusqlite::Connection::open_with_flags(
            &temp_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(BackupError::OpenStagedSnapshot)?;
        let stored = match crate::migrations::read_stored_version(&conn) {
            Ok(v) => v,
            Err(crate::migrations::SchemaError::Read(e)) => {
                return Err(BackupError::OpenStagedSnapshot(e));
            }
            Err(e) => return Err(BackupError::SchemaVersionUnreadable(e.to_string())),
        };
        match crate::migrations::check_supported(stored) {
            Ok(()) => Ok(()),
            Err(crate::migrations::SchemaError::TooNew { found, supported }) => {
                Err(BackupError::SchemaVersionTooNew {
                    found,
                    max: i64::from(supported),
                })
            }
            Err(e) => Err(BackupError::SchemaVersionUnreadable(e.to_string())),
        }
    })();
    let _ = std::fs::remove_file(&temp_db);
    let _ = std::fs::remove_dir(&dir);
    result
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

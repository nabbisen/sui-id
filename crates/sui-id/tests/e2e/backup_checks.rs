//! Backup, restore and verify — one isolated trigger per safety check.
//!
//! The observational-equivalence record for moving `backup/` into
//! `sui-id-store` (RFC 094 handoff `backup-into-store.md`, RFC 096's bar):
//! each test feeds an input that exactly one check rejects, and pins that
//! check by its own message, so a dropped check cannot hide behind another
//! "refused" outcome. The tests go through the CLI-facing
//! `sui_id::backup` surface, which the move keeps, so the same source runs
//! before and after.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use sui_id::backup::{self, BackupOptions, RestoreOptions};
use sui_id::config::{Config, LogConfig, ServerConfig, StorageConfig, TokensConfig};
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;

const PASS: &str = "correct horse battery staple";

fn cfg(db_path: PathBuf, key_file: PathBuf) -> Config {
    Config {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".into(),
            issuer: "https://idp.test".into(),
            cookie_secure: false,
            trusted_proxies: Vec::new(),
            metrics_enabled: false,
            metrics_listen_addr: String::new(),
        },
        storage: StorageConfig { db_path, key_file },
        tokens: TokensConfig::default(),
        user_sources: Vec::new(),
        federation_providers: Vec::new(),
        log: LogConfig {
            format: "fmt".into(),
            filter: "off".into(),
            access_log: false,
            file: None,
        },
        security: sui_id::config::SecurityConfig::default(),
    }
}

/// A migrated database and its key file under `dir`.
fn source(dir: &Path) -> Config {
    let db_path = dir.join("src.sqlite");
    let key_file = dir.join("src.key");
    let key = MasterKey::generate();
    std::fs::write(&key_file, key.to_base64()).expect("key");
    let _db = Database::open(&db_path, key).expect("open db");
    cfg(db_path, key_file)
}

fn empty_target(dir: &Path) -> Config {
    cfg(dir.join("dst.sqlite"), dir.join("dst.key"))
}

fn err_text<T: std::fmt::Debug>(r: anyhow::Result<T>) -> String {
    format!("{:#}", r.expect_err("expected a refusal"))
}

fn plain_backup(dir: &Path) -> PathBuf {
    let src = source(dir);
    let out = dir.join("plain.tar");
    backup::run_backup(&src, &out, &BackupOptions::default()).expect("backup");
    out
}

fn encrypted_backup(dir: &Path) -> PathBuf {
    let src = source(dir);
    let out = dir.join("enc.tar.enc");
    backup::run_backup(
        &src,
        &out,
        &BackupOptions {
            passphrase: Some(PASS.into()),
        },
    )
    .expect("backup");
    out
}

// ── a minimal ustar writer, for crafted archives ─────────────────────

fn octal(buf: &mut [u8], mut v: u64) {
    let n = buf.len();
    for i in (0..n - 1).rev() {
        buf[i] = b'0' + (v & 7) as u8;
        v >>= 3;
    }
    buf[n - 1] = 0;
}

fn tar_entry(out: &mut Vec<u8>, name: &[u8], body: &[u8]) {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name);
    octal(&mut h[100..108], 0o600);
    octal(&mut h[124..136], body.len() as u64);
    for b in &mut h[148..156] {
        *b = b' ';
    }
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    let sum: u32 = h.iter().map(|&b| b as u32).sum();
    let s = format!("{sum:06o}\0 ");
    h[148..148 + s.len()].copy_from_slice(s.as_bytes());
    out.extend_from_slice(&h);
    out.extend_from_slice(body);
    out.resize(out.len().div_ceil(512) * 512, 0);
}

fn manifest(format_version: u32, schema_version: i64) -> Vec<u8> {
    format!(
        r#"{{"format_version":{format_version},"sui_id_version":"t","schema_version":{schema_version},"created_at":"","hostname":"","issuer":""}}"#
    )
    .into_bytes()
}

/// Write an archive of the given entries, terminated.
fn archive(dir: &Path, name: &str, entries: &[(&[u8], &[u8])]) -> PathBuf {
    let mut buf = Vec::new();
    for (n, b) in entries {
        tar_entry(&mut buf, n, b);
    }
    buf.extend_from_slice(&[0u8; 1024]);
    let p = dir.join(name);
    std::fs::write(&p, buf).expect("write archive");
    p
}

fn restore_err(dir: &Path, archive: &Path, opts: RestoreOptions) -> String {
    err_text(backup::run_restore(&empty_target(dir), archive, &opts))
}

fn no_force() -> RestoreOptions {
    RestoreOptions {
        force: false,
        passphrase: None,
    }
}

// ── backup ───────────────────────────────────────────────────────────

#[test]
fn k01_backup_refuses_existing_destination() {
    let t = tempfile::tempdir().unwrap();
    let src = source(t.path());
    let out = t.path().join("exists.tar");
    std::fs::write(&out, b"x").unwrap();
    let e = err_text(backup::run_backup(&src, &out, &BackupOptions::default()));
    assert!(e.contains("refusing to overwrite existing file"), "{e}");
}

#[test]
fn k02_backup_create_new_refuses_dangling_symlink() {
    // `exists()` follows the link and says no; only `create_new` refuses.
    let t = tempfile::tempdir().unwrap();
    let src = source(t.path());
    let out = t.path().join("link.tar");
    std::os::unix::fs::symlink(t.path().join("nowhere"), &out).unwrap();
    let e = err_text(backup::run_backup(&src, &out, &BackupOptions::default()));
    assert!(e.contains("creating backup file"), "{e}");
    assert!(
        !t.path().join("nowhere").exists(),
        "wrote through the symlink"
    );
}

#[test]
fn k03_backup_refuses_missing_database() {
    let t = tempfile::tempdir().unwrap();
    let mut c = source(t.path());
    c.storage.db_path = t.path().join("absent.sqlite");
    let e = err_text(backup::run_backup(
        &c,
        &t.path().join("o.tar"),
        &BackupOptions::default(),
    ));
    assert!(e.contains("configured database does not exist"), "{e}");
}

#[test]
fn k04_backup_refuses_missing_key_file() {
    let t = tempfile::tempdir().unwrap();
    let mut c = source(t.path());
    c.storage.key_file = t.path().join("absent.key");
    let e = err_text(backup::run_backup(
        &c,
        &t.path().join("o.tar"),
        &BackupOptions::default(),
    ));
    assert!(e.contains("configured key file does not exist"), "{e}");
}

/// Runs the real binary, so the temp directory can be chosen per process.
fn backup_with_tmpdir(dir: &Path, tmpdir: &std::ffi::OsStr) -> std::process::Output {
    let src = source(dir);
    let toml = format!(
        "[server]\nlisten_addr = \"127.0.0.1:0\"\nissuer = \"https://idp.test\"\n\
         [storage]\ndb_path = \"{}\"\nkey_file = \"{}\"\n",
        src.storage.db_path.display(),
        src.storage.key_file.display()
    );
    let cfg_path = dir.join("cfg.toml");
    std::fs::write(&cfg_path, toml).unwrap();
    Command::new(env!("CARGO_BIN_EXE_sui-id"))
        .arg("backup")
        .arg("--config")
        .arg(&cfg_path)
        .arg("--to")
        .arg(dir.join("sub.tar"))
        .env("TMPDIR", tmpdir)
        .output()
        .expect("run sui-id backup")
}

#[test]
fn k05_backup_refuses_non_utf8_snapshot_path() {
    use std::os::unix::ffi::OsStrExt;
    let t = tempfile::tempdir().unwrap();
    let mut raw = t.path().as_os_str().as_bytes().to_vec();
    raw.extend_from_slice(b"/bad\xff");
    let out = backup_with_tmpdir(t.path(), std::ffi::OsStr::from_bytes(&raw));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{stderr}");
    assert!(
        stderr.contains("snapshot path must be valid UTF-8"),
        "{stderr}"
    );
}

#[test]
fn k06_backup_quotes_the_snapshot_path_for_vacuum_into() {
    let t = tempfile::tempdir().unwrap();
    let tmp = t.path().join("it's-quoted");
    std::fs::create_dir_all(&tmp).unwrap();
    let out = backup_with_tmpdir(t.path(), tmp.as_os_str());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "a quote in the path must be escaped: {stderr}"
    );
    assert!(t.path().join("sub.tar").exists());
}

#[test]
fn k07_backup_file_mode_is_0600() {
    let t = tempfile::tempdir().unwrap();
    let out = plain_backup(t.path());
    let mode = std::fs::metadata(&out).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

// ── restore / verify: inputs ─────────────────────────────────────────

#[test]
fn k08_restore_refuses_missing_source() {
    let t = tempfile::tempdir().unwrap();
    let e = restore_err(t.path(), &t.path().join("absent.tar"), no_force());
    assert!(e.contains("does not exist"), "{e}");
}

#[test]
fn k09_verify_refuses_missing_source() {
    let t = tempfile::tempdir().unwrap();
    let e = err_text(backup::run_verify(&t.path().join("absent.tar"), None));
    assert!(e.contains("does not exist"), "{e}");
}

#[test]
fn k10_encrypted_backup_requires_passphrase() {
    let t = tempfile::tempdir().unwrap();
    let a = encrypted_backup(t.path());
    let e = restore_err(t.path(), &a, no_force());
    assert!(e.contains("this backup is encrypted"), "{e}");
}

#[test]
fn k11_plain_backup_refuses_a_passphrase() {
    let t = tempfile::tempdir().unwrap();
    let a = plain_backup(t.path());
    let e = restore_err(
        t.path(),
        &a,
        RestoreOptions {
            force: false,
            passphrase: Some(PASS.into()),
        },
    );
    assert!(
        e.contains("not encrypted, but a passphrase was provided"),
        "{e}"
    );
}

#[test]
fn k12_refuses_unparseable_manifest() {
    let t = tempfile::tempdir().unwrap();
    let a = archive(
        t.path(),
        "m.tar",
        &[
            (b"MANIFEST.json", b"{not json"),
            (b"sui-id.sqlite", b"db"),
            (b"sui-id.key", b"k"),
        ],
    );
    let e = restore_err(t.path(), &a, no_force());
    assert!(e.contains("parsing MANIFEST.json"), "{e}");
}

#[test]
fn k13_refuses_archive_without_database_entry() {
    let t = tempfile::tempdir().unwrap();
    let m = manifest(1, 1);
    let a = archive(
        t.path(),
        "m.tar",
        &[(b"MANIFEST.json", &m), (b"sui-id.key", b"k")],
    );
    let e = restore_err(t.path(), &a, no_force());
    assert!(e.contains("backup is missing sui-id.sqlite entry"), "{e}");
}

#[test]
fn k14_refuses_archive_without_key_entry() {
    let t = tempfile::tempdir().unwrap();
    let m = manifest(1, 1);
    let a = archive(
        t.path(),
        "m.tar",
        &[(b"MANIFEST.json", &m), (b"sui-id.sqlite", b"db")],
    );
    let e = restore_err(t.path(), &a, no_force());
    assert!(e.contains("backup is missing sui-id.key entry"), "{e}");
}

#[test]
fn k15_restore_refuses_newer_format_version() {
    let t = tempfile::tempdir().unwrap();
    let m = manifest(2, 1);
    let a = archive(
        t.path(),
        "m.tar",
        &[
            (b"MANIFEST.json", &m),
            (b"sui-id.sqlite", b"db"),
            (b"sui-id.key", b"k"),
        ],
    );
    let e = restore_err(t.path(), &a, no_force());
    assert!(e.contains("backup format_version 2 is newer"), "{e}");
}

#[test]
fn k16_restore_refuses_newer_schema_version() {
    let t = tempfile::tempdir().unwrap();
    let m = manifest(1, 999_999);
    let a = archive(
        t.path(),
        "m.tar",
        &[
            (b"MANIFEST.json", &m),
            (b"sui-id.sqlite", b"db"),
            (b"sui-id.key", b"k"),
        ],
    );
    let e = restore_err(t.path(), &a, no_force());
    assert!(e.contains("backup schema_version 999999 is newer"), "{e}");
}

// ── restore: destinations ────────────────────────────────────────────

#[test]
fn k17_restore_refuses_existing_database_without_force() {
    let t = tempfile::tempdir().unwrap();
    let a = plain_backup(t.path());
    let dst = empty_target(t.path());
    std::fs::write(&dst.storage.db_path, b"existing").unwrap();
    let e = err_text(backup::run_restore(&dst, &a, &no_force()));
    assert!(e.contains("refusing to overwrite existing database"), "{e}");
}

#[test]
fn k18_restore_refuses_existing_key_without_force() {
    let t = tempfile::tempdir().unwrap();
    let a = plain_backup(t.path());
    let dst = empty_target(t.path());
    std::fs::write(&dst.storage.key_file, b"existing").unwrap();
    let e = err_text(backup::run_restore(&dst, &a, &no_force()));
    assert!(e.contains("refusing to overwrite existing key file"), "{e}");
}

#[test]
fn k19_restored_files_are_mode_0600() {
    let t = tempfile::tempdir().unwrap();
    let a = plain_backup(t.path());
    let dst = empty_target(t.path());
    backup::run_restore(&dst, &a, &no_force()).expect("restore");
    for p in [&dst.storage.db_path, &dst.storage.key_file] {
        let mode = std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{}", p.display());
    }
}

#[test]
fn k20_restore_replaces_via_temp_file_and_clears_a_stale_one() {
    let t = tempfile::tempdir().unwrap();
    let a = plain_backup(t.path());
    let dst = empty_target(t.path());
    std::fs::write(&dst.storage.db_path, b"old").unwrap();
    std::fs::write(&dst.storage.key_file, b"old").unwrap();
    // A leftover temp file from an interrupted restore: `create_new` on it
    // would fail unless the stale file is removed first.
    std::fs::write(dst.storage.db_path.with_extension("restoring"), b"stale").unwrap();
    backup::run_restore(
        &dst,
        &a,
        &RestoreOptions {
            force: true,
            passphrase: None,
        },
    )
    .expect("forced restore");
    assert_ne!(std::fs::read(&dst.storage.db_path).unwrap(), b"old");
    assert!(!dst.storage.db_path.with_extension("restoring").exists());
}

#[test]
fn k21_verify_refuses_corrupt_database() {
    let t = tempfile::tempdir().unwrap();
    let m = manifest(1, 1);
    let mut corrupt = b"SQLite format 3\0".to_vec();
    corrupt.resize(4096, 0xAB);
    let a = archive(
        t.path(),
        "m.tar",
        &[
            (b"MANIFEST.json", &m),
            (b"sui-id.sqlite", &corrupt),
            (b"sui-id.key", b"k"),
        ],
    );
    let e = err_text(backup::run_verify(&a, None));
    assert!(
        e.contains("integrity_check") || e.contains("integrity check"),
        "{e}"
    );
}

// ── encrypted envelope ───────────────────────────────────────────────

#[test]
fn k22_refuses_truncated_envelope() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("short.enc");
    std::fs::write(&p, b"SUIDIDBK\0\0\0\x01").unwrap();
    let e = err_text(backup::run_verify(&p, Some(PASS)));
    assert!(e.contains("encrypted backup truncated"), "{e}");
}

#[test]
fn k24_refuses_unsupported_envelope_version() {
    let t = tempfile::tempdir().unwrap();
    let a = encrypted_backup(t.path());
    let mut bytes = std::fs::read(&a).unwrap();
    bytes[8..12].copy_from_slice(&2u32.to_be_bytes());
    std::fs::write(&a, bytes).unwrap();
    let e = err_text(backup::run_verify(&a, Some(PASS)));
    assert!(e.contains("envelope version 2 is not supported"), "{e}");
}

#[test]
fn k26_refuses_wrong_passphrase() {
    let t = tempfile::tempdir().unwrap();
    let a = encrypted_backup(t.path());
    let e = err_text(backup::run_verify(&a, Some("wrong passphrase")));
    assert!(e.contains("could not decrypt backup"), "{e}");
}

#[test]
fn k26b_refuses_tampered_ciphertext() {
    let t = tempfile::tempdir().unwrap();
    let a = encrypted_backup(t.path());
    let mut bytes = std::fs::read(&a).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&a, bytes).unwrap();
    let e = err_text(backup::run_verify(&a, Some(PASS)));
    assert!(e.contains("could not decrypt backup"), "{e}");
}

// ── tar reader ───────────────────────────────────────────────────────

#[test]
fn k29_refuses_non_utf8_entry_name() {
    let t = tempfile::tempdir().unwrap();
    let a = archive(t.path(), "m.tar", &[(b"bad\xffname", b"x")]);
    let e = err_text(backup::run_verify(&a, None));
    assert!(e.contains("tar entry name is not UTF-8"), "{e}");
}

#[test]
fn k30_refuses_truncated_entry() {
    let t = tempfile::tempdir().unwrap();
    let mut buf = Vec::new();
    tar_entry(&mut buf, b"MANIFEST.json", &[b'x'; 2000]);
    buf.truncate(512 + 100); // header plus a fraction of the body
    let p = t.path().join("trunc.tar");
    std::fs::write(&p, buf).unwrap();
    let e = err_text(backup::run_verify(&p, None));
    assert!(e.contains("truncated tar entry for MANIFEST.json"), "{e}");
}

#[test]
fn k31_refuses_archive_with_no_entries() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("empty.tar");
    std::fs::write(&p, [0u8; 1024]).unwrap();
    let e = err_text(backup::run_verify(&p, None));
    assert!(e.contains("tar archive contains no entries"), "{e}");
}

#[test]
fn k32_refuses_invalid_octal_size() {
    let t = tempfile::tempdir().unwrap();
    let mut buf = Vec::new();
    tar_entry(&mut buf, b"MANIFEST.json", b"x");
    buf[124] = b'9';
    let p = t.path().join("octal.tar");
    std::fs::write(&p, buf).unwrap();
    let e = err_text(backup::run_verify(&p, None));
    assert!(e.contains("invalid octal digit in tar header"), "{e}");
}

// ── happy paths ──────────────────────────────────────────────────────

fn round_trip(passphrase: Option<&str>) {
    let t = tempfile::tempdir().unwrap();
    let src = source(t.path());
    let key = std::fs::read_to_string(&src.storage.key_file).unwrap();
    let out = t.path().join("rt.tar");
    backup::run_backup(
        &src,
        &out,
        &BackupOptions {
            passphrase: passphrase.map(str::to_owned),
        },
    )
    .expect("backup");
    let report = backup::run_verify(&out, passphrase).expect("verify");
    assert_eq!(report.encrypted, passphrase.is_some());
    assert!(report.key_present);
    let expected_schema = sui_id_store::migrations::MAX_SCHEMA_VERSION as i64;
    assert_eq!(report.manifest.schema_version, expected_schema);
    assert_eq!(report.manifest.format_version, backup::FORMAT_VERSION);

    let dst = empty_target(t.path());
    backup::run_restore(
        &dst,
        &out,
        &RestoreOptions {
            force: false,
            passphrase: passphrase.map(str::to_owned),
        },
    )
    .expect("restore");
    let restored_key = std::fs::read_to_string(&dst.storage.key_file).unwrap();
    assert_eq!(restored_key, key);
    let db = Database::open(
        &dst.storage.db_path,
        MasterKey::from_base64(restored_key.trim()).unwrap(),
    )
    .expect("restored database opens");
    drop(db);
}

#[test]
fn round_trip_plain() {
    round_trip(None);
}

#[test]
fn round_trip_encrypted() {
    round_trip(Some(PASS));
}

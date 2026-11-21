//! Master-key rotation CLI (v0.26.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use super::common::*;

// ---------- urgent fix 2026-11-03: stage the key before the re-seal
// commits (rfcs/handoffs/094-transactional-audit/
// urgent-master-key-ordering-2026-11-03.md) ----------
//
// These run the **real binary** (`CARGO_BIN_EXE_sui-id`), the same way
// `r112_stage2.rs` does, because the bug and its fix are entirely in
// `cli.rs`'s file handling: `rotate_master_key` itself (exercised
// in-process above) never touches the filesystem.
mod ordering {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("tempdir"),
            }
        }
        fn db(&self) -> PathBuf {
            self.dir.path().join("t.sqlite")
        }
        fn key(&self) -> PathBuf {
            self.dir.path().join("t.key")
        }
        fn staged_key(&self) -> PathBuf {
            self.dir.path().join("t.key.new")
        }
        fn config(&self) -> PathBuf {
            let sample = Command::new(BIN)
                .arg("--print-sample-config")
                .output()
                .expect("sample");
            let mut text = String::from_utf8(sample.stdout).expect("utf8");
            text = text
                .replace("./sui-id.sqlite", &self.db().display().to_string())
                .replace("./sui-id.key", &self.key().display().to_string())
                .replace("127.0.0.1:8801", "127.0.0.1:18813");
            let path = self.dir.path().join("cfg.toml");
            std::fs::write(&path, text).expect("config");
            path
        }
        /// Lets the binary create and migrate the database and bootstrap
        /// the key file (the command fails with "user not found", which
        /// is the point: the open succeeded).
        fn create(&self, cfg: &Path) {
            let out = self.run(
                &["admin", "unlock-user", "--username", "nobody", "--config"],
                cfg,
            );
            assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
            assert!(self.db().exists());
            assert!(self.key().exists());
        }
        fn run(&self, args: &[&str], cfg: &Path) -> Output {
            Command::new(BIN).args(args).arg(cfg).output().expect("run")
        }
        /// Same as [`Home::run`], with `SUI_ID_MASTER_KEY` set on the
        /// **child process only** (`Command::env`) -- never
        /// `std::env::set_var` in the test process itself, which would
        /// leak into every other test in this binary.
        fn run_with_master_key_env(&self, args: &[&str], cfg: &Path, key_b64: &str) -> Output {
            Command::new(BIN)
                .args(args)
                .arg(cfg)
                .env("SUI_ID_MASTER_KEY", key_b64)
                .output()
                .expect("run")
        }
        fn raw(&self) -> rusqlite::Connection {
            rusqlite::Connection::open(self.db()).expect("raw")
        }
        /// The most recent `note` recorded for `action`, if any row with
        /// that action exists.
        fn audit_note(&self, action: &str) -> Option<String> {
            self.raw()
                .query_row(
                    "SELECT note FROM audit_log WHERE action = ?1 ORDER BY seq DESC LIMIT 1",
                    [action],
                    |r| r.get(0),
                )
                .ok()
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }

    fn mode_bits(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    /// Insert a signing-key row whose `private_key_enc` cannot be
    /// decrypted under any key (too short for even the nonce+tag
    /// `crypto::open` requires) — a genuine re-seal failure, the
    /// nearest honest substitute for fault injection: the dynamic
    /// `FaultInjector` (`sui-id-store/src/backend.rs`) is
    /// `pub(crate)` to `sui-id-store` and unreachable from this
    /// crate's e2e binary, and nothing else hooks `with_tx_sync`
    /// for synthetic failure the way `class_a` wires its own.
    fn poison_a_signing_key(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO signing_keys(id, algorithm, private_key_enc, public_key, \
             is_active, created_at, rotated_at) \
             VALUES ('11111111-1111-1111-1111-111111111111', 'EdDSA', X'0000', X'00', \
             1, '2026-01-01T00:00:00Z', NULL)",
            [],
        )
        .expect("poison insert");
    }

    /// Test 1 — successful rotation (generate flow): the key is at the
    /// configured path, `.new` is gone, the old key is at
    /// `.bak.<stamp>`, and the database opens with the new key and not
    /// the old.
    #[test]
    fn successful_rotation_stages_then_lands_the_key_and_archives_the_old_one() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

        assert!(!h.staged_key().exists(), "staged key must be gone");
        assert!(h.key().exists(), "new key must be at the configured path");
        let new_key_body = std::fs::read_to_string(h.key()).expect("new key");
        assert_ne!(new_key_body, old_key_body, "the key actually changed");

        let bak = std::fs::read_dir(h.dir.path())
            .expect("read dir")
            .filter_map(|e| e.ok())
            .find(|e| e.file_name().to_string_lossy().starts_with("t.key.bak."))
            .expect("a .bak.<stamp> file exists");
        let bak_body = std::fs::read_to_string(bak.path()).expect("bak key");
        assert_eq!(bak_body, old_key_body, "the archived key is the old one");

        // RFC 094 cycle J stage 5: `admin.master_key.rotated`'s note now
        // reports the key files, not a row count -- that moved to
        // `admin.master_key.resealed` (K07), which this run also wrote.
        assert!(
            h.audit_note("admin.master_key.resealed").is_some(),
            "the sealed re-seal record exists"
        );
        let rotated_note = h
            .audit_note("admin.master_key.rotated")
            .expect("the rotated record exists");
        assert!(
            rotated_note.contains("new key in place at") && rotated_note.contains(" archived at"),
            "note reports the key files, not a row count: {rotated_note}"
        );
        assert!(
            !rotated_note.contains("rows"),
            "the row count belongs to admin.master_key.resealed now: {rotated_note}"
        );

        // The database now opens (and the server would start) with the
        // key at the configured path, i.e. the NEW one -- prove it by
        // running another admin command against the same config and
        // getting the ordinary "user not found" failure, not a refusal
        // to open the database.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
        assert!(
            !stderr(&out2).contains("refusing"),
            "the server would start fine with the key now at the configured path: {}",
            stderr(&out2)
        );
    }

    /// Test 2 — `.new` already exists: the command refuses, and
    /// nothing is touched (neither key file, nor the database).
    #[test]
    fn staged_key_already_present_refuses_before_touching_anything() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");
        std::fs::write(h.staged_key(), "stray-leftover-from-a-previous-run\n").expect("stray");
        let stray_body = std::fs::read_to_string(h.staged_key()).expect("stray body");

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(1), "must refuse: {}", stderr(&out));
        assert!(
            stderr(&out).contains("already exists")
                && stderr(&out).contains("may not have completed"),
            "{}",
            stderr(&out)
        );

        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key still there"),
            old_key_body,
            "the old key file is untouched"
        );
        assert_eq!(
            std::fs::read_to_string(h.staged_key()).expect("stray still there"),
            stray_body,
            "the stray staged file is untouched, not overwritten"
        );
        assert!(
            std::fs::read_dir(h.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .all(|e| !e.file_name().to_string_lossy().starts_with("t.key.bak.")),
            "no .bak file was created -- the database was never re-sealed"
        );

        // The database still opens with the OLD key.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
    }

    /// Test 3 — the staged file is created with mode `0600`, not
    /// `chmod`'d afterward.
    #[test]
    fn staged_key_is_created_with_owner_only_permissions() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);

        // A corrupted signing key stalls the rotation right after the
        // staged file is written (see test 4) -- which is exactly the
        // window this test needs to inspect the file mid-rotation.
        poison_a_signing_key(&h.raw());

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            h.staged_key().exists(),
            "the staged key survives the re-seal failure"
        );
        assert_eq!(mode_bits(&h.staged_key()), 0o600);
    }

    /// Test 4 (§3's ordering test) — a re-seal failure leaves the
    /// staged key on disk and the database still opens under the OLD
    /// key: the assertion that proves the key is persisted before the
    /// irreversible step, against a genuine failure rather than an
    /// injected one.
    #[test]
    fn reseal_failure_leaves_the_staged_key_and_the_old_key_file_untouched() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");
        poison_a_signing_key(&h.raw());

        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("rotating sealed columns"),
            "failed inside the re-seal, not before it: {}",
            stderr(&out)
        );

        // The new key is durable, even though rotation did not finish.
        assert!(h.staged_key().exists(), "the new key is still recoverable");
        let staged_body = std::fs::read_to_string(h.staged_key()).expect("staged body");
        assert_ne!(staged_body, old_key_body);

        // Neither key file at the configured path moved.
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("old key still there"),
            old_key_body
        );
        assert!(
            std::fs::read_dir(h.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .all(|e| !e.file_name().to_string_lossy().starts_with("t.key.bak.")),
            "no rename happened -- the transaction rolled back"
        );

        // And the database still opens with the OLD key: the same
        // config, same key file, an ordinary command succeeds exactly
        // as it did before the failed attempt.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
    }

    /// `0.91.0` stage 1, part B — a key that came from `SUI_ID_MASTER_KEY`
    /// must not be rotated: the command would write the new key to the
    /// *file*, which a start with the variable still set would ignore,
    /// stranding the server on the old key against a database rotation
    /// had already re-sealed. The variable here carries the *same* key
    /// already at the configured path, so the only thing this test can
    /// be failing on is the origin check -- not an unrelated wrong-key
    /// refusal from `database::open`.
    /// `0.91.0` stage 1, A2, the operator's side: when something else holds
    /// the database open (here a raw read transaction, as a running server
    /// would), K07 commits but cannot be made durable. The command must say
    /// where each key is and what to do. It must not invite a second
    /// rotation, which would start from the old key, which no longer opens
    /// the database.
    #[test]
    fn a_reseal_that_cannot_be_made_durable_says_where_the_keys_are() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let old_key = std::fs::read_to_string(h.key()).expect("key");

        let holder = h.raw();
        holder
            .execute_batch("BEGIN; SELECT * FROM signing_keys;")
            .expect("hold a read transaction, as a running server would");
        let out = h.run(&["admin", "rotate-key", "--yes", "--config"], &cfg);
        holder.execute_batch("ROLLBACK;").ok();
        drop(holder);

        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        let err = stderr(&out);
        let staged = h.staged_key().display().to_string();
        let active = h.key().display().to_string();
        for must in [
            "now sealed under the NEW key",
            staged.as_str(),
            active.as_str(),
            "stop sui-id",
            "do not run the rotation again",
        ] {
            assert!(err.contains(must), "stderr must contain {must:?}: {err}");
        }
        assert!(h.staged_key().exists(), "the new key is still staged");
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("old key still in place"),
            old_key,
            "the key files were not touched"
        );
        assert!(
            h.audit_note("admin.master_key.resealed").is_some(),
            "the re-seal did commit"
        );
    }

    #[test]
    fn rotation_refuses_when_the_old_key_came_from_the_environment() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let key_body = std::fs::read_to_string(h.key()).expect("key");

        let out = h.run_with_master_key_env(
            &["admin", "rotate-key", "--yes", "--config"],
            &cfg,
            key_body.trim(),
        );
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("SUI_ID_MASTER_KEY"),
            "the refusal must name the variable: {}",
            stderr(&out)
        );

        assert!(
            !h.staged_key().exists(),
            "nothing was staged -- the refusal runs before the database opens"
        );
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key still there"),
            key_body,
            "the key file's bytes are unchanged"
        );
        assert!(
            h.audit_note("admin.master_key.resealed").is_none(),
            "no re-seal was attempted"
        );

        // An ordinary command still runs against the configured key FILE,
        // with no environment variable this time. (`unlock-user` decrypts
        // nothing, so this is not evidence about the sealed rows; that
        // nothing was re-sealed is shown above by the absent audit row.)
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
        assert!(
            !stderr(&out2).contains("refusing"),
            "the original key file must still open the database: {}",
            stderr(&out2)
        );
    }
}

// ---------- RFC 100 stage 2 (2026-11-21): `--check` ----------
mod check_mode {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("tempdir"),
            }
        }
        fn db(&self) -> PathBuf {
            self.dir.path().join("t.sqlite")
        }
        fn key(&self) -> PathBuf {
            self.dir.path().join("t.key")
        }
        fn config(&self) -> PathBuf {
            let sample = Command::new(BIN)
                .arg("--print-sample-config")
                .output()
                .expect("sample");
            let mut text = String::from_utf8(sample.stdout).expect("utf8");
            text = text
                .replace("./sui-id.sqlite", &self.db().display().to_string())
                .replace("./sui-id.key", &self.key().display().to_string())
                .replace("127.0.0.1:8801", "127.0.0.1:18815");
            let path = self.dir.path().join("cfg.toml");
            std::fs::write(&path, text).expect("config");
            path
        }
        fn create(&self, cfg: &Path) {
            let out = self.run(
                &["admin", "unlock-user", "--username", "nobody", "--config"],
                cfg,
            );
            assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
            assert!(self.db().exists());
            assert!(self.key().exists());
        }
        fn run(&self, args: &[&str], cfg: &Path) -> Output {
            Command::new(BIN).args(args).arg(cfg).output().expect("run")
        }
        fn run_with_master_key_env(&self, args: &[&str], cfg: &Path, key_b64: &str) -> Output {
            Command::new(BIN)
                .args(args)
                .arg(cfg)
                .env("SUI_ID_MASTER_KEY", key_b64)
                .output()
                .expect("run")
        }
        /// Every file under `dir`, with its contents, independent of
        /// `readdir` order -- `--check` claims to touch nothing, and this
        /// is what proves it, not just "the same files still exist."
        fn snapshot(&self) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
            std::fs::read_dir(self.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .map(|e| {
                    let path = e.path();
                    let body = std::fs::read(&path).expect("read");
                    (path, body)
                })
                .collect()
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }
    fn stdout(o: &Output) -> String {
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    #[test]
    fn a_healthy_instance_passes_and_the_directory_is_byte_identical_after() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let before = h.snapshot();

        let out = h.run(
            &["admin", "master-key", "rotate", "--check", "--config"],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            stdout(&out).contains("All checked preconditions passed."),
            "{}",
            stdout(&out)
        );

        assert_eq!(
            h.snapshot(),
            before,
            "--check must create, remove or rename nothing"
        );
    }

    #[test]
    fn a_symlinked_key_file_refuses_and_names_the_link() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let real = h.dir.path().join("t.key.real");
        std::fs::rename(h.key(), &real).expect("move key aside");
        std::os::unix::fs::symlink(&real, h.key()).expect("symlink");

        let out = h.run(
            &["admin", "master-key", "rotate", "--check", "--config"],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        let report = stdout(&out);
        assert!(
            report.contains(&h.key().display().to_string())
                && report.contains(&real.display().to_string()),
            "the report names the link and its target: {report}"
        );
    }

    #[test]
    fn refuses_when_the_key_came_from_the_environment() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let key_body = std::fs::read_to_string(h.key()).expect("key");
        let before = h.snapshot();

        let out = h.run_with_master_key_env(
            &["admin", "master-key", "rotate", "--check", "--config"],
            &cfg,
            key_body.trim(),
        );
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        assert!(
            stdout(&out).contains("SUI_ID_MASTER_KEY"),
            "{}",
            stdout(&out)
        );
        assert_eq!(h.snapshot(), before, "--check creates nothing");
    }

    #[test]
    fn reports_every_failing_precondition_at_once() {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg);
        let staged = h.dir.path().join("t.key.new");
        std::fs::write(&staged, "leftover").expect("leftover");
        let real = h.dir.path().join("t.key.real");
        std::fs::rename(h.key(), &real).expect("move key aside");
        std::os::unix::fs::symlink(&real, h.key()).expect("symlink");

        let out = h.run(
            &["admin", "master-key", "rotate", "--check", "--config"],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        let report = stdout(&out);
        assert!(report.contains("[FAIL] P3"), "{report}");
        assert!(report.contains("[FAIL] P4"), "{report}");
        assert!(
            report.contains("2 of 4 checked precondition(s) failed"),
            "{report}"
        );
    }
}

// ---------- RFC 100 stage 0 (2026-11-16): the crash-injection
// harness's Failpoint adapter, through the real binary ----------
//
// `SUI_ID_FAILPOINT=<point>:<action>` only exists under the
// `test-support` feature (`key_adapter::default_adapter`), so this
// drives it exactly the way `ordering`'s `SUI_ID_MASTER_KEY` tests do:
// on the **child process only**.
mod failpoint {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use sui_id_store::crypto::MasterKey;

    const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

    /// Every point named in the adapter's two call sites
    /// (`write_key_file_impl`'s `"rotate.stage"` prefix, and
    /// `rotate_key_files`'s own renames/syncs) — `rfcs/handoffs/
    /// 100-master-key-rotation/0.92.0-stage-0-the-harness-2026-11-16.md`
    /// §A/§D5.
    const POINTS: &[&str] = &[
        "rotate.stage.create_new",
        "rotate.stage.write",
        "rotate.stage.sync_file",
        "rotate.stage.sync_dir",
        "rotate.rename.1",
        "rotate.sync_dir.after_rename.1",
        "rotate.rename.2",
        "rotate.sync_dir.after_rename.2",
    ];

    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("tempdir"),
            }
        }
        fn db(&self) -> PathBuf {
            self.dir.path().join("t.sqlite")
        }
        fn key(&self) -> PathBuf {
            self.dir.path().join("t.key")
        }
        fn staged_key(&self) -> PathBuf {
            self.dir.path().join("t.key.new")
        }
        fn config(&self) -> PathBuf {
            let sample = Command::new(BIN)
                .arg("--print-sample-config")
                .output()
                .expect("sample");
            let mut text = String::from_utf8(sample.stdout).expect("utf8");
            text = text
                .replace("./sui-id.sqlite", &self.db().display().to_string())
                .replace("./sui-id.key", &self.key().display().to_string())
                .replace("127.0.0.1:8801", "127.0.0.1:18814");
            let path = self.dir.path().join("cfg.toml");
            std::fs::write(&path, text).expect("config");
            path
        }
        /// Creates and migrates the database, bootstraps the key file —
        /// and, unlike `admin unlock-user` alone, seeds one active
        /// signing key directly, so [`key_opens`] has something real to
        /// unseal (no HTTP setup wizard runs in this binary-only
        /// harness).
        async fn create(&self, cfg: &Path) {
            let out = self.run(
                &["admin", "unlock-user", "--username", "nobody", "--config"],
                cfg,
            );
            assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
            assert!(self.db().exists());
            assert!(self.key().exists());

            let body = std::fs::read_to_string(self.key()).expect("key body");
            let key = MasterKey::from_base64(body.trim()).expect("key");
            let mut bytes = *key.as_bytes();
            let db_key = MasterKey::from_bytes(bytes);
            use zeroize::Zeroize;
            bytes.zeroize();
            let db = sui_id_store::Database::open(&self.db(), db_key).expect("open");
            sui_id_store::repos::signing_keys::insert_with_plaintext(
                &db,
                sui_id_shared::ids::SigningKeyId::new(),
                "EdDSA",
                b"a signing private key, for this test only",
                b"a public key",
                true,
            )
            .await
            .expect("seed a signing key");
        }
        fn run(&self, args: &[&str], cfg: &Path) -> Output {
            Command::new(BIN).args(args).arg(cfg).output().expect("run")
        }
        /// Same as [`Home::run`], with `SUI_ID_FAILPOINT` set on the
        /// **child process only**.
        fn run_with_failpoint(&self, args: &[&str], cfg: &Path, spec: &str) -> Output {
            Command::new(BIN)
                .args(args)
                .arg(cfg)
                .env("SUI_ID_FAILPOINT", spec)
                .output()
                .expect("run")
        }
        fn bak_files(&self) -> Vec<PathBuf> {
            std::fs::read_dir(self.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("t.key.bak."))
                .map(|e| e.path())
                .collect()
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }

    /// A genuine decrypt attempt, not a byte comparison: does the key at
    /// `key_path` unseal the active signing key in the database at
    /// `db_path`? The dispatch's §C oracle, reused by §D5.
    async fn key_opens(key_path: &Path, db_path: &Path) -> bool {
        let Ok(body) = std::fs::read_to_string(key_path) else {
            return false;
        };
        let Ok(key) = MasterKey::from_base64(body.trim()) else {
            return false;
        };
        let mut bytes = *key.as_bytes();
        let key_for_db = MasterKey::from_bytes(bytes);
        use zeroize::Zeroize;
        bytes.zeroize();
        let Ok(db) = sui_id_store::Database::open(db_path, key_for_db) else {
            return false;
        };
        match sui_id_store::repos::signing_keys::active(&db).await {
            Ok(row) => sui_id_store::crypto::open(
                &key,
                &row.private_key_enc,
                sui_id_store::repos::signing_keys::SIGNING_KEY_AAD,
            )
            .is_ok(),
            Err(_) => false,
        }
    }

    /// M2: some file in `h`'s directory holds the key the database is
    /// actually sealed under.
    async fn m2_holds_on_disk(h: &Home) -> bool {
        let mut candidates = vec![h.key(), h.staged_key()];
        candidates.extend(h.bak_files());
        for path in candidates {
            if path.exists() && key_opens(&path, &h.db()).await {
                return true;
            }
        }
        false
    }

    /// For every listed point, `abort` leaves a directory in which M2
    /// holds — a kill loses nothing written, so this is a real-process
    /// confirmation of D1's kill-model finding, not a new claim.
    #[tokio::test]
    async fn abort_at_every_point_leaves_m2_holding() {
        for point in POINTS {
            let h = Home::new();
            let cfg = h.config();
            h.create(&cfg).await;
            let spec = format!("{point}:abort");

            let out =
                h.run_with_failpoint(&["admin", "rotate-key", "--yes", "--config"], &cfg, &spec);
            assert!(
                out.status.code().is_none(),
                "abort at {point} must terminate by signal, not exit normally: {:?}",
                out.status
            );
            assert!(
                m2_holds_on_disk(&h).await,
                "M2 must hold after an abort at {point}"
            );
        }
    }

    /// A short write at the staging write must leave the old key active
    /// and the database untouched — nothing after that point ever runs.
    #[tokio::test]
    async fn short_write_at_the_staging_write_leaves_the_old_key_active_and_the_database_untouched()
    {
        let h = Home::new();
        let cfg = h.config();
        h.create(&cfg).await;
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");

        let out = h.run_with_failpoint(
            &["admin", "rotate-key", "--yes", "--config"],
            &cfg,
            "rotate.stage.write:short-write",
        );
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));

        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key"),
            old_key_body,
            "the old key file is untouched"
        );
        assert!(
            key_opens(&h.key(), &h.db()).await,
            "the old key must still open the database -- untouched"
        );
        assert!(
            h.bak_files().is_empty(),
            "no rename happened -- the database was never re-sealed"
        );
    }

    /// RFC 100 stage 2, P5: a real rotation probes the filesystem first,
    /// after preconditions pass and before anything else. A failure at
    /// any of its primitives must refuse before the staged key or the
    /// `.bak` file ever appear, and the probe file must not be left
    /// behind under either of its two possible names.
    const PROBE_POINTS: &[&str] = &[
        "rotate.probe.create_new",
        "rotate.probe.write",
        "rotate.probe.sync_file",
        "rotate.probe.sync_dir",
        "rotate.probe.rename",
        "rotate.probe.sync_dir.after_rename",
    ];

    #[tokio::test]
    async fn probe_failure_at_each_primitive_refuses_before_anything_else() {
        for point in PROBE_POINTS {
            let h = Home::new();
            let cfg = h.config();
            h.create(&cfg).await;
            let old_key_body = std::fs::read_to_string(h.key()).expect("old key");
            let spec = format!("{point}:error");

            let out =
                h.run_with_failpoint(&["admin", "rotate-key", "--yes", "--config"], &cfg, &spec);
            assert_eq!(out.status.code(), Some(1), "at {point}: {}", stderr(&out));
            assert!(
                stderr(&out).contains("does not support"),
                "at {point}, the refusal names a filesystem limitation: {}",
                stderr(&out)
            );
            assert_eq!(
                std::fs::read_to_string(h.key()).expect("old key"),
                old_key_body,
                "at {point}: the old key file is untouched"
            );
            assert!(
                !h.staged_key().exists(),
                "at {point}: nothing was staged -- the probe runs first"
            );
            assert!(h.bak_files().is_empty(), "at {point}: no rename happened");
            for probe_name in ["t.key.rotation-probe", "t.key.rotation-probe.2"] {
                assert!(
                    !h.dir.path().join(probe_name).exists(),
                    "at {point}: the probe file must not be left behind"
                );
            }
        }
    }
}

// ---------- v0.26.0: Master-key rotation ----------

/// End-to-end rotation: set up sui-id, enroll TOTP and a passkey,
/// configure SMTP password, then rotate the master key and assert
/// every sealed-row read still works under the new key.
#[tokio::test]
async fn rotation_reseal_succeeds_and_old_key_no_longer_decrypts() {
    use sui_id_store::commands::reseal_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // The setup wizard creates a signing key (active) — that's
    // already one sealed row. Add an SMTP password to also exercise
    // the smtp_config path.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");

    // Add an SMTP config row with a sealed password so rotation
    // has something to re-key in that table.
    {
        use chrono::Utc;
        let now = Utc::now();
        sui_id_store::repos::smtp_config::upsert(
            &state.db,
            &sui_id_store::models::SmtpConfigRow {
                enabled: true,
                host: "smtp.test".into(),
                port: 587,
                tls_mode: sui_id_store::models::SmtpTlsMode::StartTls,
                username: Some("user".into()),
                password_enc: Some(
                    sui_id_store::crypto::seal(
                        state.db.key(),
                        b"my-smtp-password",
                        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
                    )
                    .expect("seal smtp pw"),
                ),
                from_address: "alice@example.test".into(),
                from_name: None,
                base_url: "https://idp.test".into(),
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("smtp upsert");
    }

    // Generate a brand-new master key.
    let new_key = MasterKey::generate();

    // The Database carries the old key. We open a SECOND handle
    // to the same underlying DB file? In-memory DBs cannot be
    // re-opened, so we test the rotation on the existing handle:
    // run rotation, then re-open under the new key and verify.
    //
    // The in-memory DB shares its connection across the
    // Database handle, so after rotation the same handle still
    // works (the key field is unchanged). Reading sealed columns
    // through that old handle would now FAIL — which is exactly
    // what we assert.
    let before_audit_rows = sui_id_store::repos::audit::recent(&state.db, 1000)
        .await
        .expect("audit tail")
        .len();
    let report = reseal_master_key(&state.db, &new_key)
        .await
        .expect("rotate")
        .into_inner();

    assert!(
        report.signing_keys >= 1,
        "expected at least 1 signing key re-sealed, got {}",
        report.signing_keys
    );
    assert_eq!(report.smtp_config, 1, "smtp password should re-seal");
    assert!(report.total() >= 2);

    // The OLD key (still held by `state.db`) must no longer
    // decrypt the signing key column — it has been re-sealed
    // under `new_key`.
    let signing_row = sui_id_store::repos::signing_keys::active(&state.db)
        .await
        .expect("active");
    let opened_old = sui_id_store::crypto::open(
        state.db.key(),
        &signing_row.private_key_enc,
        b"sui-id/signing_key/v1",
    );
    assert!(
        opened_old.is_err(),
        "old key must no longer decrypt the re-sealed column"
    );
    // The NEW key decrypts it.
    let opened_new = sui_id_store::crypto::open(
        &new_key,
        &signing_row.private_key_enc,
        b"sui-id/signing_key/v1",
    );
    assert!(
        opened_new.is_ok(),
        "new key must decrypt the re-sealed column"
    );

    // SMTP password also re-sealed.
    let smtp_row = sui_id_store::repos::smtp_config::get(&state.db)
        .await
        .expect("smtp")
        .expect("smtp configured");
    let smtp_enc = smtp_row.password_enc.expect("password set");
    let opened_old_smtp = sui_id_store::crypto::open(
        state.db.key(),
        &smtp_enc,
        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
    );
    assert!(opened_old_smtp.is_err());
    let opened_new_smtp = sui_id_store::crypto::open(
        &new_key,
        &smtp_enc,
        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
    );
    assert_eq!(
        opened_new_smtp.expect("decrypt with new key"),
        b"my-smtp-password"
    );

    // RFC 094 cycle J stage 4: the re-seal is now itself a sealed
    // command (K07), so the audit row commits with it regardless of
    // whether the CLI is the caller — asserted here, not left to the
    // CLI-path test, since this test IS a caller now.
    let after_audit = sui_id_store::repos::audit::recent(&state.db, 1000)
        .await
        .expect("audit tail");
    assert_eq!(
        after_audit.len(),
        before_audit_rows + 1,
        "exactly one audit row from the reseal"
    );
    assert_eq!(after_audit[0].action, "admin.master_key.resealed");

    // Avoid an unused-variable warning.
    let _ = user;
}

/// Sanity: rotation on a DB with no sealed rows runs successfully
/// and reports zeroes (other than the signing-key the setup
/// wizard always creates). Pin this so future migrations that
/// add new sealed columns have to update the rotation entry
/// list.
#[tokio::test]
async fn rotation_on_minimal_db_only_rekeys_signing_key() {
    use sui_id_store::commands::reseal_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // Brand-new install: only the active signing key exists as
    // a sealed row. (Refresh tokens are issued by /token; not
    // exercised here. TOTP / WebAuthn / SMTP all require admin
    // action.)
    let new_key = MasterKey::generate();
    let report = reseal_master_key(&state.db, &new_key)
        .await
        .expect("rotate")
        .into_inner();
    assert_eq!(report.signing_keys, 1);
    assert_eq!(report.refresh_tokens, 0);
    assert_eq!(report.user_totp_secrets, 0);
    assert_eq!(report.user_totp_recovery_codes, 0);
    assert_eq!(report.user_webauthn_credentials, 0);
    assert_eq!(report.smtp_config, 0);
    assert_eq!(report.total(), 1);
}

// ---------- RFC 100 stage 4 (2026-11-21): startup recovery, through the
// real binary ----------
mod recovery {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use sui_id::key_adapter::{KeyFileOps, Real};
    use sui_id::rotate::journal::{self, BackupDisposition, JournalBody, JournalState};
    use sui_id::rotate::preconditions::RotationPaths;
    use sui_id_store::Database;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::master_key_check::ensure_sentinel_written;

    const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("tempdir"),
            }
        }
        fn db(&self) -> PathBuf {
            self.dir.path().join("t.sqlite")
        }
        fn key(&self) -> PathBuf {
            self.dir.path().join("t.key")
        }
        fn config(&self, port: u16) -> PathBuf {
            let sample = Command::new(BIN)
                .arg("--print-sample-config")
                .output()
                .expect("sample");
            let mut text = String::from_utf8(sample.stdout).expect("utf8");
            text = text
                .replace("./sui-id.sqlite", &self.db().display().to_string())
                .replace("./sui-id.key", &self.key().display().to_string())
                .replace("127.0.0.1:8801", &format!("127.0.0.1:{port}"));
            let path = self.dir.path().join("cfg.toml");
            std::fs::write(&path, text).expect("config");
            path
        }
        fn run(&self, args: &[&str], cfg: &Path) -> Output {
            Command::new(BIN).args(args).arg(cfg).output().expect("run")
        }
        fn snapshot(&self) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
            fn walk(dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
                for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, out);
                    } else {
                        out.insert(path.clone(), std::fs::read(&path).expect("read"));
                    }
                }
            }
            let mut out = std::collections::BTreeMap::new();
            walk(self.dir.path(), &mut out);
            out
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }
    fn stdout(o: &Output) -> String {
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    fn write_key_at(path: &Path, key: &MasterKey, mode: u32) {
        std::fs::write(path, format!("{}\n", key.to_base64())).expect("write key");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
    }

    /// Seeds `home` with a real, sentinel-sealed database under a fresh
    /// old key, and a real rotation workspace prepared (through the
    /// library directly -- nothing in the live binary writes one yet)
    /// up to `state`, with whatever outside-workspace artifacts that
    /// state implies.
    async fn seed_leftover(home: &Home, state: JournalState) -> RotationPaths {
        let old_key = MasterKey::generate();
        let new_key = MasterKey::generate();
        let db = Database::open(&home.db(), old_key_copy(&old_key)).expect("open");
        ensure_sentinel_written(&db, db.key())
            .await
            .expect("sentinel");
        drop(db);
        write_key_at(&home.key(), &old_key, 0o600);

        let paths = RotationPaths::for_key_file(&home.key());
        journal::lock_workspace(&Real, &paths.workspace).expect("lock");
        let body = sample_body(&paths, &old_key, &new_key);
        let mut envelope = journal::write_intent(&Real, &paths.workspace, body, &old_key, &new_key)
            .expect("write_intent");
        if state >= JournalState::NextTempVerified {
            write_key_at(&paths.next_temp, &new_key, 0o600);
            envelope = journal::advance(
                &Real,
                &paths.workspace,
                &envelope,
                JournalState::NextTempVerified,
                &old_key,
                &new_key,
            )
            .expect("advance");
        }
        if state >= JournalState::NextPublished {
            Real.publish_no_replace("t.next.publish", &paths.next_temp, &paths.next)
                .expect("publish next");
            journal::advance(
                &Real,
                &paths.workspace,
                &envelope,
                JournalState::NextPublished,
                &old_key,
                &new_key,
            )
            .expect("advance");
        }
        paths
    }

    /// `JournalBody`'s own fields are all `pub`, but the convenience
    /// constructor `journal::sample_body` is `pub(crate)` to `sui-id` --
    /// invisible from this separate integration-test crate. The real
    /// uid/mode here match what `Real::create_new` actually produces.
    fn sample_body(paths: &RotationPaths, old_key: &MasterKey, new_key: &MasterKey) -> JournalBody {
        let p = |path: &Path| path.to_str().expect("utf8").to_owned();
        JournalBody {
            rotation_id: "11111111-1111-1111-1111-111111111111".to_owned(),
            old_fingerprint: sui_id_store::crypto::fingerprint(old_key),
            new_fingerprint: sui_id_store::crypto::fingerprint(new_key),
            active_path: p(&paths.active),
            workspace_path: p(&paths.workspace),
            next_path: p(&paths.next),
            backup_path: p(&paths.backup),
            intent_temp_path: p(&paths.intent_temp),
            update_temp_path: p(&paths.update_temp),
            next_temp_path: p(&paths.next_temp),
            backup_temp_path: p(&paths.backup_temp),
            expected_uid: rustix::process::geteuid().as_raw(),
            expected_mode: 0o600,
            backup_disposition: BackupDisposition::RemoveAfterVerified {
                verification_reference: "test-only".to_owned(),
            },
            state: JournalState::Intent,
        }
    }

    fn old_key_copy(key: &MasterKey) -> MasterKey {
        let mut bytes = *key.as_bytes();
        let copy = MasterKey::from_bytes(bytes);
        zeroize::Zeroize::zeroize(&mut bytes);
        copy
    }

    #[tokio::test]
    async fn an_admin_command_cleans_a_leftover_and_reports_it_on_stderr_first() {
        let home = Home::new();
        let cfg = home.config(18816);
        let paths = seed_leftover(&home, JournalState::Intent).await;

        let out = home.run(
            &[
                "admin",
                "user",
                "unlock",
                "--username",
                "nobody",
                "--config",
            ],
            &cfg,
        );
        let err = stderr(&out);
        assert!(
            err.starts_with("a stopped rotation was found and cleaned"),
            "the report is the first thing printed: {err}"
        );
        assert!(
            err.contains(&format!("removed {}", paths.workspace.display())),
            "{err}"
        );
        assert!(!paths.workspace.exists());
    }

    #[tokio::test]
    async fn the_server_itself_cleans_a_leftover_and_logs_it() {
        use std::io::Read;
        use std::time::{Duration, Instant};

        let home = Home::new();
        let cfg = home.config(18817);
        let paths = seed_leftover(&home, JournalState::Intent).await;

        let mut child = Command::new(BIN)
            .arg("serve")
            .arg("--config")
            .arg(&cfg)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn");
        let out_buf = drain(child.stdout.take().expect("stdout"));
        let err_buf = drain(child.stderr.take().expect("stderr"));

        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().expect("wait") {
                panic!(
                    "sui-id serve exited early ({status:?}): stdout={} stderr={}",
                    String::from_utf8_lossy(&out_buf.lock().expect("lock")),
                    String::from_utf8_lossy(&err_buf.lock().expect("lock")),
                );
            }
            let err_text = String::from_utf8_lossy(&err_buf.lock().expect("lock")).into_owned();
            if err_text.contains("a stopped rotation was found and cleaned") {
                assert!(
                    err_text.contains(&format!("removed {}", paths.workspace.display())),
                    "{err_text}"
                );
                let _ = child.kill();
                return;
            }
            // A hang guard, not a speed assertion (see o2a_cli_registry.rs).
            if start.elapsed() > Duration::from_secs(180) {
                let _ = child.kill();
                panic!("sui-id serve never reported the cleaned rotation: stderr={err_text}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        fn drain(
            mut pipe: impl Read + Send + 'static,
        ) -> std::sync::Arc<std::sync::Mutex<Vec<u8>>> {
            let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let shared = buf.clone();
            std::thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => shared.lock().expect("lock").extend_from_slice(&chunk[..n]),
                    }
                }
            });
            buf
        }
    }

    #[tokio::test]
    async fn a_next_key_of_the_wrong_fingerprint_refuses_before_readiness_and_changes_nothing() {
        let home = Home::new();
        let cfg = home.config(18818);
        let paths = seed_leftover(&home, JournalState::NextPublished).await;
        write_key_at(&paths.next, &MasterKey::generate(), 0o600);
        let before = home.snapshot();

        let out = home.run(
            &[
                "admin",
                "user",
                "unlock",
                "--username",
                "nobody",
                "--config",
            ],
            &cfg,
        );
        assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
        assert!(stderr(&out).contains("fingerprint"), "{}", stderr(&out));
        assert_eq!(
            home.snapshot(),
            before,
            "byte-identical after a fail-closed refusal"
        );
    }
}

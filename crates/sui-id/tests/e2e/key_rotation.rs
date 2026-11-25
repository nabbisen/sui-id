//! Master-key rotation CLI (v0.26.0).
//!
//! Part of the integration test binary. RFC 100 `0.94.0` P2's own
//! sub-modules build their own fixtures directly, through the real
//! binary; none of them need `super::common`'s HTTP-level helpers.

#![allow(dead_code)]

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

// ---------- RFC 100 `0.94.0` P2 (2026-11-24): the crash-injection
// harness's Failpoint adapter, through the real binary, against the
// journalled sequence ----------
//
// `SUI_ID_FAILPOINT=<point>:<action>` only exists under the
// `test-support` feature (`key_adapter::default_adapter`), so this
// drives it on the **child process only**. `crash-matrix.md`'s own
// thorough enumeration (every subset of every pending operation, at
// every point) runs in-process against `rotate::tests` -- `Failpoint`
// fires at most once per distinct point *name* per process, so this is
// a real-binary **sample**, one abort per name, confirming termination
// behaves the way the in-process kill model predicts, not a second
// exhaustive pass.
mod failpoint {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use sui_id_store::crypto::MasterKey;

    const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

    /// RFC 100 `0.94.0` P2 §2: every rotation needs exactly one custody
    /// choice now. Every `admin rotate-key`/`admin master-key rotate`
    /// invocation in this module appends these.
    const CUSTODY_ARGS: &[&str] = &[
        "--discard-backup-once-verified",
        "--verification-reference",
        "e2e-test",
    ];

    /// `prefix` ends right before `--config` -- this inserts the
    /// custody choice there and re-adds `--config` itself at the end,
    /// so the caller's own `.arg(cfg)` (`Home::run`/`run_with_failpoint`)
    /// still supplies `--config`'s value last, exactly as it always has.
    fn rotate_args<'a>(prefix: &[&'a str]) -> Vec<&'a str> {
        prefix
            .iter()
            .copied()
            .chain(CUSTODY_ARGS.iter().copied())
            .chain(["--config"])
            .collect()
    }

    /// Every point the new sequence's preparation and activation steps
    /// (`crash-matrix.md` points 1-7, 9) name, through the adapter --
    /// points 8 and 10 (K08, K05) have no `KeyFileOps` point of their
    /// own to abort at; their crash safety is the Class-A transaction's
    /// own atomicity, exercised at `sui-id-store` level.
    const PRE_ACTIVATION_POINTS: &[&str] = &[
        "journal.lock.create_dir",
        "journal.lock.sync_parent",
        "journal.intent.create_new",
        "journal.intent.write",
        "journal.intent.sync_file",
        "journal.intent.readback",
        "journal.intent.publish",
        "journal.intent.sync_dir",
        "rotate.next.create_new",
        "rotate.next.write",
        "rotate.next.sync_file",
        "rotate.next.readback",
        "journal.advance.create_new",
        "journal.advance.write",
        "journal.advance.sync_file",
        "journal.advance.readback",
        "journal.advance.replace",
        "journal.advance.sync_dir",
        "rotate.next.publish",
        "rotate.next.sync_dir.workspace",
        "rotate.next.sync_dir.active",
        "rotate.backup.create_new",
        "rotate.backup.write",
        "rotate.backup.sync_file",
        "rotate.backup.readback",
        "rotate.backup.publish",
        "rotate.backup.sync_dir.workspace",
        "rotate.backup.sync_dir.active",
    ];

    const ACTIVATION_POINTS: &[&str] = &[
        "rotate.activate.activate",
        "rotate.activate.sync_active_dir",
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
        fn workspace(&self) -> PathBuf {
            self.dir.path().join("t.key.rotation-work")
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
        /// Runs an ordinary command against this config -- the real
        /// recovery trigger, since every admin command opens the
        /// database through `open_checked` (or, for `rotate-key`
        /// itself, calls `recover_and_report` explicitly first).
        fn recover_with(&self, cfg: &Path) -> Output {
            self.run(
                &["admin", "unlock-user", "--username", "nobody", "--config"],
                cfg,
            )
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

    /// RFC 100 `0.94.0` P2 §4: every pre-activation point must leave
    /// the service recoverable on the OLD key -- an abort there never
    /// reached K08, so there is nothing to complete, only a leftover
    /// workspace recovery cleans. A kill is strictly less lossy than
    /// the power-loss model `rotate::tests` already enumerates for
    /// this same point, so this is a real-process confirmation of that
    /// finding, not a new claim.
    #[tokio::test]
    async fn abort_before_activation_recovers_to_the_old_key() {
        for point in PRE_ACTIVATION_POINTS {
            let h = Home::new();
            let cfg = h.config();
            h.create(&cfg).await;
            let spec = format!("{point}:abort");

            let out =
                h.run_with_failpoint(&rotate_args(&["admin", "rotate-key", "--yes"]), &cfg, &spec);
            assert!(
                out.status.code().is_none(),
                "abort at {point} must terminate by signal, not exit normally: {:?}",
                out.status
            );

            let recovered = h.recover_with(&cfg);
            assert_eq!(
                recovered.status.code(),
                Some(1),
                "at {point}: the follow-up command's own ordinary failure (no such user): {}",
                stderr(&recovered)
            );
            assert!(
                !stderr(&recovered).contains("refusing"),
                "at {point}: the service must recover, not refuse: {}",
                stderr(&recovered)
            );
            assert!(
                key_opens(&h.key(), &h.db()).await,
                "at {point}: the active key file must open the database (still the old key)"
            );
            assert!(
                !h.workspace().exists(),
                "at {point}: the leftover workspace must be cleaned"
            );
        }
    }

    /// RFC 100 `0.94.0` P2 §4: every activation-or-later point must
    /// leave the service recoverable on the NEW key -- an abort there
    /// reached K08 (durable, by this command's own contract), so
    /// completion is owed, not cleanup.
    #[tokio::test]
    async fn abort_at_or_after_activation_recovers_to_the_new_key() {
        for point in ACTIVATION_POINTS {
            let h = Home::new();
            let cfg = h.config();
            h.create(&cfg).await;
            let spec = format!("{point}:abort");

            let out =
                h.run_with_failpoint(&rotate_args(&["admin", "rotate-key", "--yes"]), &cfg, &spec);
            assert!(
                out.status.code().is_none(),
                "abort at {point} must terminate by signal, not exit normally: {:?}",
                out.status
            );

            let recovered = h.recover_with(&cfg);
            assert_eq!(
                recovered.status.code(),
                Some(1),
                "at {point}: the follow-up command's own ordinary failure (no such user): {}",
                stderr(&recovered)
            );
            assert!(
                !stderr(&recovered).contains("refusing"),
                "at {point}: the service must recover, not refuse: {}",
                stderr(&recovered)
            );
            assert!(
                key_opens(&h.key(), &h.db()).await,
                "at {point}: the active key file must open the database under the NEW key"
            );
            assert!(
                !h.workspace().exists(),
                "at {point}: the leftover workspace must be cleaned"
            );
        }
    }

    /// RFC 100 stage 2, P5: a real rotation probes the filesystem first,
    /// after preconditions pass and before anything else. A failure at
    /// any of its primitives must refuse before anything of the
    /// journalled protocol is ever created.
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
                h.run_with_failpoint(&rotate_args(&["admin", "rotate-key", "--yes"]), &cfg, &spec);
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
                !h.workspace().exists(),
                "at {point}: no workspace was created -- the probe runs first"
            );
            for probe_name in ["t.key.rotation-probe", "t.key.rotation-probe.2"] {
                assert!(
                    !h.dir.path().join(probe_name).exists(),
                    "at {point}: the probe file must not be left behind"
                );
            }
        }
    }
}

// ---------- RFC 100 `0.94.0` P2 (2026-11-24): a full rotation, end to
// end, and the custody choice's own usage errors, through the real
// binary ----------
mod live {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use sui_id_store::crypto::MasterKey;

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
        fn backup(&self) -> PathBuf {
            self.dir.path().join("t.key.pre-rotation")
        }
        fn workspace(&self) -> PathBuf {
            self.dir.path().join("t.key.rotation-work")
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
        /// Creates the database, bootstraps the key file, and seeds one
        /// active signing key directly (no HTTP wizard in this
        /// binary-only harness), so there is a real sealed column to
        /// prove the rotation actually re-keys.
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
        fn audit_actions(&self) -> Vec<String> {
            let conn = rusqlite::Connection::open(self.db()).expect("raw");
            conn.prepare("SELECT action FROM audit_log ORDER BY seq")
                .expect("prepare")
                .query_map([], |r| r.get::<_, String>(0))
                .expect("query")
                .map(|r| r.expect("row"))
                .collect()
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }

    /// A genuine decrypt attempt, not a byte comparison: does the key at
    /// `key_path` unseal the active signing key in the database at
    /// `db_path`?
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

    /// RFC 100 `0.94.0` P2: a full rotation, start to finish, through
    /// the real binary -- the dispatch's own "a full rotation's real
    /// stdout and stderr" and "the service starts on the new key, and
    /// the audit log holds `resealed`, then `rotated`, once each."
    #[tokio::test]
    async fn full_rotation_succeeds_end_to_end() {
        let h = Home::new();
        let cfg = h.config(18819);
        h.create(&cfg).await;
        let old_key_body = std::fs::read_to_string(h.key()).expect("old key");

        let out = h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--discard-backup-once-verified",
                "--verification-reference",
                "verified-by-test",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let err = stderr(&out);
        assert!(err.contains("Rotation complete"), "{err}");

        // The configured key file now holds a DIFFERENT key, and it
        // opens the database -- a real decrypt, not a byte comparison.
        let new_key_body = std::fs::read_to_string(h.key()).expect("new key");
        assert_ne!(new_key_body, old_key_body, "the key actually changed");
        assert!(
            key_opens(&h.key(), &h.db()).await,
            "the new key must open the database"
        );

        // The backup stays -- P3, not this stage, enforces the
        // disposition -- and the old key is the one archived there.
        assert!(h.backup().exists(), "the pre-rotation backup survives");
        assert_eq!(
            std::fs::read_to_string(h.backup()).expect("backup"),
            old_key_body,
            "the backup holds the old key"
        );

        // The workspace is cleaned.
        assert!(!h.workspace().exists());

        // The audit log holds `resealed`, then `rotated`, exactly once
        // each (M4: two distinct facts, never merged, never duplicated).
        let actions = h.audit_actions();
        assert_eq!(
            actions
                .iter()
                .filter(|a| a.as_str() == "admin.master_key.resealed")
                .count(),
            1,
            "{actions:?}"
        );
        assert_eq!(
            actions
                .iter()
                .filter(|a| a.as_str() == "admin.master_key.rotated")
                .count(),
            1,
            "{actions:?}"
        );
        let resealed_at = actions
            .iter()
            .position(|a| a == "admin.master_key.resealed")
            .expect("resealed present");
        let rotated_at = actions
            .iter()
            .position(|a| a == "admin.master_key.rotated")
            .expect("rotated present");
        assert!(
            resealed_at < rotated_at,
            "resealed must be recorded before rotated: {actions:?}"
        );

        // The service starts fine on the now-configured (new) key.
        let out2 = h.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &cfg,
        );
        assert_eq!(out2.status.code(), Some(1), "{}", stderr(&out2));
        assert!(!stderr(&out2).contains("refusing"), "{}", stderr(&out2));
    }

    /// `0.94.0` P2 review: a second rotation while the last one's backup is
    /// kept. The backup has one fixed name and is never replaced, so this
    /// must be refused **before anything is created**. Before the fix it
    /// failed part-way, at publishing the backup, and left a workspace and
    /// a `next` key behind. Every later start then refused, because the
    /// backup it found held the previous key, not the active one.
    #[tokio::test]
    async fn a_second_rotation_with_the_backup_still_kept_refuses_before_anything() {
        let h = Home::new();
        let cfg = h.config(18823);
        h.create(&cfg).await;
        let args = [
            "admin",
            "master-key",
            "rotate",
            "--yes",
            "--discard-backup-once-verified",
            "--verification-reference",
            "first",
            "--config",
        ];
        let first = h.run(&args, &cfg);
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        assert!(h.backup().exists());
        let key_after_first = std::fs::read_to_string(h.key()).expect("key");

        let second = h.run(&args, &cfg);
        assert_ne!(
            second.status.code(),
            Some(0),
            "the second rotation must refuse"
        );
        let err = stderr(&second);
        assert!(
            err.contains("the last rotation's old key is still kept at"),
            "{err}"
        );
        assert!(!err.contains("Rotation complete"), "{err}");
        assert!(!h.workspace().exists(), "no workspace was created");
        let next = std::path::PathBuf::from(format!("{}.next", h.key().display()));
        assert!(!next.exists(), "no next key was written");
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key"),
            key_after_first,
            "the active key is unchanged"
        );
        // The instance still opens: nothing was left for recovery to refuse.
        assert!(key_opens(&h.key(), &h.db()).await);
        let after = h.run(
            &["admin", "master-key", "rotate", "--check", "--config"],
            &cfg,
        );
        assert!(
            !stderr(&after).contains("refusing to open the database"),
            "{}",
            stderr(&after)
        );
    }

    /// RFC 100 `0.94.0` P2 §2/§4: missing, mixed, or a `review_at` in
    /// the past, each a usage error -- exit **2**, like clap's own.
    /// `--check` needs none of this (`check_mode`'s own tests already
    /// cover that it runs with no custody flags at all).
    #[tokio::test]
    async fn custody_choice_missing_is_a_usage_error() {
        let h = Home::new();
        let cfg = h.config(18820);
        h.create(&cfg).await;
        let before = std::fs::read_to_string(h.key()).expect("key");

        let out = h.run(
            &["admin", "master-key", "rotate", "--yes", "--config"],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(stderr(&out).contains("custody"), "{}", stderr(&out));
        assert_eq!(
            std::fs::read_to_string(h.key()).expect("key"),
            before,
            "nothing touched"
        );
        assert!(!h.workspace().exists());
    }

    #[tokio::test]
    async fn custody_choice_mixed_is_a_usage_error() {
        let h = Home::new();
        let cfg = h.config(18821);
        h.create(&cfg).await;

        let out = h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--retain-backup-until",
                "2099-01-01T00:00:00Z",
                "--custody-reference",
                "ops-team",
                "--discard-backup-once-verified",
                "--verification-reference",
                "verified",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(!h.workspace().exists());
    }

    #[tokio::test]
    async fn review_at_in_the_past_is_a_usage_error() {
        let h = Home::new();
        let cfg = h.config(18822);
        h.create(&cfg).await;

        let out = h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--retain-backup-until",
                "2020-01-01T00:00:00Z",
                "--custody-reference",
                "ops-team",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("not in the future"),
            "{}",
            stderr(&out)
        );
        assert!(!h.workspace().exists());
    }

    /// RFC 100 `0.94.0` P2 §4 mutation: "success printed before K05" --
    /// the real check is that nothing a probe-filesystem failure
    /// (which refuses before any of the journalled protocol exists)
    /// prints "Rotation complete" anywhere. A mutation that declares
    /// success unconditionally, before checking the rotation's own
    /// result, is caught here: it would print the banner even though
    /// this command exits non-zero.
    #[tokio::test]
    async fn a_failed_rotation_never_prints_rotation_complete() {
        let h = Home::new();
        let cfg = h.config(18823);
        h.create(&cfg).await;
        std::fs::write(
            h.dir.path().join("t.key.new"),
            "a leftover from a pre-100 rotation\n",
        )
        .expect("leftover");

        let out = h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--discard-backup-once-verified",
                "--verification-reference",
                "x",
                "--config",
            ],
            &cfg,
        );
        assert_ne!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            !stderr(&out).contains("Rotation complete"),
            "{}",
            stderr(&out)
        );
    }
}

// ---------- RFC 100 stage 4 (2026-11-21): startup recovery, through the
// real binary ----------
mod recovery {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use sui_id::key_adapter::{KeyFileOps, Real};
    use sui_id::rotate::journal::{self, JournalBody, JournalState};
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
            backup_disposition:
                sui_id_store::master_key_rotation::RotationDisposition::RemoveAfterVerified {
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
        // RFC 100 `0.94.0` P2 §1: `recover` now creates the permanent
        // liveness lock file, if absent, before classifying anything --
        // the one change a fail-closed refusal is still allowed to
        // make, since the file holds no information and is never
        // deleted. Everything else must be byte-identical.
        let mut after = home.snapshot();
        let lock_path = paths.lock.clone();
        match after.remove(&lock_path) {
            Some(body) => assert!(body.is_empty(), "the lock file must be empty"),
            None => panic!("recover must have created the (permanent) lock file"),
        }
        assert_eq!(
            after, before,
            "byte-identical after a fail-closed refusal, other than the new lock file"
        );
    }
}

// ---------- RFC 100 `0.94.0` P3 (2026-11-25): custody ----------
mod custody {
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
        fn backup(&self) -> PathBuf {
            self.dir.path().join("t.key.pre-rotation")
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
        /// Creates the database and bootstraps the key file (no HTTP
        /// wizard in this binary-only harness).
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
        fn audit_actions(&self) -> Vec<String> {
            let conn = rusqlite::Connection::open(self.db()).expect("raw");
            conn.prepare("SELECT action FROM audit_log ORDER BY seq")
                .expect("prepare")
                .query_map([], |r| r.get::<_, String>(0))
                .expect("query")
                .map(|r| r.expect("row"))
                .collect()
        }
        fn backdate_review_at(&self, when: chrono::DateTime<chrono::Utc>) {
            let conn = rusqlite::Connection::open(self.db()).expect("raw");
            conn.execute(
                "UPDATE master_key_rotation SET review_at = ?1",
                rusqlite::params![when],
            )
            .expect("backdate");
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }
    fn stdout(o: &Output) -> String {
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    fn rotate_remove_after_verified(h: &Home, cfg: &Path, reference: &str) -> Output {
        h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--discard-backup-once-verified",
                "--verification-reference",
                reference,
                "--config",
            ],
            cfg,
        )
    }
    fn rotate_retain_until(h: &Home, cfg: &Path, until: &str, custody_reference: &str) -> Output {
        h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--retain-backup-until",
                until,
                "--custody-reference",
                custody_reference,
                "--config",
            ],
            cfg,
        )
    }
    fn far_future() -> String {
        (chrono::Utc::now() + chrono::TimeDelta::days(90)).to_rfc3339()
    }

    /// RFC 100 `0.94.0` P7 §2: the old flag name is gone, with no
    /// alias -- clap's own usage error, exit 2, the same as any other
    /// unrecognized flag.
    #[test]
    fn the_old_flag_name_is_a_usage_error_not_an_alias() {
        let h = Home::new();
        let cfg = h.config(18930);
        h.create(&cfg);

        let out = h.run(
            &[
                "admin",
                "master-key",
                "rotate",
                "--yes",
                "--remove-backup-after-verified",
                "--verification-reference",
                "x",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("unexpected argument")
                || stderr(&out).contains("--remove-backup-after-verified"),
            "{}",
            stderr(&out)
        );
    }

    /// RFC 100 `0.94.0` P7 §2: the success message, per disposition,
    /// names the next step and that the next rotation refuses until
    /// it's taken.
    #[test]
    fn the_success_message_names_the_next_step_by_disposition() {
        let h1 = Home::new();
        let cfg1 = h1.config(18931);
        h1.create(&cfg1);
        let out1 = rotate_remove_after_verified(&h1, &cfg1, "checked-it");
        assert_eq!(out1.status.code(), Some(0), "{}", stderr(&out1));
        let err1 = stderr(&out1);
        assert!(
            err1.contains("sui-id admin master-key backup remove"),
            "{err1}"
        );
        assert!(err1.contains("the next rotation refuses"), "{err1}");

        let h2 = Home::new();
        let cfg2 = h2.config(18932);
        h2.create(&cfg2);
        let until = far_future();
        let out2 = rotate_retain_until(&h2, &cfg2, &until, "ops-team");
        assert_eq!(out2.status.code(), Some(0), "{}", stderr(&out2));
        let err2 = stderr(&out2);
        assert!(err2.contains("ops-team"), "{err2}");
        assert!(err2.contains("sui-id warns at every start"), "{err2}");
        assert!(err2.contains("the next rotation refuses"), "{err2}");
    }

    /// RFC 100 `0.94.0` P7 §2: the kept-backup refusal names both
    /// settling commands, not only one.
    #[test]
    fn the_kept_backup_refusal_names_both_settling_commands() {
        let h = Home::new();
        let cfg = h.config(18933);
        h.create(&cfg);
        let first = rotate_remove_after_verified(&h, &cfg, "first");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));

        let second = rotate_remove_after_verified(&h, &cfg, "second");
        assert_ne!(second.status.code(), Some(0));
        let err = stderr(&second);
        assert!(
            err.contains("sui-id admin master-key backup remove"),
            "{err}"
        );
        assert!(
            err.contains("sui-id admin master-key backup renew"),
            "{err}"
        );
    }

    /// RFC 100 `0.94.0` P3 §5: rotate, settle by removal, rotate again
    /// -- the second rotation must succeed, and each command's own
    /// audit event appears exactly once.
    #[test]
    fn rotate_settle_by_removal_then_rotate_again_succeeds() {
        let h = Home::new();
        let cfg = h.config(18901);
        h.create(&cfg);

        let first = rotate_remove_after_verified(&h, &cfg, "first-verified");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        assert!(h.backup().exists());

        let removed = h.run(
            &[
                "admin",
                "master-key",
                "backup",
                "remove",
                "--confirm-retired-or-reencrypted",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(removed.status.code(), Some(0), "{}", stderr(&removed));
        assert!(!h.backup().exists());

        let second = rotate_remove_after_verified(&h, &cfg, "second-verified");
        assert_eq!(second.status.code(), Some(0), "{}", stderr(&second));
        assert!(
            h.backup().exists(),
            "the second rotation's own fresh backup"
        );

        let actions = h.audit_actions();
        assert_eq!(
            actions
                .iter()
                .filter(|a| a.as_str() == "admin.master_key.backup_removed")
                .count(),
            1,
            "{actions:?}"
        );
        assert_eq!(
            actions
                .iter()
                .filter(|a| a.as_str() == "admin.master_key.rotated")
                .count(),
            2,
            "{actions:?}"
        );
    }

    /// RFC 100 `0.94.0` P3 §5: rotate, settle by moving the backup to
    /// its own custody location (no command needed for that path),
    /// rotate again -- succeeds, since nothing is left at the fixed
    /// backup path.
    #[test]
    fn rotate_settle_by_moving_the_file_away_then_rotate_again_succeeds() {
        let h = Home::new();
        let cfg = h.config(18902);
        h.create(&cfg);
        let until = far_future();

        let first = rotate_retain_until(&h, &cfg, &until, "ops-team");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        assert!(h.backup().exists());

        let elsewhere = h.dir.path().join("archived-elsewhere.key");
        std::fs::rename(h.backup(), &elsewhere).expect("move the backup to its own custody");

        let second = rotate_retain_until(&h, &cfg, &until, "ops-team");
        assert_eq!(second.status.code(), Some(0), "{}", stderr(&second));
        assert!(
            h.backup().exists(),
            "the second rotation's own fresh backup"
        );
    }

    /// Mutation coverage: without `--confirm-retired-or-reencrypted`,
    /// `backup remove` must refuse and change nothing.
    #[test]
    fn backup_remove_refuses_without_the_confirmation_flag_and_changes_nothing() {
        let h = Home::new();
        let cfg = h.config(18903);
        h.create(&cfg);
        let first = rotate_remove_after_verified(&h, &cfg, "first");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        let before = std::fs::read(h.backup()).expect("backup bytes");

        let out = h.run(
            &["admin", "master-key", "backup", "remove", "--config"],
            &cfg,
        );
        assert_ne!(
            out.status.code(),
            Some(0),
            "must refuse without the confirmation flag"
        );
        assert!(
            stderr(&out).contains("--confirm-retired-or-reencrypted"),
            "{}",
            stderr(&out)
        );
        assert!(h.backup().exists());
        assert_eq!(std::fs::read(h.backup()).expect("backup bytes"), before);
        assert!(
            h.audit_actions()
                .iter()
                .all(|a| a != "admin.master_key.backup_removed")
        );
    }

    /// A later `backup remove`, run against an already-absent backup,
    /// reports that and records nothing (RFC 100 `0.94.0` P3 §2).
    #[test]
    fn backup_remove_against_an_absent_file_reports_and_records_nothing() {
        let h = Home::new();
        let cfg = h.config(18904);
        h.create(&cfg);

        let out = h.run(
            &[
                "admin",
                "master-key",
                "backup",
                "remove",
                "--confirm-retired-or-reencrypted",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("Nothing to remove"),
            "{}",
            stderr(&out)
        );
        assert!(
            h.audit_actions()
                .iter()
                .all(|a| a != "admin.master_key.backup_removed")
        );
    }

    /// "No secure-erasure claim" (RFC 100 `0.94.0` P3 §2) is printed
    /// exactly once on a real removal.
    #[test]
    fn backup_remove_prints_the_no_secure_erasure_claim_exactly_once() {
        let h = Home::new();
        let cfg = h.config(18905);
        h.create(&cfg);
        let first = rotate_remove_after_verified(&h, &cfg, "first");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));

        let out = h.run(
            &[
                "admin",
                "master-key",
                "backup",
                "remove",
                "--confirm-retired-or-reencrypted",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let err = stderr(&out);
        assert_eq!(err.matches("No secure-erasure claim").count(), 1, "{err}");
    }

    /// `backup renew` refuses a past instant and changes nothing.
    #[test]
    fn backup_renew_refuses_a_past_instant_and_changes_nothing() {
        let h = Home::new();
        let cfg = h.config(18906);
        h.create(&cfg);
        let until = far_future();
        let first = rotate_retain_until(&h, &cfg, &until, "ops-team");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));

        let past = (chrono::Utc::now() - chrono::TimeDelta::days(1)).to_rfc3339();
        let out = h.run(
            &[
                "admin",
                "master-key",
                "backup",
                "renew",
                "--until",
                &past,
                "--custody-reference",
                "ops-team-2",
                "--config",
            ],
            &cfg,
        );
        assert_ne!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            h.audit_actions()
                .iter()
                .all(|a| a != "admin.master_key.backup_renewed")
        );
    }

    /// `backup renew` refuses a `RemoveAfterVerified` row -- it has no
    /// `review_at` of its own to move.
    #[test]
    fn backup_renew_refuses_for_remove_after_verified_and_changes_nothing() {
        let h = Home::new();
        let cfg = h.config(18907);
        h.create(&cfg);
        let first = rotate_remove_after_verified(&h, &cfg, "first");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));

        let until = far_future();
        let out = h.run(
            &[
                "admin",
                "master-key",
                "backup",
                "renew",
                "--until",
                &until,
                "--custody-reference",
                "ops-team",
                "--config",
            ],
            &cfg,
        );
        assert_ne!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            h.audit_actions()
                .iter()
                .all(|a| a != "admin.master_key.backup_renewed")
        );
    }

    /// A valid instant moves `review_at` forward and is audited.
    #[test]
    fn backup_renew_moves_review_at_forward_and_is_audited() {
        let h = Home::new();
        let cfg = h.config(18908);
        h.create(&cfg);
        let until = far_future();
        let first = rotate_retain_until(&h, &cfg, &until, "ops-team");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));

        let new_until = (chrono::Utc::now() + chrono::TimeDelta::days(200)).to_rfc3339();
        let out = h.run(
            &[
                "admin",
                "master-key",
                "backup",
                "renew",
                "--until",
                &new_until,
                "--custody-reference",
                "ops-team-2",
                "--config",
            ],
            &cfg,
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(
            h.audit_actions()
                .iter()
                .filter(|a| a.as_str() == "admin.master_key.backup_renewed")
                .count(),
            1
        );
    }

    /// RFC 100 `0.94.0` P3 §3: `rotate --check` reports an overdue
    /// `RetainUntil` backup beside its precondition lines.
    #[test]
    fn the_check_subcommand_reports_the_overdue_finding() {
        let h = Home::new();
        let cfg = h.config(18909);
        h.create(&cfg);
        let until = far_future();
        let first = rotate_retain_until(&h, &cfg, &until, "ops-team");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));

        h.backdate_review_at(chrono::Utc::now() - chrono::TimeDelta::days(1));

        let out = h.run(
            &["admin", "master-key", "rotate", "--check", "--config"],
            &cfg,
        );
        // A kept backup already fails precondition P3 regardless of
        // being overdue (P2's own fold-in) -- `--check` exits 1 for
        // that reason, but still reports the finding alongside it.
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        let text = stdout(&out);
        assert!(text.contains("past its review date"), "{text}");
        assert!(text.contains("ops-team"), "{text}");
    }

    /// RFC 100 `0.94.0` P3 §3: the same finding appears at every
    /// server start too, not only under `--check`.
    #[tokio::test]
    async fn the_server_itself_logs_the_overdue_finding_at_every_start() {
        use std::io::Read;
        use std::time::{Duration, Instant};

        let h = Home::new();
        let cfg = h.config(18910);
        h.create(&cfg);
        let until = far_future();
        let first = rotate_retain_until(&h, &cfg, &until, "ops-team");
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        h.backdate_review_at(chrono::Utc::now() - chrono::TimeDelta::days(1));

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
            if err_text.contains("past its review date") {
                assert!(err_text.contains("ops-team"), "{err_text}");
                let _ = child.kill();
                return;
            }
            // A hang guard, not a speed assertion (see o2a_cli_registry.rs).
            if start.elapsed() > Duration::from_secs(180) {
                let _ = child.kill();
                panic!("sui-id serve never logged the overdue finding: stderr={err_text}");
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
}

// ---------- RFC 100 `0.94.0` P7 (2026-11-25): a rotation must not run
// beside a running sui-id ----------
mod concurrency {
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{Duration, Instant};

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
        fn lock(&self) -> PathBuf {
            self.dir.path().join("t.key.rotation-lock")
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
        /// Every file under `dir` except the permanent lock file, with
        /// its contents, independent of `readdir` order -- the lock
        /// file is the one thing a running server is allowed to have
        /// created or touched by the time a rotation attempt refuses.
        fn snapshot_except_lock(&self) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
            let lock = self.lock();
            std::fs::read_dir(self.dir.path())
                .expect("read dir")
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| *p != lock)
                .map(|path| {
                    let body = std::fs::read(&path).expect("read");
                    (path, body)
                })
                .collect()
        }
    }

    fn stderr(o: &Output) -> String {
        String::from_utf8_lossy(&o.stderr).into_owned()
    }

    fn rotate_args(reference: &str) -> [&str; 8] {
        [
            "admin",
            "master-key",
            "rotate",
            "--yes",
            "--discard-backup-once-verified",
            "--verification-reference",
            reference,
            "--config",
        ]
    }

    fn drain(mut pipe: impl Read + Send + 'static) -> std::sync::Arc<std::sync::Mutex<Vec<u8>>> {
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

    /// Spawns `sui-id serve`, waits for it to report itself listening
    /// (its own stderr line, the real readiness signal -- not a sleep),
    /// and returns the child plus its drained stderr buffer.
    /// `std::process::Child` has no `Drop` that kills it -- a test
    /// that panics before its own explicit `child.kill()` would
    /// otherwise leave the real server running and the port bound,
    /// exactly what broke a subsequent run while this test was being
    /// written. This kills it unconditionally, panic or not.
    struct KillOnDrop(std::process::Child);

    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn spawn_listening_server(
        cfg: &Path,
    ) -> (KillOnDrop, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        let mut child = Command::new(BIN)
            .arg("serve")
            .arg("--config")
            .arg(cfg)
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
            // `tracing::info!`'s own line has no `eprintln!` companion
            // (unlike an operator-facing `WARN`), so it goes wherever
            // `tracing_subscriber::fmt()`'s default writer sends it --
            // stdout -- not necessarily stderr. Check both.
            let out_text = String::from_utf8_lossy(&out_buf.lock().expect("lock")).into_owned();
            let err_text = String::from_utf8_lossy(&err_buf.lock().expect("lock")).into_owned();
            if out_text.contains("sui-id listening") || err_text.contains("sui-id listening") {
                return (KillOnDrop(child), err_buf);
            }
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                panic!(
                    "sui-id serve never reported listening: stdout={out_text} stderr={err_text}"
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// RFC 100 `0.94.0` P7 §1: the dispatch's own central proof -- a
    /// rotation beside a running server refuses, changes nothing but
    /// the permanent lock file, and proceeds once the server stops.
    #[test]
    fn a_rotation_refuses_while_a_real_server_runs_and_changes_nothing() {
        let h = Home::new();
        let cfg = h.config(18920);
        h.create(&cfg);

        let (child, _err_buf) = spawn_listening_server(&cfg);
        // Taken only once the server is up: starting it (WAL/SHM
        // sidecar files, an opened connection) already changes the
        // directory on its own, before any rotation is even attempted
        // -- the property under test is what the *refused rotation*
        // changes, not what running the server changes.
        let before = h.snapshot_except_lock();

        let out = h.run(&rotate_args("first"), &cfg);
        assert_ne!(
            out.status.code(),
            Some(0),
            "a rotation must refuse while the server is running"
        );
        let err = stderr(&out);
        assert!(err.contains("sui-id is running"), "{err}");
        assert!(!err.contains("Rotation complete"), "{err}");
        // P7 review: the refusal is the first thing read, not something an
        // operator meets after the summary and a typed `yes`.
        assert!(!err.contains("About to rotate"), "{err}");

        let after = h.snapshot_except_lock();
        assert_eq!(
            after, before,
            "byte-identical apart from the lock file, which the snapshot excludes"
        );

        drop(child);

        // Once the server stops, the rotation proceeds.
        let out2 = h.run(&rotate_args("second"), &cfg);
        assert_eq!(out2.status.code(), Some(0), "{}", stderr(&out2));
    }

    /// RFC 100 `0.94.0` P7 §1: an ordinary command -- one that, like
    /// the server, only opens the database through `open_checked` --
    /// coexists with a running server. Both hold the lock shared.
    #[test]
    fn an_ordinary_command_coexists_with_a_running_server() {
        let h = Home::new();
        let cfg = h.config(18922);
        h.create(&cfg);

        let (_child, _err_buf) = spawn_listening_server(&cfg);

        let out = h.run(&["admin", "registration-token", "list", "--config"], &cfg);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
}

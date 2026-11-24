use super::*;
use crate::key_adapter::Real;
use crate::key_adapter::simulated::{RecordedPoint, Simulated, materialize};
use crate::rotate::journal::{self, JournalEnvelope, JournalState};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use sui_id_store::Database;
use sui_id_store::master_key_check::ensure_sentinel_written;

fn dup_key(key: &MasterKey) -> MasterKey {
    let mut bytes = *key.as_bytes();
    let copy = MasterKey::from_bytes(bytes);
    zeroize::Zeroize::zeroize(&mut bytes);
    copy
}

async fn sentinel_db(dir: &std::path::Path, key: &MasterKey) -> PathBuf {
    let db_path = dir.join("t.sqlite");
    let db = Database::open(&db_path, dup_key(key)).expect("open");
    ensure_sentinel_written(&db, db.key())
        .await
        .expect("sentinel");
    db_path
}

fn write_key_at(path: &std::path::Path, key: &MasterKey, mode: u32) {
    std::fs::write(path, format!("{}\n", key.to_base64())).expect("write key");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

/// A fully set-up deployment: an active key, a sentinel-sealed database
/// under it, and (unless the caller skips it) a locked, published
/// rotation workspace at `state`, with whatever outside-workspace
/// artifacts that state implies. `old_key`/`new_key`/`envelope` are
/// `None` only before any journal is written.
struct Fixture {
    _dir: tempfile::TempDir,
    key_path: PathBuf,
    db_path: PathBuf,
    paths: RotationPaths,
    old_key: MasterKey,
    new_key: MasterKey,
}

impl Fixture {
    async fn bare() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let old_key = MasterKey::generate();
        let new_key = MasterKey::generate();
        let db_path = sentinel_db(dir.path(), &old_key).await;
        let key_path = dir.path().join("t.key");
        write_key_at(&key_path, &old_key, 0o600);
        let paths = RotationPaths::for_key_file(&key_path);
        Self {
            _dir: dir,
            key_path,
            db_path,
            paths,
            old_key,
            new_key,
        }
    }

    /// Locks the workspace and publishes `intent.json` at `Intent`, then
    /// advances and writes whatever artifacts each state beyond that
    /// implies, up to and including `state`.
    fn prepare_to(&self, state: JournalState) -> JournalEnvelope {
        self.prepare_to_naming(state, None)
    }

    /// [`prepare_to`], with the journal naming `new_fingerprint` instead
    /// of the new key's own: a body a buggy writer could seal, since it
    /// holds both keys.
    fn prepare_to_naming(
        &self,
        state: JournalState,
        new_fingerprint: Option<String>,
    ) -> JournalEnvelope {
        journal::lock_workspace(&Real, &self.paths.workspace).expect("lock");
        let mut body = journal::sample_body(
            &self.key_path,
            &self.paths.workspace,
            &self.old_key,
            &self.new_key,
        );
        if let Some(fp) = new_fingerprint {
            body.new_fingerprint = fp;
        }
        let mut envelope = journal::write_intent(
            &Real,
            &self.paths.workspace,
            body,
            &self.old_key,
            &self.new_key,
        )
        .expect("write_intent");

        if state >= JournalState::NextTempVerified {
            write_key_at(&self.paths.next_temp, &self.new_key, 0o600);
            envelope = journal::advance(
                &Real,
                &self.paths.workspace,
                &envelope,
                JournalState::NextTempVerified,
                &self.old_key,
                &self.new_key,
            )
            .expect("advance to NextTempVerified");
        }
        if state >= JournalState::NextPublished {
            Real.publish_no_replace("test.next.publish", &self.paths.next_temp, &self.paths.next)
                .expect("publish next");
            envelope = journal::advance(
                &Real,
                &self.paths.workspace,
                &envelope,
                JournalState::NextPublished,
                &self.old_key,
                &self.new_key,
            )
            .expect("advance to NextPublished");
        }
        if state >= JournalState::BackupTempVerified {
            write_key_at(&self.paths.backup_temp, &self.old_key, 0o600);
            envelope = journal::advance(
                &Real,
                &self.paths.workspace,
                &envelope,
                JournalState::BackupTempVerified,
                &self.old_key,
                &self.new_key,
            )
            .expect("advance to BackupTempVerified");
        }
        if state >= JournalState::BackupPublished {
            Real.publish_no_replace(
                "test.backup.publish",
                &self.paths.backup_temp,
                &self.paths.backup,
            )
            .expect("publish backup");
            envelope = journal::advance(
                &Real,
                &self.paths.workspace,
                &envelope,
                JournalState::BackupPublished,
                &self.old_key,
                &self.new_key,
            )
            .expect("advance to BackupPublished");
        }
        if state >= JournalState::Prepared {
            envelope = journal::advance(
                &Real,
                &self.paths.workspace,
                &envelope,
                JournalState::Prepared,
                &self.old_key,
                &self.new_key,
            )
            .expect("advance to Prepared");
        }
        envelope
    }

    fn remaining_artifacts(&self) -> Vec<PathBuf> {
        [
            &self.paths.workspace,
            &self.paths.next,
            &self.paths.backup,
            &self.paths.next_temp,
            &self.paths.backup_temp,
            &self.paths.update_temp,
            &self.paths.intent_temp,
            &self.paths.workspace.join("intent.json"),
        ]
        .into_iter()
        .filter(|p| p.exists())
        .cloned()
        .collect()
    }
}

// ---------- Row 1 ----------

#[tokio::test]
async fn row1_no_workspace_recovers_to_nothing_and_simulated_records_no_operation() {
    let f = Fixture::bare().await;
    let sim = Simulated::new(&[(
        f.key_path.clone(),
        format!("{}\n", f.old_key.to_base64()).into_bytes(),
    )]);
    let result = recover(&sim, &f.key_path, &f.db_path).expect("recover");
    assert_eq!(result, Recovered::Nothing);
    assert!(sim.points().is_empty(), "no operation recorded");
}

// ---------- Row 2: no intent.json yet ----------

#[tokio::test]
async fn row2_cleans_every_subset_of_partial_intent_temporaries() {
    for files in [
        Vec::<&str>::new(),
        vec!["intent.write"],
        vec!["update.write"],
        vec!["intent.write", "update.write"],
    ] {
        let f = Fixture::bare().await;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&f.paths.workspace)
            .expect("mkdir");
        for name in &files {
            std::fs::write(f.paths.workspace.join(name), b"partial, unpublished").expect("write");
        }

        let result = recover(&Real, &f.key_path, &f.db_path).expect("recover");
        let Recovered::Cleaned { removed, .. } = result else {
            panic!("expected Cleaned for {files:?}")
        };
        assert!(!f.paths.workspace.exists(), "{files:?}");
        assert!(removed.contains(&f.paths.workspace), "{files:?}");
        for name in &files {
            assert!(
                removed.contains(&f.paths.workspace.join(name)),
                "{files:?}: {name} must be in removed"
            );
        }
        assert_eq!(
            removed.len(),
            files.len() + 1,
            "{files:?}: removed must be exact"
        );
    }
}

#[tokio::test]
async fn row2_an_unrecognized_file_fails_closed_and_changes_nothing() {
    let f = Fixture::bare().await;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&f.paths.workspace)
        .expect("mkdir");
    std::fs::write(f.paths.workspace.join("notes.txt"), b"hi").expect("write");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("unexpected entry"), "{err:#}");
    assert!(f.paths.workspace.join("notes.txt").exists());
}

#[tokio::test]
async fn row2_a_subdirectory_fails_closed() {
    let f = Fixture::bare().await;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&f.paths.workspace)
        .expect("mkdir");
    std::fs::write(f.paths.workspace.join("intent.write"), b"x").expect("write");
    std::fs::create_dir(f.paths.workspace.join("sub")).expect("subdir");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    // Refused by the plain-file check, which runs before any name check.
    assert!(
        format!("{err:#}").contains("symlink or a directory"),
        "{err:#}"
    );
    assert!(f.paths.workspace.join("sub").exists());
}

#[tokio::test]
async fn row2_a_symlink_named_intent_write_fails_closed() {
    let f = Fixture::bare().await;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&f.paths.workspace)
        .expect("mkdir");
    std::os::unix::fs::symlink(&f.key_path, f.paths.workspace.join("intent.write"))
        .expect("symlink");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert!(f.paths.workspace.join("intent.write").exists());
}

// ---------- The workspace check itself ----------

#[tokio::test]
async fn workspace_as_a_symlink_fails_closed() {
    let f = Fixture::bare().await;
    let elsewhere = f.db_path.parent().unwrap().join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, &f.paths.workspace).expect("symlink");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
}

#[tokio::test]
async fn workspace_with_the_wrong_mode_fails_closed() {
    let f = Fixture::bare().await;
    std::fs::DirBuilder::new()
        .mode(0o755)
        .create(&f.paths.workspace)
        .expect("mkdir");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("0700"), "{err:#}");
}

/// A test-only adapter reporting a fabricated uid for one path, so the
/// owner comparison is exercised without needing root to actually chown
/// anything (the dispatch's own escape: "a real file with another owner
/// needs root, and root is not asked for").
struct WrongOwner<'a> {
    inner: &'a dyn KeyFileOps,
    path: PathBuf,
    uid: u32,
}

impl KeyFileOps for WrongOwner<'_> {
    fn create_new(&self, p: &str, path: &std::path::Path) -> std::io::Result<()> {
        self.inner.create_new(p, path)
    }
    fn write_all(&self, p: &str, path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        self.inner.write_all(p, path, data)
    }
    fn sync_file(&self, p: &str, path: &std::path::Path) -> std::io::Result<()> {
        self.inner.sync_file(p, path)
    }
    fn rename(&self, p: &str, from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        self.inner.rename(p, from, to)
    }
    fn publish_no_replace(
        &self,
        p: &str,
        from: &std::path::Path,
        to: &std::path::Path,
    ) -> std::io::Result<()> {
        self.inner.publish_no_replace(p, from, to)
    }
    fn sync_dir(&self, p: &str, dir: &std::path::Path) -> std::io::Result<()> {
        self.inner.sync_dir(p, dir)
    }
    fn read(&self, p: &str, path: &std::path::Path) -> std::io::Result<Vec<u8>> {
        self.inner.read(p, path)
    }
    fn remove(&self, p: &str, path: &std::path::Path) -> std::io::Result<()> {
        self.inner.remove(p, path)
    }
    fn metadata(
        &self,
        p: &str,
        path: &std::path::Path,
    ) -> std::io::Result<crate::key_adapter::KeyFileMetadata> {
        let mut m = self.inner.metadata(p, path)?;
        if path == self.path {
            m.uid = self.uid;
        }
        Ok(m)
    }
    fn create_dir(&self, p: &str, path: &std::path::Path) -> std::io::Result<()> {
        self.inner.create_dir(p, path)
    }
    fn remove_dir(&self, p: &str, path: &std::path::Path) -> std::io::Result<()> {
        self.inner.remove_dir(p, path)
    }
    fn list_dir(&self, p: &str, dir: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
        self.inner.list_dir(p, dir)
    }
    fn db_committed(&self, label: &str) {
        self.inner.db_committed(label)
    }
    fn db_durable(&self, label: &str) {
        self.inner.db_durable(label)
    }
}

#[tokio::test]
async fn workspace_owned_by_another_uid_fails_closed() {
    let f = Fixture::bare().await;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&f.paths.workspace)
        .expect("mkdir");
    let real_uid = rustix::process::geteuid().as_raw();
    let faked = WrongOwner {
        inner: &Real,
        path: f.paths.workspace.clone(),
        uid: real_uid.wrapping_add(1),
    };

    let err = recover(&faked, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("not the uid"), "{err:#}");
    assert!(f.paths.workspace.exists());
}

// ---------- Rows 3-7: a journal in each state ----------

#[tokio::test]
async fn rows_3_to_7_each_clean_to_row_1_and_the_database_still_opens_with_the_active_key() {
    for state in [
        JournalState::Intent,
        JournalState::NextTempVerified,
        JournalState::NextPublished,
        JournalState::BackupTempVerified,
        JournalState::BackupPublished,
        JournalState::Prepared,
    ] {
        let f = Fixture::bare().await;
        f.prepare_to(state);

        let result = recover(&Real, &f.key_path, &f.db_path).expect("recover");
        assert!(
            matches!(result, Recovered::Cleaned { .. }),
            "state {state:?}: {result:?}"
        );
        let remaining = f.remaining_artifacts();
        assert!(
            remaining.is_empty(),
            "state {state:?}: left behind {remaining:?}"
        );

        let db = Database::open(&f.db_path, dup_key(&f.old_key))
            .unwrap_or_else(|e| panic!("state {state:?}: database must still open: {e}"));
        drop(db);
    }
}

#[tokio::test]
async fn a_backup_sealed_under_the_wrong_key_fails_closed_and_changes_nothing() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::BackupPublished);
    let before = f.remaining_artifacts();
    write_key_at(&f.paths.backup, &MasterKey::generate(), 0o600);

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("fingerprint"), "{err:#}");
    // Every artifact from before the tamper is still exactly there
    // (the tamper itself changed `backup`'s content, not its presence).
    for path in &before {
        assert!(path.exists(), "{}", path.display());
    }
}

#[tokio::test]
async fn a_next_key_sealed_under_the_wrong_key_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::NextPublished);
    write_key_at(&f.paths.next, &MasterKey::generate(), 0o600);

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("fingerprint"), "{err:#}");
    assert!(f.paths.workspace.exists());
}

#[tokio::test]
async fn a_next_key_that_is_a_symlink_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::NextPublished);
    std::fs::remove_file(&f.paths.next).expect("remove next");
    std::os::unix::fs::symlink(&f.key_path, &f.paths.next).expect("symlink");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert!(f.paths.workspace.exists());
}

// ---------- Binding ----------

#[tokio::test]
async fn a_journal_binding_a_different_workspace_fails_closed() {
    let f = Fixture::bare().await;
    journal::lock_workspace(&Real, &f.paths.workspace).expect("lock");
    // A body built for a *different* key file's paths, written into
    // *this* workspace -- sealed correctly, bound to somewhere else.
    let elsewhere = f.db_path.parent().unwrap().join("elsewhere.key");
    let other_workspace = crate::rotate::preconditions::workspace_path(&elsewhere);
    let wrong_body = journal::sample_body(&elsewhere, &other_workspace, &f.old_key, &f.new_key);
    journal::write_intent(
        &Real,
        &f.paths.workspace,
        wrong_body,
        &f.old_key,
        &f.new_key,
    )
    .expect("write_intent");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("does not match"), "{err:#}");
    assert!(f.paths.workspace.exists());
}

// ---------- Authentication ----------

#[tokio::test]
async fn a_tampered_journal_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Intent);
    let published = f.paths.workspace.join("intent.json");
    let mut bytes = std::fs::read(&published).expect("read");
    let pos = String::from_utf8_lossy(&bytes)
        .find("rotation_id")
        .expect("field present");
    bytes[pos] ^= 1;
    std::fs::write(&published, &bytes).expect("write tampered");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(f.paths.workspace.exists());
    let _ = err;
}

#[tokio::test]
async fn a_journal_sealed_under_keys_that_do_not_include_the_active_one_fails_closed() {
    let f = Fixture::bare().await;
    journal::lock_workspace(&Real, &f.paths.workspace).expect("lock");
    let unrelated_old = MasterKey::generate();
    let body = journal::sample_body(&f.key_path, &f.paths.workspace, &unrelated_old, &f.new_key);
    journal::write_intent(&Real, &f.paths.workspace, body, &unrelated_old, &f.new_key)
        .expect("write_intent");

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("neither role"), "{err:#}");
}

// ---------- Role New (stage 6's territory) ----------

#[tokio::test]
async fn role_new_with_the_active_key_fails_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The database's sentinel is already sealed under the *new* key --
    // as if K07/K08 had already committed and activation had already
    // swapped the key file -- and the active key file now holds that
    // same new key. Rows 8-10's territory (`0.94.0` P1) -- but no
    // `master_key_rotation` row exists here, one of P1's own named
    // fail-closed conditions, so this still fails closed, even though
    // rows 9/10 themselves are now recovered.
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &new_key).await;
    let key_path = dir.path().join("t.key");
    write_key_at(&key_path, &new_key, 0o600);
    let paths = RotationPaths::for_key_file(&key_path);

    journal::lock_workspace(&Real, &paths.workspace).expect("lock");
    let body = journal::sample_body(&key_path, &paths.workspace, &old_key, &new_key);
    journal::write_intent(&Real, &paths.workspace, body, &old_key, &new_key).expect("write_intent");

    let err = recover(&Real, &key_path, &db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("new"), "{err:#}");
    assert!(paths.workspace.exists());
}

// ---------- `--check` with a leftover workspace ----------

#[tokio::test]
async fn check_with_a_leftover_workspace_changes_nothing_in_simulated() {
    let f = Fixture::bare().await;
    let sim = Simulated::new(&[(
        f.key_path.clone(),
        format!("{}\n", f.old_key.to_base64()).into_bytes(),
    )]);
    sim.create_dir("test.workspace", &f.paths.workspace)
        .expect("create_dir");
    let before = sim.points().len();

    let (results, _resolved) = crate::rotate::preconditions::check(&sim, &f.key_path, &f.db_path);
    let p3 = results.iter().find(|r| r.id == "P3").expect("P3");
    assert!(p3.failure.is_some());
    assert_eq!(
        sim.points().len(),
        before,
        "P3's own check records no new operation"
    );
}

// ---------- Recovery survives its own crash ----------

/// Every point `lock_workspace` + `write_intent` records, materialized
/// into a real directory beside a real sentinel-sealed database, must
/// recover to row 1 -- the same property proven for `recover`'s own
/// cleanup below, but entered from *preparation*'s crash points instead
/// of a hand-built fixture.
#[tokio::test]
async fn every_preparation_point_recovers_to_row_1() {
    let base = tempfile::tempdir().expect("tempdir");
    let key_path = base.path().join("t.key");
    let workspace = base.path().join("t.key.rotation-work");
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();

    let sim = Simulated::new(&[(
        key_path.clone(),
        format!("{}\n", old_key.to_base64()).into_bytes(),
    )]);
    journal::lock_workspace(&sim, &workspace).expect("lock");
    let body = journal::sample_body(&key_path, &workspace, &old_key, &new_key);
    journal::write_intent(&sim, &workspace, body, &old_key, &new_key).expect("write_intent");

    let points = sim.points();
    check_every_point_recovers(&points, &old_key, base.path()).await;
}

/// Same property, for a full run through every state advance.
#[tokio::test]
async fn every_advance_point_recovers_to_row_1() {
    let base = tempfile::tempdir().expect("tempdir");
    let key_path = base.path().join("t.key");
    let workspace = base.path().join("t.key.rotation-work");
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();

    let sim = Simulated::new(&[(
        key_path.clone(),
        format!("{}\n", old_key.to_base64()).into_bytes(),
    )]);
    journal::lock_workspace(&sim, &workspace).expect("lock");
    let body = journal::sample_body(&key_path, &workspace, &old_key, &new_key);
    let intent =
        journal::write_intent(&sim, &workspace, body, &old_key, &new_key).expect("write_intent");
    let before = sim.points().len();
    journal::advance(
        &sim,
        &workspace,
        &intent,
        JournalState::NextTempVerified,
        &old_key,
        &new_key,
    )
    .expect("advance");

    let points = sim.points();
    check_every_point_recovers(&points[before..], &old_key, base.path()).await;
}

/// For every point in `points`, materialize both the power-loss states
/// and the kill state into `base` -- the same directory the journal's
/// own bound paths were built against, re-cleared before each one, since
/// a materialization into a *different* directory would never match
/// what the journal itself names -- beside a real sentinel-sealed
/// database under `old_key`. Confirms the real `recover` either lands at
/// row 1 with the database still opening, or -- never for a state
/// `lock_workspace`/`write_intent`/`advance` could leave -- fails
/// closed. A short write never applies here: recovery only removes.
async fn check_every_point_recovers(
    points: &[RecordedPoint],
    old_key: &MasterKey,
    base: &std::path::Path,
) {
    for point in points {
        let mut disks = point.power_loss_states();
        disks.push(point.kill_state());
        for disk in disks {
            if base.exists() {
                std::fs::remove_dir_all(base).expect("clear base directory");
            }
            std::fs::create_dir_all(base).expect("recreate base directory");
            materialize(&disk, base).expect("materialize");
            let key_path = base.join("t.key");
            let workspace = base.join("t.key.rotation-work");
            if !key_path.exists() {
                // This disk lost the active key file itself -- not a
                // state recovery is meant to repair; skip it the same
                // way a real crash before the key file ever existed
                // would never reach this code path at all.
                continue;
            }
            let db_path = sentinel_db(base, old_key).await;

            match recover(&Real, &key_path, &db_path) {
                Ok(_) => {
                    assert!(
                        !workspace.exists(),
                        "recovery must have removed the workspace"
                    );
                    let db = Database::open(&db_path, dup_key(old_key));
                    assert!(
                        db.is_ok(),
                        "the database must still open with the active key"
                    );
                }
                Err(e) => {
                    // A fail-closed outcome is only acceptable when this
                    // disk itself is not a state `lock_workspace`/
                    // `write_intent`/`advance` could leave -- which,
                    // entered directly from their own recorded points,
                    // should never happen. Report it loudly if it does.
                    panic!("recovery refused a state its own preparation could leave: {e:#}");
                }
            }
        }
    }
}

/// From every state, `Intent` through `Prepared`: `NextTempVerified` is
/// where `intent.json` and `next.write` coexist (so merging steps 7b and
/// 7c's syncs leaves a state row 2 refuses), and `BackupPublished` on is
/// where step 7a removes real published artifacts.
#[tokio::test]
async fn recovery_itself_survives_a_crash_mid_cleanup() {
    for state in [
        JournalState::Intent,
        JournalState::NextTempVerified,
        JournalState::NextPublished,
        JournalState::BackupTempVerified,
        JournalState::BackupPublished,
        JournalState::Prepared,
    ] {
        recovery_survives_a_crash_from(state).await;
    }
}

async fn recovery_survives_a_crash_from(state: JournalState) {
    let f = Fixture::bare().await;
    f.prepare_to(state);

    let sim = Simulated::new(&[(
        f.key_path.clone(),
        format!("{}\n", f.old_key.to_base64()).into_bytes(),
    )]);
    // Seed the real, prepared workspace and artifacts into the simulator,
    // then run `recover` through it to record its own crash points.
    for entry in [&f.paths.next, &f.paths.backup] {
        seed_from_real(&sim, entry);
    }
    seed_dir_from_real(&sim, &f.paths.workspace);

    // Only `recover`'s own points, not the seeding above (which also
    // records crash points -- seeding a disk is not what this test
    // means to crash-inject).
    let before = sim.points().len();
    let result = recover(&sim, &f.key_path, &f.db_path);
    assert!(
        matches!(result, Ok(Recovered::Cleaned { .. })),
        "{state:?}: {result:?}"
    );
    let points = sim.points();
    let points = &points[before..];
    assert!(!points.is_empty());

    // Re-materialize into the *same* directory the journal's own bound
    // paths were built against -- `f`'s own, re-cleared each time.
    let base = f._dir.path();
    for point in points {
        let mut disks = point.power_loss_states();
        disks.push(point.kill_state());
        for disk in disks {
            if base.exists() {
                std::fs::remove_dir_all(base).expect("clear base directory");
            }
            std::fs::create_dir_all(base).expect("recreate base directory");
            materialize(&disk, base).expect("materialize");
            let key_path = base.join("t.key");
            if !key_path.exists() {
                continue;
            }
            let db_path = sentinel_db(base, &f.old_key).await;
            if let Err(e) = recover(&Real, &key_path, &db_path) {
                panic!(
                    "{state:?}, {}: a re-run after a crash mid-cleanup must finish the job: {e:#}",
                    point.label
                );
            }
            for artifact in [&f.paths.next, &f.paths.backup] {
                assert!(!artifact.exists(), "{state:?}: {}", artifact.display());
            }
            let workspace = base.join("t.key.rotation-work");
            assert!(!workspace.exists());
            assert!(Database::open(&db_path, dup_key(&f.old_key)).is_ok());
        }
    }
}

fn seed_from_real(sim: &Simulated, path: &std::path::Path) {
    if !path.exists() || path.is_dir() {
        return;
    }
    let data = std::fs::read(path).expect("read real file");
    sim.create_new("seed.create_new", path)
        .expect("seed create_new");
    sim.write_all("seed.write", path, &data)
        .expect("seed write");
    sim.sync_file("seed.sync_file", path)
        .expect("seed sync_file");
    crate::keyring::sync_parent_dir_impl(sim, "seed.sync_dir", path).expect("seed sync_dir");
}

fn seed_dir_from_real(sim: &Simulated, workspace: &std::path::Path) {
    sim.create_dir("seed.workspace.create_dir", workspace)
        .expect("seed create_dir");
    crate::keyring::sync_parent_dir_impl(sim, "seed.workspace.sync_dir", workspace)
        .expect("seed sync_dir");
    for entry in std::fs::read_dir(workspace).expect("read dir") {
        let path = entry.expect("entry").path();
        seed_from_real(sim, &path);
        crate::keyring::sync_parent_dir_impl(sim, "seed.workspace.entry.sync_dir", &path)
            .expect("seed entry sync_dir");
    }
}

// ---------- review fold-ins (2026-11-21) ----------

/// Row 2 has no journal, so nothing binds a published artifact (M3). A
/// `next` or a `backup` beside such a workspace is not this rotation's,
/// and is never removed unverified: recovery refuses and keeps it.
#[tokio::test]
async fn row2_with_a_stray_next_or_backup_fails_closed_and_keeps_it() {
    for which in ["next", "backup"] {
        let f = Fixture::bare().await;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&f.paths.workspace)
            .expect("mkdir");
        std::fs::write(f.paths.workspace.join("intent.write"), b"partial").expect("write");
        let stray = if which == "next" {
            &f.paths.next
        } else {
            &f.paths.backup
        };
        write_key_at(stray, &f.new_key, 0o600);
        let before = std::fs::read(stray).expect("read stray");

        let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
        assert!(
            format!("{err:#}").contains("no rotation journal was ever published"),
            "{which}: {err:#}"
        );
        assert_eq!(
            std::fs::read(stray).expect("still there"),
            before,
            "{which}"
        );
        assert!(f.paths.workspace.join("intent.write").exists(), "{which}");
    }
}

/// A directory where a bound temporary should be is refused *before*
/// anything is removed: with `backup` and `next` published, a cleanup
/// that discovered it only at `remove` would already have deleted both.
#[tokio::test]
async fn a_directory_named_like_a_bound_temporary_fails_closed_before_any_removal() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::BackupPublished);
    std::fs::create_dir(&f.paths.next_temp).expect("a directory named next.write");
    let before = f.remaining_artifacts();

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(
        format!("{err:#}").contains("symlink or a directory"),
        "{err:#}"
    );
    assert_eq!(f.remaining_artifacts(), before, "nothing was removed");
    assert!(f.paths.next.exists() && f.paths.backup.exists());
}

/// The report names the command an operator types, not an alias.
#[test]
fn the_report_names_the_canonical_rotation_command() {
    let lines = report(&Recovered::Cleaned {
        removed: vec![PathBuf::from("/k.rotation-work")],
        active_fingerprint: "0123456789abcdef".to_owned(),
    });
    assert!(
        lines
            .last()
            .is_some_and(|l| l.contains("sui-id admin master-key rotate")),
        "{lines:?}"
    );
}

/// Every refusal reads as sentences: the detail, wherever it was worded,
/// starts with a capital.
#[tokio::test]
async fn a_refusal_detail_starts_a_sentence() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::BackupPublished);
    write_key_at(&f.paths.backup, &MasterKey::generate(), 0o600);
    let err = format!(
        "{:#}",
        recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed")
    );
    assert!(err.contains("check out. The pre-rotation backup"), "{err}");
    assert!(err.contains("it should be"), "{err}");
}

// ---------- Rows 8-10 (`0.94.0` P1): the database half already ran ----------
//
// Each fixture is a `Fixture::bare()` database sealed under `old_key`,
// fully prepared (`Prepared`, so `next`/`backup` are both published),
// with K08 simulated directly -- the store-side primitives themselves,
// since the live rotation does not call them yet (this stage's own
// scoping).
//
// `journal::sample_body`'s own `rotation_id` is a fixed constant; every
// row that must *match* the journal uses it verbatim, and only the
// mismatch test below uses a different one on purpose.

const SAMPLE_ROTATION_ID: &str = "11111111-1111-1111-1111-111111111111";

/// Simulate K08 having committed: reseal the sentinel to `new_key` and
/// write the `master_key_rotation` row `db_committed_pending_
/// activation`, through the real store command (`commands::
/// reseal_master_key_for_rotation`) -- not a raw write, so this fixture
/// is exactly what a real K08 run leaves.
async fn seed_db_committed(fixture: &Fixture, rotation_id: &str) {
    let db =
        Database::open(&fixture.db_path, dup_key(&fixture.old_key)).expect("open with old key");
    sui_id_store::commands::reseal_master_key_for_rotation(
        &db,
        &dup_key(&fixture.new_key),
        sui_id_store::master_key_rotation::RotationRecord {
            rotation_id: rotation_id.to_owned(),
            old_fingerprint: sui_id_store::crypto::fingerprint(&fixture.old_key),
            backup_filename: fixture
                .paths
                .backup
                .file_name()
                .and_then(|n| n.to_str())
                .expect("utf8")
                .to_owned(),
            disposition:
                sui_id_store::master_key_rotation::RotationDisposition::RemoveAfterVerified {
                    verification_reference: "test".into(),
                },
        },
    )
    .await
    .expect("simulate K08");
}

/// Simulate K05 having completed too (row 10), through the real store
/// command.
async fn mark_row_complete(fixture: &Fixture, rotation_id: &str) {
    let db =
        Database::open(&fixture.db_path, dup_key(&fixture.new_key)).expect("open with new key");
    sui_id_store::commands::complete_master_key_rotation(&db, rotation_id.to_owned())
        .await
        .expect("simulate K05")
        .expect("K05 writes something the first time");
}

#[tokio::test]
async fn row8_activates_the_next_key_and_needs_completion() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    // The active key file still holds `old_key` (`bare()`'s own setup):
    // K08 ran, but activation has not -- exactly row 8.

    let result = recover(&Real, &f.key_path, &f.db_path).expect("recover");
    match result {
        Recovered::NeedsCompletion {
            rotation_id,
            active_fingerprint,
            activated,
        } => {
            assert_eq!(rotation_id, SAMPLE_ROTATION_ID);
            assert_eq!(
                active_fingerprint,
                sui_id_store::crypto::fingerprint(&f.new_key)
            );
            assert!(activated, "row 8 activates");
        }
        other => panic!("expected NeedsCompletion, got {other:?}"),
    }
    let active_now = std::fs::read_to_string(&f.key_path).expect("read key file");
    assert_eq!(
        active_now.trim(),
        f.new_key.to_base64(),
        "the active key file now holds the new key"
    );
    assert!(!f.paths.next.exists(), "renamed into place, not copied");
    // K05 has not run yet -- nothing in the workspace is cleaned until
    // the caller does that.
    assert!(f.paths.workspace.exists());
    assert!(f.paths.backup.exists());
}

#[tokio::test]
async fn row9_needs_completion_without_activating_again() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    write_key_at(&f.key_path, &f.new_key, 0o600); // activation already done, in an earlier run

    let result = recover(&Real, &f.key_path, &f.db_path).expect("recover");
    match result {
        Recovered::NeedsCompletion {
            rotation_id,
            activated,
            ..
        } => {
            assert_eq!(rotation_id, SAMPLE_ROTATION_ID);
            assert!(!activated, "row 9: already activated before this run");
        }
        other => panic!("expected NeedsCompletion, got {other:?}"),
    }
    assert!(f.paths.next.exists(), "recover itself does not clean yet");
    assert!(f.paths.workspace.exists());
}

#[tokio::test]
async fn row10_only_cleans_the_stale_workspace() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    write_key_at(&f.key_path, &f.new_key, 0o600);
    mark_row_complete(&f, SAMPLE_ROTATION_ID).await;

    let result = recover(&Real, &f.key_path, &f.db_path).expect("recover");
    match result {
        Recovered::Cleaned {
            active_fingerprint, ..
        } => {
            assert_eq!(
                active_fingerprint,
                sui_id_store::crypto::fingerprint(&f.new_key)
            );
        }
        other => panic!("expected Cleaned, got {other:?}"),
    }
    assert!(
        f.remaining_artifacts().is_empty(),
        "everything cleaned: {:?}",
        f.remaining_artifacts()
    );
    // The service starts normally, against the new key, afterward.
    let db = Database::open(&f.db_path, dup_key(&f.new_key)).expect("the new key opens it");
    drop(db);
}

#[tokio::test]
async fn row8_without_next_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    std::fs::remove_file(&f.paths.next).expect("remove next");
    let before = f.remaining_artifacts();

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("does not open"), "{err:#}");
    assert_eq!(f.remaining_artifacts(), before, "nothing removed");
}

#[tokio::test]
async fn row8_a_next_of_the_wrong_key_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    write_key_at(&f.paths.next, &MasterKey::generate(), 0o600);
    let before = f.remaining_artifacts();

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("does not open"), "{err:#}");
    assert_eq!(f.remaining_artifacts(), before, "nothing removed");
}

#[tokio::test]
async fn row9_a_row_rotation_id_mismatch_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    // A different rotation entirely -- not the journal's own
    // `SAMPLE_ROTATION_ID`.
    seed_db_committed(&f, "22222222-2222-2222-2222-222222222222").await;
    write_key_at(&f.key_path, &f.new_key, 0o600);
    let before = f.remaining_artifacts();

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("different rotation"), "{err:#}");
    assert_eq!(f.remaining_artifacts(), before, "nothing removed");
}

/// Row 10's own inconsistent twin: the row says complete, but the
/// active key file is still the old one -- K05 only ever marks a row
/// complete after activation, so this should never happen; fail
/// closed rather than guess which fact to trust.
#[tokio::test]
async fn complete_row_with_the_active_key_still_old_fails_closed() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    mark_row_complete(&f, SAMPLE_ROTATION_ID).await;
    // The active key file was never swapped to `new_key`.

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(
        format!("{err:#}").contains("activation and completion disagree"),
        "{err:#}"
    );
}

/// Activation's own crash safety (row 8), in the power-loss mode: every
/// state the simulator enumerates during `recover`'s own run -- most
/// importantly the `rename` that activates `next` and its directory
/// sync. The real `recover`, re-run against the real adapter from each
/// materialized state, must reach `NeedsCompletion` (this stage's own
/// stopping point -- row 9's shape, since by then the file swap, at
/// least, already happened) and never fail closed on a state its own
/// activation could leave.
#[tokio::test]
async fn row8_activation_is_crash_safe_under_power_loss() {
    let f = Fixture::bare().await;
    f.prepare_to(JournalState::Prepared);
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;

    let sim = Simulated::new(&[(
        f.key_path.clone(),
        format!("{}\n", f.old_key.to_base64()).into_bytes(),
    )]);
    // Seed the real, prepared workspace and artifacts into the
    // simulator, then run `recover` through it to record its own
    // crash points -- the same shape `recovery_survives_a_crash_from`
    // already uses for rows 2-7.
    for entry in [&f.paths.next, &f.paths.backup] {
        seed_from_real(&sim, entry);
    }
    seed_dir_from_real(&sim, &f.paths.workspace);

    let before = sim.points().len();
    let result = recover(&sim, &f.key_path, &f.db_path);
    assert!(
        matches!(
            result,
            Ok(Recovered::NeedsCompletion {
                activated: true,
                ..
            })
        ),
        "{result:?}"
    );
    let points = sim.points();
    let points = &points[before..];
    assert!(!points.is_empty());

    let base = f._dir.path();
    // The database itself is real, not simulated (`seed_db_committed`
    // already wrote it, under the real adapter, and checkpointed it --
    // K08's own durability contract) -- only the key files and the
    // workspace move with each materialized disk. `materialize` clears
    // and repopulates `base` from the simulator's own tracked files
    // alone, which never included `t.sqlite`, so it is saved here and
    // restored after each one.
    let db_bytes = std::fs::read(&f.db_path).expect("read the real database file");
    for point in points {
        let mut disks = point.power_loss_states();
        disks.push(point.kill_state());
        for disk in disks {
            if base.exists() {
                std::fs::remove_dir_all(base).expect("clear base directory");
            }
            std::fs::create_dir_all(base).expect("recreate base directory");
            materialize(&disk, base).expect("materialize");
            std::fs::write(&f.db_path, &db_bytes).expect("restore the real database file");
            let key_path = base.join("t.key");
            // Unlike rows 2-7's own equivalent loop, a missing active
            // key file here is not skipped: `t.key` existed (holding
            // `old_key`) before this run started, so M2 ("no crash
            // prefix leaves the system without a key that decrypts
            // the database") applies squarely to it. An atomic
            // `rename` can never observe this; only remove-then-rename
            // could, which is exactly the regression this guards.
            assert!(
                key_path.exists(),
                "{}: the active key file must never go missing during activation",
                point.label
            );
            match recover(&Real, &key_path, &f.db_path) {
                Ok(Recovered::NeedsCompletion { .. }) => {}
                Ok(other) => panic!("{}: expected NeedsCompletion, got {other:?}", point.label),
                Err(e) => panic!(
                    "{}: must not fail closed on its own activation: {e:#}",
                    point.label
                ),
            }
        }
    }
}

// ---------- review fold-in (2026-11-24): M6, the fingerprint itself ----------

/// RFC 100 M6: a fingerprint mismatch fails closed. Here the journal
/// and the `master_key_rotation` row **agree** on a new fingerprint that
/// the bound `next` key does not have, though `next` is the key that
/// really opens the sentinel. Only recovery's own comparison of `next`'s
/// fingerprint against the journal's can refuse this: the row-versus-
/// journal check passes, because both were made to agree. That is the
/// state a buggy writer, or a tampered row, could leave.
#[tokio::test]
async fn row8_a_next_whose_fingerprint_the_journal_and_row_do_not_name_fails_closed() {
    let f = Fixture::bare().await;
    let claimed = sui_id_store::crypto::fingerprint(&MasterKey::generate());
    f.prepare_to_naming(JournalState::Prepared, Some(claimed.clone()));
    seed_db_committed(&f, SAMPLE_ROTATION_ID).await;
    {
        let conn = rusqlite::Connection::open(&f.db_path).expect("open for tampering");
        let n = conn
            .execute(
                "UPDATE master_key_rotation SET new_fingerprint = ?1 WHERE id = 1",
                [&claimed],
            )
            .expect("tamper the row");
        assert_eq!(n, 1);
    }
    let before = f.remaining_artifacts();

    let err = recover(&Real, &f.key_path, &f.db_path).expect_err("must fail closed");
    assert!(format!("{err:#}").contains("fingerprint"), "{err:#}");
    assert_eq!(
        f.remaining_artifacts(),
        before,
        "nothing removed or activated"
    );
}

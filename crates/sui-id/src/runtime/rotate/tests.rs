//! RFC 100 `0.94.0` P2 §4: `crash-matrix.md`'s preparation and
//! activation points (1-10), enumerated against the real [`rotate`]
//! through [`Simulated`] -- the same harness shape stage 0's own
//! (now-removed) D1-D4 used for the sequence this module replaces, with
//! three differences the new protocol needs: three database snapshots
//! instead of two (before K08, after K08, after K05 -- the row's own
//! state, not just the key, now matters), the required outcome is
//! "recovery reaches `OldReady` or `Complete`," not "M2 holds," and the
//! oracle runs the **real** [`recovery::recover`] against a
//! materialized disk, not a key-file byte scan.
//!
//! **Short write, termination, power loss** (`crash-matrix.md`'s three
//! failure modes): [`RecordedPoint::power_loss_states`] already
//! enumerates every torn-write prefix as one of its candidates (a short
//! write is one of the states a power loss can leave), and
//! [`RecordedPoint::kill_state`] is strictly less lossy than any
//! power-loss state at the same point -- the same equivalence D1's own
//! design relied on. Two computational models, three required modes.

use super::*;
use crate::key_adapter::Real;
use crate::key_adapter::simulated::{Disk, RecordedPoint, Simulated, materialize};
use crate::rotate::recovery::{self, Recovered};
use std::path::{Path, PathBuf};
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use sui_id_store::master_key_rotation::{RotationDisposition, RotationRecord};
use zeroize::Zeroize;

fn dup_key(key: &MasterKey) -> MasterKey {
    let mut bytes = *key.as_bytes();
    let copy = MasterKey::from_bytes(bytes);
    bytes.zeroize();
    copy
}

fn disposition() -> RotationDisposition {
    RotationDisposition::RemoveAfterVerified {
        verification_reference: "test-only".to_owned(),
    }
}

/// A fresh on-disk database with one active signing key and its
/// sentinel written, keyed under a fresh `old_key`.
async fn fresh_db() -> (tempfile::TempDir, Database, MasterKey, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let old_key = MasterKey::generate();
    let db_path = dir.path().join("t.sqlite");
    let db = Database::open(&db_path, dup_key(&old_key)).expect("open");
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
    sui_id_store::master_key_check::ensure_sentinel_written(&db, db.key())
        .await
        .expect("sentinel");
    (dir, db, old_key, db_path)
}

fn snapshot(db: &Database, dest: &Path) {
    let dest = dest.to_owned();
    db.with_conn_sync(move |conn| {
        conn.execute_batch(&format!("VACUUM INTO '{}'", dest.display()))?;
        Ok(())
    })
    .expect("vacuum into");
}

async fn audit_rotated_count(db_path: &Path, new_key: &MasterKey) -> usize {
    let db = Database::open(db_path, dup_key(new_key)).expect("open under new key");
    sui_id_store::repos::audit::recent(&db, 1000)
        .await
        .expect("audit tail")
        .into_iter()
        .filter(|r| r.action == "admin.master_key.rotated")
        .count()
}

fn key_opens_fingerprint(path: &Path, expected: &MasterKey) -> bool {
    let Ok(body) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(key) = MasterKey::from_base64(body.trim()) else {
        return false;
    };
    sui_id_store::crypto::fingerprint(&key) == sui_id_store::crypto::fingerprint(expected)
}

/// Drives the real [`rotate`] once through [`Simulated`], producing:
/// every recorded crash point, and the three database snapshots a
/// crash prefix selects between (`db_before`: nothing committed yet;
/// `db_after_k08`: the reseal committed, row pending; `db_after_k05`:
/// the row complete). `db_after_k08` is produced by replaying K08
/// directly on a byte-identical copy of `db_before`, with the exact
/// `rotation_id` [`rotate`] minted -- deterministic given identical
/// inputs, so this reproduces the in-band commit without needing a hook
/// into `rotate` itself.
struct Harness {
    _dir: tempfile::TempDir,
    _mid_dir: tempfile::TempDir,
    /// The one real directory every materialized crash state is
    /// written into, cleared between states (see [`materialize_state`]).
    /// **Not** a fresh `tempfile::tempdir()` per state: the journal
    /// [`rotate`] sealed binds `key_file`'s exact path string
    /// (`check_bound_paths`), so the real path `recovery::recover` is
    /// asked to check must be the *same* path [`rotate`] itself ran
    /// against -- a fresh, differently-named tempdir per state would
    /// make every single state refuse on a bound-path mismatch that
    /// has nothing to do with what is actually being tested.
    work_dir: tempfile::TempDir,
    key_file: PathBuf,
    old_key: MasterKey,
    new_key: MasterKey,
    points: Vec<RecordedPoint>,
    db_before: PathBuf,
    db_after_k08: PathBuf,
    db_after_k05: PathBuf,
}

impl Harness {
    async fn build() -> Self {
        let (dir, db, old_key, db_path) = fresh_db().await;
        let db_before = dir.path().join("before.sqlite");
        snapshot(&db, &db_before);

        let new_key = MasterKey::generate();
        let new_key_b64 = new_key.to_base64();
        let work_dir = tempfile::tempdir().expect("tempdir");
        let key_file = work_dir.path().join("t.key");
        let sim = Simulated::new(&[(
            key_file.clone(),
            format!("{}\n", old_key.to_base64()).into_bytes(),
        )]);

        let outcome = rotate(
            &sim,
            &db,
            &db_path,
            &key_file,
            &old_key,
            &new_key,
            &new_key_b64,
            disposition(),
            crate::database::PreMigrationBackup::Declined,
        )
        .await
        .map_err(|e| match e {
            RotateError::NotDurable { phase, source } => format!("{phase}: {source}"),
            RotateError::Other(e) => format!("{e:#}"),
        })
        .expect("rotate");
        sim.finish();

        let db_after_k05 = dir.path().join("after_k05.sqlite");
        snapshot(&db, &db_after_k05);

        let mid_dir = tempfile::tempdir().expect("tempdir");
        let mid_path = mid_dir.path().join("mid.sqlite");
        std::fs::copy(&db_before, &mid_path).expect("copy db_before for the K08 replica");
        let db_mid = Database::open(&mid_path, dup_key(&old_key)).expect("open mid");
        sui_id_store::commands::reseal_master_key_for_rotation(
            &db_mid,
            &new_key,
            RotationRecord {
                rotation_id: outcome.rotation_id.clone(),
                old_fingerprint: sui_id_store::crypto::fingerprint(&old_key),
                backup_filename: "t.key.pre-rotation".to_owned(),
                disposition: disposition(),
            },
        )
        .await
        .expect("K08 replica for the after-K08 snapshot");
        let db_after_k08 = mid_dir.path().join("after_k08.sqlite");
        snapshot(&db_mid, &db_after_k08);

        Harness {
            _dir: dir,
            _mid_dir: mid_dir,
            work_dir,
            key_file,
            old_key,
            new_key,
            points: sim.points(),
            db_before,
            db_after_k08,
            db_after_k05,
        }
    }

    fn db_snapshot_for(&self, d: &Disk) -> &Path {
        if d.has_marker("rotate.rotated") {
            &self.db_after_k05
        } else if d.has_marker("rotate.reseal") {
            &self.db_after_k08
        } else {
            &self.db_before
        }
    }

    /// Clears [`Self::work_dir`] and materializes `d` into it, alongside
    /// a copy of `db_snapshot` -- reusing one fixed real directory
    /// across every state (see [`Self::work_dir`]'s own doc for why).
    fn materialize_state(&self, d: &Disk, db_snapshot: &Path) -> PathBuf {
        clear_dir(self.work_dir.path());
        materialize(d, self.work_dir.path()).expect("materialize");
        let db_path = self.work_dir.path().join("t.sqlite");
        std::fs::copy(db_snapshot, &db_path).expect("copy db snapshot");
        db_path
    }
}

fn clear_dir(dir: &Path) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path).expect("remove dir");
        } else {
            std::fs::remove_file(&path).expect("remove file");
        }
    }
}

/// The required outcome for one materialized state, run against the
/// real [`recovery::recover`] -- then, for the two rows that need it,
/// exactly the completion `database::open_checked` itself performs
/// (K05, then the clean), before the final checks.
async fn assert_required_outcome(
    h: &Harness,
    label: &str,
    key_file: &Path,
    db_path: &Path,
    reseal_durable: bool,
) {
    let result = recovery::recover(&Real, key_file, db_path, false);
    let recovered = result.unwrap_or_else(|e| {
        panic!("{label}: recovery must never error on a state the rotation itself left: {e:#}")
    });

    if !reseal_durable {
        assert!(
            matches!(recovered, Recovered::Nothing | Recovered::Cleaned { .. }),
            "{label}: a pre-K08-durable prefix must reach OldReady, got {recovered:?}"
        );
        let paths = crate::rotate::preconditions::RotationPaths::for_key_file(key_file);
        assert!(
            !paths.workspace.exists(),
            "{label}: the workspace must be gone"
        );
        if key_file.exists() {
            assert!(
                key_opens_fingerprint(key_file, &h.old_key),
                "{label}: the active key file must still be the old key"
            );
        }
        return;
    }

    match recovered {
        Recovered::NeedsCompletion { rotation_id, .. } => {
            let db_new = Database::open(db_path, dup_key(&h.new_key)).expect("open under new key");
            sui_id_store::commands::complete_master_key_rotation(&db_new, rotation_id)
                .await
                .expect("K05");
            drop(db_new);
            let paths = crate::rotate::preconditions::RotationPaths::for_key_file(key_file);
            recovery::clean_workspace(&Real, &paths, false).expect("clean after completion");
        }
        // `Cleaned` (the workspace still existed, and is now gone) and
        // `Nothing` (the workspace was already gone in this exact
        // state -- a crash point inside the clean's own last step can
        // leave that) are both fine: either way nothing is left to
        // complete.
        Recovered::Cleaned { .. } | Recovered::Nothing => {}
    }

    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(key_file);
    assert!(
        !paths.workspace.exists(),
        "{label}: the workspace must be gone once the rotation is complete"
    );
    assert!(
        key_opens_fingerprint(key_file, &h.new_key),
        "{label}: the active key file must be the new key once the rotation is complete"
    );
    assert_eq!(
        audit_rotated_count(db_path, &h.new_key).await,
        1,
        "{label}: admin.master_key.rotated must be recorded exactly once"
    );
}

#[tokio::test]
async fn every_crash_matrix_point_reaches_the_required_outcome() {
    let h = Harness::build().await;
    let mut power_loss_states = 0usize;
    let mut kill_states = 0usize;

    for (i, p) in h.points.iter().enumerate() {
        for (j, d) in p.power_loss_states().into_iter().enumerate() {
            power_loss_states += 1;
            let db_snapshot = h.db_snapshot_for(&d).to_path_buf();
            let reseal_durable = d.has_marker("rotate.reseal");
            let db_path = h.materialize_state(&d, &db_snapshot);
            let label = format!("power-loss point {i} ({}) state {j}", p.label);
            assert_required_outcome(&h, &label, &h.key_file, &db_path, reseal_durable).await;
        }

        kill_states += 1;
        let d = p.kill_state();
        let db_snapshot = h.db_snapshot_for(&d).to_path_buf();
        let reseal_durable = d.has_marker("rotate.reseal");
        let db_path = h.materialize_state(&d, &db_snapshot);
        let label = format!("kill point {i} ({})", p.label);
        assert_required_outcome(&h, &label, &h.key_file, &db_path, reseal_durable).await;
    }

    eprintln!(
        "crash-matrix: {} crash point(s), {power_loss_states} power-loss state(s), \
         {kill_states} kill state(s), every one reaching its required outcome",
        h.points.len()
    );
}

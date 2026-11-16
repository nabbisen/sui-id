//! RFC 100 stage 0, §D: the harness's prototype, reproduced against the
//! real code. Each test drives the real [`rotate_key_files`] (or, for
//! the variants that must never exist in production, a test-only copy
//! of its sequence) through [`key_adapter::simulated::Simulated`], then
//! checks the design's two properties over every enumerated crash state,
//! against two genuine `VACUUM INTO` snapshots of a real database —
//! "does this key decrypt the active signing key" is a real decrypt
//! attempt, not a byte comparison.

use super::*;
use crate::key_adapter::KeyFileOps;
use crate::key_adapter::simulated::{Disk, RecordedPoint, Simulated};
use std::path::{Path, PathBuf};
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use zeroize::Zeroize;

fn dup_key(key: &MasterKey) -> MasterKey {
    let mut bytes = *key.as_bytes();
    let copy = MasterKey::from_bytes(bytes);
    bytes.zeroize();
    copy
}

/// A fresh on-disk database with one active signing key, so there is
/// something real to unseal. Returns the directory (kept alive for the
/// database file and its snapshots) and the database, keyed under a
/// fresh `old_key`.
async fn fresh_db() -> (tempfile::TempDir, Database, MasterKey) {
    let dir = tempfile::tempdir().expect("tempdir");
    let old_key = MasterKey::generate();
    let db = Database::open(&dir.path().join("t.sqlite"), dup_key(&old_key)).expect("open");
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
    (dir, db, old_key)
}

/// `VACUUM INTO` a real snapshot of `db` at `dest`.
fn snapshot(db: &Database, dest: &Path) {
    let dest = dest.to_owned();
    db.with_conn_sync(move |conn| {
        conn.execute_batch(&format!("VACUUM INTO '{}'", dest.display()))?;
        Ok(())
    })
    .expect("vacuum into");
}

/// Does `key_bytes` (a base64 key file's content, with or without a
/// trailing newline) decrypt the active signing key in the database at
/// `db_snapshot`? A real decrypt attempt, per the dispatch's §C.
async fn opens(key_bytes: &[u8], db_snapshot: &Path) -> bool {
    let Ok(s) = std::str::from_utf8(key_bytes) else {
        return false;
    };
    let Ok(key) = MasterKey::from_base64(s.trim()) else {
        return false;
    };
    let Ok(db) = Database::open(db_snapshot, dup_key(&key)) else {
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

/// M2: some file in `disk` holds the key `db_snapshot` is actually
/// sealed under.
async fn m2_holds(disk: &Disk, db_snapshot: &Path) -> bool {
    for path in disk.paths() {
        if let Some(bytes) = disk.read(path)
            && opens(bytes, db_snapshot).await
        {
            return true;
        }
    }
    false
}

/// The stronger property startup needs today, with no recovery: the
/// active path itself holds the key `db_snapshot` is sealed under.
async fn active_holds(disk: &Disk, old_path: &Path, db_snapshot: &Path) -> bool {
    match disk.read(old_path) {
        Some(bytes) => opens(bytes, db_snapshot).await,
        None => false,
    }
}

struct ModelCounts {
    crash_points: usize,
    states: usize,
    m2_violated: usize,
    wrong_after_success: usize,
}

/// Reproduces the prototype's `run`: every crash point's enumerated
/// states (power-loss: every subset of the pending directory ops and
/// writes; kill: the one state a kill leaves), checked against whichever
/// of the two snapshots this state's own `rotate.reseal` marker selects.
async fn count_violations(
    points: &[RecordedPoint],
    old_path: &Path,
    db_before: &Path,
    db_after: &Path,
    power_loss: bool,
) -> ModelCounts {
    let mut states = 0;
    let mut m2_violated = 0;
    let mut wrong_after_success = 0;
    let last = points.len().saturating_sub(1);
    for (i, p) in points.iter().enumerate() {
        let disks: Vec<Disk> = if power_loss {
            p.power_loss_states()
        } else {
            vec![p.kill_state()]
        };
        for d in disks {
            states += 1;
            let db_snap = if d.has_marker("rotate.reseal") {
                db_after
            } else {
                db_before
            };
            if !m2_holds(&d, db_snap).await {
                m2_violated += 1;
            }
            if i == last
                && (!active_holds(&d, old_path, db_snap).await || !d.has_marker("rotate.reseal"))
            {
                wrong_after_success += 1;
            }
        }
    }
    ModelCounts {
        crash_points: points.len(),
        states,
        m2_violated,
        wrong_after_success,
    }
}

/// Which of today's syncs (or the checkpoint) a test-only run skips —
/// `None` reproduces today's order exactly; every other variant is a
/// sequence that must never exist in production.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Skip {
    StageSyncFile,
    StageSyncDir,
    SyncDirAfterRename1,
    SyncDirAfterRename2,
    DbDurable,
    /// The `0.90.0` order: every sync and the checkpoint, all at once.
    All,
}

/// A **test-only copy** of [`rotate_key_files`]'s sequence (never in
/// production code — dispatch §D2), with one step optionally skipped, so
/// D2–D4 can show the harness catches dropping each one.
#[allow(clippy::too_many_arguments)]
async fn test_only_rotation_sequence(
    adapter: &dyn KeyFileOps,
    db: &Database,
    new_key: &MasterKey,
    old_path: &Path,
    staged_path: &Path,
    bak_path: &Path,
    new_key_b64: &str,
    skip: Skip,
) {
    let skip_all = skip == Skip::All;
    adapter
        .create_new("rotate.stage.create_new", staged_path)
        .expect("create_new");
    let mut data = new_key_b64.as_bytes().to_vec();
    data.push(b'\n');
    adapter
        .write_all("rotate.stage.write", staged_path, &data)
        .expect("write_all");
    if skip != Skip::StageSyncFile && !skip_all {
        adapter
            .sync_file("rotate.stage.sync_file", staged_path)
            .expect("sync_file");
    }
    if skip != Skip::StageSyncDir && !skip_all {
        adapter
            .sync_dir(
                "rotate.stage.sync_dir",
                staged_path.parent().unwrap_or(Path::new(".")),
            )
            .expect("sync_dir stage");
    }

    sui_id_store::commands::reseal_master_key(db, new_key)
        .await
        .expect("reseal");
    adapter.db_committed("rotate.reseal");
    if skip != Skip::DbDurable && !skip_all {
        adapter.db_durable("rotate.reseal");
    }

    adapter
        .rename("rotate.rename.1", old_path, bak_path)
        .expect("rename 1");
    if skip != Skip::SyncDirAfterRename1 && !skip_all {
        adapter
            .sync_dir(
                "rotate.sync_dir.after_rename.1",
                old_path.parent().unwrap_or(Path::new(".")),
            )
            .expect("sync_dir after rename 1");
    }
    adapter
        .rename("rotate.rename.2", staged_path, old_path)
        .expect("rename 2");
    if skip != Skip::SyncDirAfterRename2 && !skip_all {
        adapter
            .sync_dir(
                "rotate.sync_dir.after_rename.2",
                old_path.parent().unwrap_or(Path::new(".")),
            )
            .expect("sync_dir after rename 2");
    }
}

fn paths() -> (PathBuf, PathBuf, PathBuf) {
    (
        PathBuf::from("/virtual/t.key"),
        PathBuf::from("/virtual/t.key.new"),
        PathBuf::from("/virtual/t.key.bak.0"),
    )
}

/// D1 — today's order (stage 1 with A2), through the real
/// [`rotate_key_files`]: M2 holds in every state, in both models, and
/// the kill model's "after the last op" state is never wrong (the
/// power-loss model's isn't either — the properties this dispatch asks
/// for, not the prototype's exact counts, which the real code's finer
/// operations do not reproduce bit-for-bit).
#[tokio::test]
async fn todays_order_holds_m2_in_every_power_loss_and_kill_state() {
    let (_dir, db, old_key) = fresh_db().await;
    let (old_path, staged_path, bak_path) = paths();
    let new_key = MasterKey::generate();
    let new_key_b64 = new_key.to_base64();

    let old_body = format!("{}\n", old_key.to_base64());
    let sim = Simulated::new(&[(old_path.clone(), old_body.into_bytes())]);

    let before_dir = tempfile::tempdir().expect("tempdir");
    let db_before = before_dir.path().join("before.sqlite");
    let db_after = before_dir.path().join("after.sqlite");
    snapshot(&db, &db_before);

    rotate_key_files(
        &sim,
        &db,
        &new_key,
        &old_path,
        &staged_path,
        &bak_path,
        &new_key_b64,
    )
    .await
    .expect("rotate");
    snapshot(&db, &db_after);
    sim.finish();

    let points = sim.points();
    let power = count_violations(&points, &old_path, &db_before, &db_after, true).await;
    let kill = count_violations(&points, &old_path, &db_before, &db_after, false).await;

    eprintln!(
        "D1 today's order: power-loss {} crash points, {} states, M2 violated {}, wrong after success {}",
        power.crash_points, power.states, power.m2_violated, power.wrong_after_success
    );
    eprintln!(
        "D1 today's order: kill {} crash points, {} states, M2 violated {}, wrong after success {}",
        kill.crash_points, kill.states, kill.m2_violated, kill.wrong_after_success
    );

    assert_eq!(
        power.m2_violated, 0,
        "M2 must hold in every power-loss state"
    );
    assert_eq!(
        power.wrong_after_success, 0,
        "once success is reported, the active path must hold the key the database is sealed under"
    );
    assert_eq!(
        kill.m2_violated, 0,
        "a kill is strictly less lossy than a power loss"
    );
    assert_eq!(kill.wrong_after_success, 0);
}

/// D2 — remove each sync, one at a time (checkpoint kept): the
/// power-loss model must find at least one M2 or wrong-after-success
/// violation for each, except the directory sync after the first
/// rename, which the design's §2 already found not load-bearing.
#[tokio::test]
async fn dropping_any_sync_except_after_the_first_rename_is_caught() {
    for skip in [
        Skip::StageSyncFile,
        Skip::StageSyncDir,
        Skip::SyncDirAfterRename1,
        Skip::SyncDirAfterRename2,
    ] {
        let (_dir, db, old_key) = fresh_db().await;
        let (old_path, staged_path, bak_path) = paths();
        let new_key = MasterKey::generate();
        let new_key_b64 = new_key.to_base64();
        let old_body = format!("{}\n", old_key.to_base64());
        let sim = Simulated::new(&[(old_path.clone(), old_body.into_bytes())]);

        let dir = tempfile::tempdir().expect("tempdir");
        let db_before = dir.path().join("before.sqlite");
        let db_after = dir.path().join("after.sqlite");
        snapshot(&db, &db_before);

        test_only_rotation_sequence(
            &sim,
            &db,
            &new_key,
            &old_path,
            &staged_path,
            &bak_path,
            &new_key_b64,
            skip,
        )
        .await;
        snapshot(&db, &db_after);
        sim.finish();

        let points = sim.points();
        let power = count_violations(&points, &old_path, &db_before, &db_after, true).await;
        eprintln!(
            "D2 drop {skip:?}: {} crash points, {} states, M2 violated {}, wrong after success {}",
            power.crash_points, power.states, power.m2_violated, power.wrong_after_success
        );

        let found_a_violation = power.m2_violated > 0 || power.wrong_after_success > 0;
        if skip == Skip::SyncDirAfterRename1 {
            // The design's §2 measured this one as not load-bearing for
            // any property this checks. Record what is actually found,
            // per the dispatch -- if it also finds a violation, that is
            // new evidence, stated here rather than asserted around.
            eprintln!(
                "D2 drop {skip:?}: {}",
                if found_a_violation {
                    "NEW EVIDENCE -- this one found a violation too, contradicting the design's §2 measurement"
                } else {
                    "confirms the design's §2: not load-bearing for M2 or wrong-after-success"
                }
            );
        } else {
            assert!(
                found_a_violation,
                "dropping {skip:?} must be caught (M2 violated {}, wrong after success {})",
                power.m2_violated, power.wrong_after_success
            );
        }
    }
}

/// D3 — remove the `db_durable` step (A2's checkpoint): the power-loss
/// model must find the state where the reseal rolled back under the new
/// key, i.e. the active path holds the new key file but the database
/// snapshot a crash there would actually boot from is the un-resealed
/// one.
#[tokio::test]
async fn dropping_the_checkpoint_finds_the_rolled_back_reseal() {
    let (_dir, db, old_key) = fresh_db().await;
    let (old_path, staged_path, bak_path) = paths();
    let new_key = MasterKey::generate();
    let new_key_b64 = new_key.to_base64();
    let old_body = format!("{}\n", old_key.to_base64());
    let sim = Simulated::new(&[(old_path.clone(), old_body.into_bytes())]);

    let dir = tempfile::tempdir().expect("tempdir");
    let db_before = dir.path().join("before.sqlite");
    let db_after = dir.path().join("after.sqlite");
    snapshot(&db, &db_before);

    test_only_rotation_sequence(
        &sim,
        &db,
        &new_key,
        &old_path,
        &staged_path,
        &bak_path,
        &new_key_b64,
        Skip::DbDurable,
    )
    .await;
    snapshot(&db, &db_after);
    sim.finish();

    let points = sim.points();
    let power = count_violations(&points, &old_path, &db_before, &db_after, true).await;
    eprintln!(
        "D3 drop db_durable: {} crash points, {} states, M2 violated {}, wrong after success {}",
        power.crash_points, power.states, power.m2_violated, power.wrong_after_success
    );

    assert!(
        power.wrong_after_success > 0,
        "without the checkpoint, there must be a state where the active path holds the new key \
         but the reseal rolled back"
    );
}

/// D4 — the `0.90.0` order (no syncs, no checkpoint), as a test-only
/// sequence: the power-loss model must find violations, and the
/// kill-only model must find none — the blindness this harness exists
/// to close, reproduced against the real code, not the prototype's
/// abstract model.
#[tokio::test]
async fn the_0_90_0_order_is_safe_under_a_kill_and_unsafe_under_a_power_loss() {
    let (_dir, db, old_key) = fresh_db().await;
    let (old_path, staged_path, bak_path) = paths();
    let new_key = MasterKey::generate();
    let new_key_b64 = new_key.to_base64();
    let old_body = format!("{}\n", old_key.to_base64());
    let sim = Simulated::new(&[(old_path.clone(), old_body.into_bytes())]);

    let dir = tempfile::tempdir().expect("tempdir");
    let db_before = dir.path().join("before.sqlite");
    let db_after = dir.path().join("after.sqlite");
    snapshot(&db, &db_before);

    test_only_rotation_sequence(
        &sim,
        &db,
        &new_key,
        &old_path,
        &staged_path,
        &bak_path,
        &new_key_b64,
        Skip::All,
    )
    .await;
    snapshot(&db, &db_after);
    sim.finish();

    let points = sim.points();
    let power = count_violations(&points, &old_path, &db_before, &db_after, true).await;
    let kill = count_violations(&points, &old_path, &db_before, &db_after, false).await;
    eprintln!(
        "D4 (0.90.0 order) power-loss: {} crash points, {} states, M2 violated {}, wrong after success {}",
        power.crash_points, power.states, power.m2_violated, power.wrong_after_success
    );
    eprintln!(
        "D4 (0.90.0 order) kill: {} crash points, {} states, M2 violated {}, wrong after success {}",
        kill.crash_points, kill.states, kill.m2_violated, kill.wrong_after_success
    );

    assert!(
        power.m2_violated > 0 || power.wrong_after_success > 0,
        "the 0.90.0 order (no syncs at all) must be unsafe under a power loss"
    );
    assert_eq!(
        kill.m2_violated, 0,
        "a kill-only test must pass this order -- that is the blindness this harness exists to close"
    );
    assert_eq!(kill.wrong_after_success, 0);
}

/// F1's own test: under the `0.90.0` order, at least one violating
/// state must hold a **torn** key file at a path the oracle tried — not
/// merely "no file holds the right key" (which an untorn wrong key
/// already shows), but a file whose bytes are a non-empty, strict
/// prefix of the key that was being written, at a path [`m2_holds`] or
/// [`active_holds`] actually read. A write that persisted only
/// partially is exactly what [`RecordedPoint::power_loss_states`] adds
/// over the prototype's own model.
#[tokio::test]
async fn the_0_90_0_order_can_leave_a_torn_key_file_in_a_violating_state() {
    let (_dir, db, old_key) = fresh_db().await;
    let (old_path, staged_path, bak_path) = paths();
    let new_key = MasterKey::generate();
    let new_key_b64 = new_key.to_base64();
    let new_full = format!("{new_key_b64}\n").into_bytes();
    let old_body = format!("{}\n", old_key.to_base64());
    let sim = Simulated::new(&[(old_path.clone(), old_body.into_bytes())]);

    let dir = tempfile::tempdir().expect("tempdir");
    let db_before = dir.path().join("before.sqlite");
    let db_after = dir.path().join("after.sqlite");
    snapshot(&db, &db_before);

    test_only_rotation_sequence(
        &sim,
        &db,
        &new_key,
        &old_path,
        &staged_path,
        &bak_path,
        &new_key_b64,
        Skip::All,
    )
    .await;
    snapshot(&db, &db_after);
    sim.finish();

    let points = sim.points();
    let last = points.len().saturating_sub(1);
    let mut found_torn_violation = false;
    for (i, p) in points.iter().enumerate() {
        for d in p.power_loss_states() {
            let db_snap = if d.has_marker("rotate.reseal") {
                &db_after
            } else {
                &db_before
            };
            let m2 = m2_holds(&d, db_snap).await;
            let wrong_after_success = i == last
                && (!active_holds(&d, &old_path, db_snap).await || !d.has_marker("rotate.reseal"));
            if m2 && !wrong_after_success {
                continue;
            }
            for path in d.paths() {
                if let Some(bytes) = d.read(path)
                    && !bytes.is_empty()
                    && bytes.len() < new_full.len()
                    && new_full.starts_with(bytes)
                {
                    found_torn_violation = true;
                }
            }
        }
    }

    assert!(
        found_torn_violation,
        "the 0.90.0 order must be able to leave a torn key file in a violating state"
    );
}

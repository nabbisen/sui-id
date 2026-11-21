use super::*;
use crate::key_adapter::Real;
use crate::key_adapter::simulated::Simulated;
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use sui_id_store::master_key_check::ensure_sentinel_written;

fn dup_key(key: &MasterKey) -> MasterKey {
    let mut bytes = *key.as_bytes();
    let copy = MasterKey::from_bytes(bytes);
    zeroize::Zeroize::zeroize(&mut bytes);
    copy
}

/// A real, on-disk database with its sentinel already written under
/// `key` -- the "healthy, established deployment" state every
/// precondition test but P1/P2's own failures starts from.
async fn sentinel_db(dir: &std::path::Path, key: &MasterKey) -> std::path::PathBuf {
    let db_path = dir.join("t.sqlite");
    let db = Database::open(&db_path, dup_key(key)).expect("open");
    ensure_sentinel_written(&db, db.key())
        .await
        .expect("write sentinel");
    db_path
}

fn key_file(dir: &std::path::Path, key: &MasterKey) -> std::path::PathBuf {
    let path = dir.join("t.key");
    std::fs::write(&path, format!("{}\n", key.to_base64())).expect("write key file");
    path
}

// P1's own refusal (the key came from `SUI_ID_MASTER_KEY`) needs that
// variable set on the *process* `check` reads -- never done to this test
// binary's own process (it would leak into every other test here); see
// `tests/e2e/key_rotation.rs`'s `check_mode::refuses_when_the_key_came_
// from_the_environment`, which sets it on a child process instead.

// ---------- Each of P2-P4, refusing before any file exists ----------

#[tokio::test]
async fn p2_refuses_when_there_is_no_existing_key_to_rotate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let key_path = dir.path().join("t.key"); // never created; env unset

    let sim = Simulated::new(&[]);
    let (results, resolved) = check(&sim, &key_path, &db_path);

    let p2 = results.iter().find(|r| r.id == "P2").expect("P2");
    assert!(p2.failure.is_some(), "P2 must refuse: {:?}", p2.failure);
    assert!(
        p2.failure.as_ref().unwrap().contains("no master key found"),
        "{:?}",
        p2.failure
    );
    assert!(
        sim.points().is_empty(),
        "checking must not mint a key for a system with none yet"
    );
    assert!(resolved.is_none());
}

#[tokio::test]
async fn p2_refuses_when_the_key_file_holds_the_wrong_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let wrong = MasterKey::generate();
    let key_path = key_file(dir.path(), &wrong);

    let sim = Simulated::new(&[(
        key_path.clone(),
        format!("{}\n", wrong.to_base64()).into_bytes(),
    )]);
    let (results, resolved) = check(&sim, &key_path, &db_path);

    let p2 = results.iter().find(|r| r.id == "P2").expect("P2");
    assert!(
        p2.failure.is_some(),
        "P2 must refuse a key that does not open the sentinel"
    );
    assert!(sim.points().is_empty(), "no operation recorded");
    assert!(resolved.is_none());
}

#[tokio::test]
async fn p3_refuses_when_a_staged_key_is_already_present() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let key_path = key_file(dir.path(), &key);
    let staged = staged_path(&key_path);

    let sim = Simulated::new(&[
        (
            key_path.clone(),
            format!("{}\n", key.to_base64()).into_bytes(),
        ),
        (staged.clone(), b"leftover".to_vec()),
    ]);
    let (results, resolved) = check(&sim, &key_path, &db_path);

    let p3 = results.iter().find(|r| r.id == "P3").expect("P3");
    let failure = p3.failure.as_ref().expect("P3 must refuse");
    assert!(failure.contains(&staged.display().to_string()), "{failure}");
    assert!(sim.points().is_empty(), "no operation recorded");
    // P1, P2 and P4 pass here -- P3 alone fails -- so a real rotation
    // would still have a key to reuse; `check` only withholds it when
    // *something* failed.
    assert!(resolved.is_none(), "withheld because P3 failed overall");
}

#[tokio::test]
async fn p3_refuses_when_a_rotation_workspace_is_already_present() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let key_path = key_file(dir.path(), &key);
    let workspace = workspace_path(&key_path);

    let sim = Simulated::new(&[
        (
            key_path.clone(),
            format!("{}\n", key.to_base64()).into_bytes(),
        ),
        (workspace.clone(), b"leftover journal".to_vec()),
    ]);
    let (results, _resolved) = check(&sim, &key_path, &db_path);

    let p3 = results.iter().find(|r| r.id == "P3").expect("P3");
    let failure = p3.failure.as_ref().expect("P3 must refuse");
    assert!(
        failure.contains(&workspace.display().to_string()),
        "{failure}"
    );
    assert!(sim.points().is_empty());
}

// `Simulated` has no symlink model (always `is_symlink: false`), so P4's
// refusal is tested through the real adapter below, alongside P3+P4
// failing together.

// ---------- `check` records no operation, pass or fail ----------

#[tokio::test]
async fn check_records_no_operation_when_everything_passes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let key_path = key_file(dir.path(), &key);

    let sim = Simulated::new(&[(
        key_path.clone(),
        format!("{}\n", key.to_base64()).into_bytes(),
    )]);
    let (results, resolved) = check(&sim, &key_path, &db_path);

    let failures: Vec<_> = results.iter().map(|r| (r.id, &r.failure)).collect();
    assert!(results.iter().all(|r| r.passed()), "{failures:?}");
    assert!(
        resolved.is_some(),
        "all preconditions passed -- the key is handed back"
    );
    assert!(sim.points().is_empty(), "a passing check creates nothing");
}

#[tokio::test]
async fn check_records_no_operation_when_several_preconditions_fail_at_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let wrong = MasterKey::generate();
    let key_path = key_file(dir.path(), &wrong);
    let staged = staged_path(&key_path);

    let sim = Simulated::new(&[
        (
            key_path.clone(),
            format!("{}\n", wrong.to_base64()).into_bytes(),
        ),
        (staged, b"leftover".to_vec()),
    ]);
    let (results, resolved) = check(&sim, &key_path, &db_path);

    let p2 = results.iter().find(|r| r.id == "P2").expect("P2");
    let p3 = results.iter().find(|r| r.id == "P3").expect("P3");
    assert!(p2.failure.is_some(), "P2 and P3 fail together");
    assert!(p3.failure.is_some(), "P2 and P3 fail together");
    assert!(resolved.is_none());
    assert!(
        sim.points().is_empty(),
        "a failing check creates nothing either"
    );
}

// ---------- Through the real filesystem: P4, and P3+P4 together ----------

#[tokio::test]
async fn p4_refuses_a_symlinked_key_file_naming_the_link_and_its_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let real_key_path = dir.path().join("t.key.real");
    std::fs::write(&real_key_path, format!("{}\n", key.to_base64())).expect("write real key");
    let key_path = dir.path().join("t.key");
    std::os::unix::fs::symlink(&real_key_path, &key_path).expect("symlink");

    let (results, resolved) = check(&Real, &key_path, &db_path);

    let p4 = results.iter().find(|r| r.id == "P4").expect("P4");
    let failure = p4.failure.as_ref().expect("P4 must refuse a symlink");
    assert!(
        failure.contains(&key_path.display().to_string()),
        "{failure}"
    );
    assert!(
        failure.contains(&real_key_path.display().to_string()),
        "the refusal names the link's target too: {failure}"
    );
    assert!(resolved.is_none(), "withheld because P4 failed overall");
}

#[tokio::test]
async fn p3_and_p4_failing_together_are_both_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = MasterKey::generate();
    let db_path = sentinel_db(dir.path(), &key).await;
    let real_key_path = dir.path().join("t.key.real");
    std::fs::write(&real_key_path, format!("{}\n", key.to_base64())).expect("write real key");
    let key_path = dir.path().join("t.key");
    std::os::unix::fs::symlink(&real_key_path, &key_path).expect("symlink");
    let staged = staged_path(&key_path);
    std::fs::write(&staged, b"leftover").expect("write leftover");

    let (results, resolved) = check(&Real, &key_path, &db_path);

    let p1 = results.iter().find(|r| r.id == "P1").expect("P1");
    let p2 = results.iter().find(|r| r.id == "P2").expect("P2");
    let p3 = results.iter().find(|r| r.id == "P3").expect("P3");
    let p4 = results.iter().find(|r| r.id == "P4").expect("P4");
    assert!(p1.passed(), "{:?}", p1.failure);
    assert!(p2.passed(), "{:?}", p2.failure);
    assert!(p3.failure.is_some(), "P3 must fail");
    assert!(p4.failure.is_some(), "P4 must fail");
    assert!(resolved.is_none());
}

// ── P5's probe and files it did not create (review fold-in, 0.93.0 O3) ─────

#[tokio::test]
async fn a_stale_probe_holding_the_marker_is_removed_and_the_probe_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key_path = dir.path().join("t.key");
    // What an interrupted probe leaves: the marker, exactly as written.
    std::fs::write(probe_path(&key_path), probe_file_bytes()).expect("stale probe");

    probe_filesystem(&Real, &key_path)
        .await
        .expect("a stale probe of our own is removed, and the probe then succeeds");

    assert!(!probe_path(&key_path).exists());
    assert!(!probe_path_renamed(&key_path).exists());
}

#[tokio::test]
async fn a_foreign_file_at_a_probe_name_is_refused_and_left_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key_path = dir.path().join("t.key");
    for name in [probe_path(&key_path), probe_path_renamed(&key_path)] {
        std::fs::write(&name, b"an operator's own file").expect("foreign file");

        let err = probe_filesystem(&Real, &key_path)
            .await
            .expect_err("a file that is not a probe must refuse");
        let message = format!("{err:#}");
        assert!(message.contains(&name.display().to_string()), "{message}");
        assert!(
            !message.contains("does not support"),
            "a foreign file is not a filesystem limitation: {message}"
        );
        assert_eq!(
            std::fs::read(&name).expect("still there"),
            b"an operator's own file",
            "a file the probe did not create is never removed or changed"
        );
        std::fs::remove_file(&name).expect("clean up for the next name");
    }
}

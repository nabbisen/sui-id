use super::*;
use anyhow::Context;

#[test]
fn real_metadata_refuses_a_missing_path() {
    assert!(Real.metadata("x", Path::new("/does/not/exist")).is_err());
}

// ── RFC 100 `0.94.0` P2 §1: the liveness lock ───────────────────────────────

#[test]
fn real_lock_exclusive_non_blocking_refuses_while_another_open_holds_it() {
    // Two separate `open`s of the same path, even in one process, are two
    // distinct holders for `flock(2)` -- the same fact that lets the
    // kernel release the lock when a process exits without this guard's
    // own `Drop` ever running (e.g. `abort()`). This is what makes a
    // single-process test of contention meaningful at all.
    let dir = tempfile::tempdir().expect("tempdir");
    let lock_path = dir.path().join("t.key.rotation-lock");

    let first = Real
        .lock_exclusive("first", &lock_path, false)
        .expect("the first holder takes it");
    let second_err = Real
        .lock_exclusive("second", &lock_path, false)
        .err()
        .expect("a second non-blocking attempt must refuse");
    assert_eq!(second_err.kind(), std::io::ErrorKind::WouldBlock);

    drop(first);
    Real.lock_exclusive("third", &lock_path, false)
        .expect("released: a later attempt succeeds");
}

#[test]
fn simulated_lock_exclusive_models_the_same_contention() {
    use simulated::Simulated;
    let path = std::path::PathBuf::from("/virtual/t.key.rotation-lock");
    let sim = Simulated::new(&[]);

    let first = sim
        .lock_exclusive("first", &path, false)
        .expect("the first holder takes it");
    let second_err = sim
        .lock_exclusive("second", &path, false)
        .err()
        .expect("a second non-blocking attempt must refuse");
    assert_eq!(second_err.kind(), std::io::ErrorKind::WouldBlock);

    drop(first);
    sim.lock_exclusive("third", &path, false)
        .expect("released: a later attempt succeeds");
}

// ── RFC 100 `0.94.0` P7 §1: shared mode -- a server (or any other CLI
// command that opens the database) holds it shared for its whole life;
// a rotation's own exclusive attempt refuses against any shared holder,
// any number of shared holders coexist, and a shared attempt refuses
// only against an exclusive holder. ──────────────────────────────────

#[test]
fn real_two_shared_holders_coexist() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock_path = dir.path().join("t.key.rotation-lock");

    let first = Real
        .lock_shared("first", &lock_path, false)
        .expect("the first shared holder takes it");
    let second = Real
        .lock_shared("second", &lock_path, false)
        .expect("a second shared holder coexists with the first");
    drop(first);
    drop(second);
}

#[test]
fn real_lock_exclusive_refuses_while_a_shared_holder_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock_path = dir.path().join("t.key.rotation-lock");

    let shared = Real
        .lock_shared("open_checked", &lock_path, false)
        .expect("a server (or any open_checked caller) holds it shared");
    let err = Real
        .lock_exclusive("rotate", &lock_path, false)
        .err()
        .expect("a rotation must refuse while sui-id has the database open");
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);

    drop(shared);
    Real.lock_exclusive("rotate-again", &lock_path, false)
        .expect("released: the rotation now proceeds");
}

#[test]
fn real_lock_shared_refuses_only_while_an_exclusive_holder_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock_path = dir.path().join("t.key.rotation-lock");

    let exclusive = Real
        .lock_exclusive("rotate", &lock_path, false)
        .expect("a rotation takes it exclusive");
    let err = Real
        .lock_shared("open_checked", &lock_path, false)
        .err()
        .expect("starting sui-id while a rotation is running must refuse");
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);

    drop(exclusive);
    Real.lock_shared("open_checked-again", &lock_path, false)
        .expect("released: opening the database now proceeds");
}

#[test]
fn simulated_shared_and_exclusive_model_the_same_rules() {
    use simulated::Simulated;
    let path = std::path::PathBuf::from("/virtual/t.key.rotation-lock");
    let sim = Simulated::new(&[]);

    let first_shared = sim
        .lock_shared("first", &path, false)
        .expect("a shared holder takes it");
    let second_shared = sim
        .lock_shared("second", &path, false)
        .expect("a second shared holder coexists");
    let exclusive_err = sim
        .lock_exclusive("rotate", &path, false)
        .err()
        .expect("exclusive must refuse against any shared holder");
    assert_eq!(exclusive_err.kind(), std::io::ErrorKind::WouldBlock);

    drop(first_shared);
    drop(second_shared);
    let exclusive = sim
        .lock_exclusive("rotate-again", &path, false)
        .expect("released: exclusive now succeeds");
    let shared_err = sim
        .lock_shared("open_checked", &path, false)
        .err()
        .expect("shared must refuse against the exclusive holder");
    assert_eq!(shared_err.kind(), std::io::ErrorKind::WouldBlock);
    drop(exclusive);
}

// ── RFC 100 stage 3: `create_dir`/`remove_dir`, the workspace's own lock ────

#[test]
fn real_create_dir_makes_an_owner_only_directory() {
    use std::os::unix::fs::MetadataExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path().join("t.key.rotation-work");
    Real.create_dir("t", &workspace).expect("create_dir");
    let m = std::fs::metadata(&workspace).expect("metadata");
    assert!(m.is_dir());
    assert_eq!(m.mode() & 0o777, 0o700);
}

#[test]
fn real_create_dir_refuses_an_existing_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path().join("t.key.rotation-work");
    Real.create_dir("t", &workspace).expect("first create_dir");
    let err = Real
        .create_dir("t", &workspace)
        .expect_err("a second create_dir must refuse");
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
}

#[test]
fn real_create_dir_refuses_an_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("t.key.rotation-work");
    std::fs::write(&path, b"not a directory").expect("write");
    let err = Real
        .create_dir("t", &path)
        .expect_err("must refuse a path occupied by a regular file");
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
}

/// `mkdir(2)` never follows a symlink for its own, final-component
/// creation -- a symlink occupying the name is itself "something already
/// there," the same refusal as an existing directory or file.
#[test]
fn real_create_dir_refuses_a_symlink_at_the_same_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("elsewhere");
    let link = dir.path().join("t.key.rotation-work");
    std::os::unix::fs::symlink(&elsewhere, &link).expect("symlink");
    let err = Real
        .create_dir("t", &link)
        .expect_err("must refuse a symlink occupying the name");
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(
        !elsewhere.join("anything").exists(),
        "nothing was created through the symlink"
    );
}

/// RFC 100 stage 3 §1's second durability rule, against `Simulated`
/// directly (not through the journal protocol, which has its own tests):
/// an entry written inside a workspace directory, with the workspace's
/// own directory-entry synced to the *workspace* but the workspace's
/// *own* creation never synced to its parent, must vanish in every
/// power-loss state at that point -- not survive because its own write
/// was independently "pending, could persist."
#[test]
fn a_power_loss_before_the_workspace_parent_is_synced_loses_everything_inside_it() {
    use crate::key_adapter::simulated::Simulated;

    let active = Path::new("/virtual/t.key");
    let sim = Simulated::new(&[(active.to_path_buf(), b"old-key\n".to_vec())]);
    let workspace = Path::new("/virtual/t.key.rotation-work");
    sim.create_dir("journal.lock.create_dir", workspace)
        .expect("create_dir");

    let intent = workspace.join("intent.write");
    sim.create_new("journal.intent.create_new", &intent)
        .expect("create_new");
    sim.write_all("journal.intent.write", &intent, b"the intent body")
        .expect("write_all");
    sim.sync_file("journal.intent.sync_file", &intent)
        .expect("sync_file");
    // The workspace's *own* entry (inside `/virtual`) is never synced --
    // syncing the workspace itself only durableizes what's *inside* it.
    sim.sync_dir("journal.intent.sync_dir", workspace)
        .expect("sync_dir workspace");

    let point = sim.points().pop().expect("a recorded point");
    let states = point.power_loss_states();

    // The workspace's own directory entry (under `/virtual`) is still
    // pending -- a power loss could leave it or lose it, same as any
    // other unsynced entry -- so both outcomes must be among the
    // enumerated states.
    assert!(
        states.iter().any(|d| !d.has_dir(workspace)),
        "a power loss losing the workspace itself must be an enumerated state"
    );
    assert!(
        states.iter().any(|d| d.has_dir(workspace)),
        "a power loss keeping the workspace must also be an enumerated state"
    );

    for disk in &states {
        if !disk.has_dir(workspace) {
            assert!(
                disk.read(&intent).is_none(),
                "an entry inside a non-durable workspace cannot survive, \
                 even though its own write and sync both ran"
            );
        }
    }
    // And pruning must not be too aggressive: when the workspace *is*
    // among the states a power loss could leave, the entry inside it
    // (whose own write was fsynced, and whose directory-sync targeted
    // the workspace itself) is one of the candidates too.
    assert!(
        states
            .iter()
            .any(|d| d.has_dir(workspace) && d.read(&intent).is_some()),
        "a power loss keeping both the workspace and its contents must be an enumerated state"
    );
}

#[test]
fn real_remove_dir_removes_an_empty_directory_and_refuses_a_nonempty_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path().join("t.key.rotation-work");
    Real.create_dir("t", &workspace).expect("create_dir");
    Real.remove_dir("t", &workspace).expect("remove empty dir");
    assert!(!workspace.exists());

    Real.create_dir("t", &workspace).expect("create_dir again");
    std::fs::write(workspace.join("intent.json"), b"x").expect("write inside");
    let err = Real
        .remove_dir("t", &workspace)
        .expect_err("must refuse a non-empty directory");
    assert_eq!(err.kind(), std::io::ErrorKind::DirectoryNotEmpty);
    assert!(workspace.exists(), "untouched");
}

/// Stage 3's journal publishes each file with `publish_no_replace`, and
/// relies on it refusing a name that already exists: `rename(2)` would
/// silently replace it. Test it directly, since stage 0 itself never calls
/// it.
#[test]
fn real_publish_no_replace_refuses_an_existing_name_and_touches_neither_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (from, to) = (dir.path().join("next.write"), dir.path().join("next.ready"));
    std::fs::write(&from, b"new").expect("write from");
    std::fs::write(&to, b"already here").expect("write to");
    let err = Real
        .publish_no_replace("t", &from, &to)
        .expect_err("must refuse an existing name");
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read(&from).expect("from"),
        b"new",
        "the source is kept"
    );
    assert_eq!(
        std::fs::read(&to).expect("to"),
        b"already here",
        "the existing file is untouched"
    );
}

#[test]
fn real_publish_no_replace_moves_the_file_when_the_name_is_free() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (from, to) = (dir.path().join("next.write"), dir.path().join("next.ready"));
    std::fs::write(&from, b"new").expect("write from");
    Real.publish_no_replace("t", &from, &to).expect("publish");
    assert_eq!(std::fs::read(&to).expect("to"), b"new");
    assert!(!from.exists(), "the temporary name is gone");
}

#[test]
fn real_metadata_reports_mode_and_does_not_follow_a_symlink() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("key");
    std::fs::write(&file, b"k").expect("write");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    let m = Real.metadata("t", &file).expect("metadata");
    assert_eq!(m.mode & 0o777, 0o600);
    assert!(!m.is_symlink);
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&file, &link).expect("symlink");
    assert!(
        Real.metadata("t", &link).expect("metadata").is_symlink,
        "a symlink is reported as one"
    );
}

// ── D6: the adapter is the only path to the key files ───────────────────────

/// Raw filesystem mutation patterns a key-file touch would use outside
/// the adapter — `std::fs::rename`/`write`/`remove_file`/`hard_link`,
/// `OpenOptions::new`, `File::create`. A call *through* the adapter
/// (`adapter.rename(point, ..)`, `.write_all(point, ..)`) never matches
/// any of these literal substrings.
const FORBIDDEN: &[&str] = &[
    "std::fs::rename",
    "std::fs::write",
    "OpenOptions::new",
    "File::create",
    "std::fs::remove_file",
    "std::fs::hard_link",
    // Not `create_dir_all`/`remove_dir_all`: `write_key_file_impl`'s own
    // `create_dir_all` ensures a key file's *parent* exists, unrelated to
    // the rotation workspace this adapter now owns, and is the one
    // pre-existing exception the opening paren excludes precisely.
    "std::fs::create_dir(",
    "std::fs::remove_dir(",
    "DirBuilder::new",
    // RFC 100 stage 4: `list_dir`'s own primitive.
    "std::fs::read_dir",
];

fn stray_fs_calls(src: &str) -> Vec<&'static str> {
    FORBIDDEN
        .iter()
        .copied()
        .filter(|pat| src.contains(pat))
        .collect()
}

/// `runtime/keyring.rs`, `runtime/rotate.rs` (where the rotation
/// sequence actually lives, since stage 0 moved it out of `cli.rs`),
/// every production file under `runtime/rotate/` (walked, not listed:
/// stage 2's preconditions, stage 3's journal, and whatever stages 4 and
/// 5 add), and `cli.rs` (which still
/// holds `run_admin_rotate_key`'s own existence check and the
/// `--new-key` file read) are the only places that used to, or now,
/// touch a key file or its workspace directly. All of them go through
/// the adapter; this fails if any gains a raw call back.
#[test]
fn nothing_touches_a_key_file_outside_the_adapter() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources: Vec<std::path::PathBuf> = [
        "src/runtime/keyring.rs",
        "src/runtime/rotate.rs",
        "src/cli.rs",
    ]
    .iter()
    .map(|rel| manifest_dir.join(rel))
    .collect();
    // Every production file under `rotate/`, found rather than listed:
    // stages 4 and 5 add files here, and a hand-kept list would miss
    // them. Test sources (`tests.rs`) build fixtures and are skipped.
    let before = sources.len();
    collect_production_sources(&manifest_dir.join("src/runtime/rotate"), &mut sources);
    assert!(
        sources.len() - before >= 2,
        "the walk must find preconditions.rs and journal.rs at least"
    );
    for path in sources {
        let src = std::fs::read_to_string(&path)
            .with_context(|| path.display().to_string())
            .expect("read the production source");
        let found = stray_fs_calls(&src);
        assert!(
            found.is_empty(),
            "{} touches a key file outside the adapter: {found:?}",
            path.display()
        );
    }
}

fn collect_production_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| dir.display().to_string())
        .expect("read a source directory")
        .map(|e| e.expect("a directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_production_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "tests.rs")
        {
            out.push(path);
        }
    }
}

// ── RFC 100 stage 3: materialize() keeps the workspace level ────────────────

/// A workspace in the snapshot becomes a real `0700` directory, and an
/// entry inside it lands inside it — not flattened beside the key file,
/// where recovery would read it as something else. A snapshot that lost
/// the workspace materializes neither the directory nor its contents.
#[test]
fn materialize_keeps_an_entry_inside_its_workspace() {
    use crate::key_adapter::simulated::{Simulated, materialize};
    use std::os::unix::fs::PermissionsExt;

    let active = Path::new("/virtual/t.key");
    let workspace = Path::new("/virtual/t.key.rotation-work");
    let intent = workspace.join("intent.json");
    let sim = Simulated::new(&[(active.to_path_buf(), b"old-key\n".to_vec())]);
    sim.create_dir("m.create_dir", workspace)
        .expect("create_dir");
    sim.create_new("m.create_new", &intent).expect("create_new");
    sim.write_all("m.write", &intent, b"journal")
        .expect("write_all");
    sim.finish();
    let point = sim.points().pop().expect("a recorded point");

    let dir = tempfile::tempdir().expect("tempdir");
    materialize(&point.kill_state(), dir.path()).expect("materialize");
    let real_workspace = dir.path().join("t.key.rotation-work");
    let mode = std::fs::metadata(&real_workspace)
        .expect("the workspace is materialized")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_eq!(
        std::fs::read(real_workspace.join("intent.json")).expect("inside"),
        b"journal"
    );
    assert!(
        !dir.path().join("intent.json").exists(),
        "never flattened beside the key file"
    );

    let lost = point
        .power_loss_states()
        .into_iter()
        .find(|d| !d.has_dir(workspace))
        .expect("nothing was synced, so a power loss can lose the workspace");
    let dir = tempfile::tempdir().expect("tempdir");
    materialize(&lost, dir.path()).expect("materialize");
    assert!(!dir.path().join("t.key.rotation-work").exists());
    assert!(!dir.path().join("intent.json").exists());
}

// ── N1: materialize() writes a Disk's entries, with 0600 permissions ────────

#[test]
fn materialize_writes_each_entrys_bytes_with_owner_only_permissions() {
    use crate::key_adapter::simulated::{Simulated, materialize};
    use std::os::unix::fs::PermissionsExt;

    let sim = Simulated::new(&[(
        std::path::PathBuf::from("/virtual/t.key"),
        b"old-key-body\n".to_vec(),
    )]);
    sim.create_new(
        "materialize.probe.create_new",
        Path::new("/virtual/t.key.new"),
    )
    .expect("create_new");
    sim.write_all(
        "materialize.probe.write",
        Path::new("/virtual/t.key.new"),
        b"new-key-body\n",
    )
    .expect("write_all");
    sim.finish();

    let disk = sim
        .points()
        .pop()
        .expect("at least one recorded point")
        .kill_state();

    let dir = tempfile::tempdir().expect("tempdir");
    materialize(&disk, dir.path()).expect("materialize");

    let old = dir.path().join("t.key");
    let new = dir.path().join("t.key.new");
    assert_eq!(std::fs::read(&old).expect("read old"), b"old-key-body\n");
    assert_eq!(std::fs::read(&new).expect("read new"), b"new-key-body\n");
    for path in [&old, &new] {
        let mode = std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{} must be owner-only", path.display());
    }
}

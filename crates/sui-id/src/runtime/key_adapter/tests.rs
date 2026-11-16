use super::*;
use anyhow::Context;

#[test]
fn real_metadata_refuses_a_missing_path() {
    assert!(Real.metadata("x", Path::new("/does/not/exist")).is_err());
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
/// and `cli.rs` (which still holds `run_admin_rotate_key`'s own
/// existence check and the `--new-key` file read) are the only places
/// that used to touch a key file directly. All three now go through
/// the adapter; this fails if any of them gains a raw call back.
#[test]
fn nothing_touches_a_key_file_outside_the_adapter() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    for rel in [
        "src/runtime/keyring.rs",
        "src/runtime/rotate.rs",
        "src/cli.rs",
    ] {
        let path = std::path::Path::new(manifest_dir).join(rel);
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

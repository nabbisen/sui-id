//! `0.91.0` stage 1, part A: durability can't be observed from a test
//! (a successful `sync_all` and a no-op one look identical from here), so
//! these test the parts that can be: the directory-sync helper's own
//! success/error shape, and that `write_key_file` still creates the file
//! the same way it always did -- `0600`, `create_new`, with the given
//! contents -- now that it also syncs before returning.

use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn sync_parent_dir_succeeds_on_a_real_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("some-file");
    std::fs::write(&file_path, b"x").expect("write");
    sync_parent_dir(&file_path).expect("sync a real directory's parent");
}

#[test]
fn sync_parent_dir_errors_for_a_path_whose_directory_does_not_exist() {
    let path = Path::new("/this/directory/does/not/exist/at/all/key.txt");
    let err = sync_parent_dir(path).expect_err("no such directory");
    // The error must name the step that failed, not just bubble up a bare
    // OS error -- `with_context`'s whole point.
    assert!(
        format!("{err:#}").contains("to sync it"),
        "error should explain what it was trying to do: {err:#}"
    );
}

#[test]
fn write_key_file_creates_0600_with_create_new_and_the_given_contents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("master.key");

    write_key_file(&path, "the-key-body").expect("first write");

    let contents = std::fs::read_to_string(&path).expect("read back");
    assert_eq!(contents, "the-key-body\n");

    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "key file must be owner-only");

    // `create_new`: a second call on the same path must fail, not
    // silently overwrite -- this is the guarantee the doc comment on
    // `write_key_file` names explicitly.
    let second = write_key_file(&path, "a-different-body");
    assert!(second.is_err(), "a second write to the same path must fail");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back again"),
        "the-key-body\n",
        "the original contents must be untouched by the failed second write"
    );
}

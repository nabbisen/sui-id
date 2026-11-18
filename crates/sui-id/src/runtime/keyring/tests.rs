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

// ── RFC 100 stage 1: generating a key, in all three modes ──────────────
//
// `resolve_checked_impl`'s generate branch calls the same
// `write_key_file_impl` rotation's own staging write already uses, so
// the write sequence itself (`create_new`, `write_all`, `sync_file`,
// `sync_dir`) is not new. What stage 1 adds is a second caller of it —
// generation — that stage 0's own tests never drove through the harness
// (only rotation's staging write was). These tests do.

use crate::key_adapter::failpoint::Failpoint;
use crate::key_adapter::simulated::Simulated;
use std::path::PathBuf;

#[test]
fn generate_with_a_short_write_errors_and_leaves_no_valid_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("t.key");
    let fp = Failpoint::from_spec("keyring.generate.write:short-write");
    let err = resolve_checked_impl(&fp, &path, Path::new("/does/not/exist"))
        .map(|_| ())
        .expect_err("a short write must surface as an error");
    assert!(
        format!("{err:#}").contains("t.key"),
        "the error names the key file: {err:#}"
    );
    // `create_new` succeeded before the short write, so the file exists
    // but holds only half its intended bytes -- confirm that half-write
    // cannot be read back as a valid key, which would be the silent
    // failure mode this test guards against.
    if let Ok(body) = std::fs::read_to_string(&path) {
        assert!(
            MasterKey::from_base64(body.trim()).is_err(),
            "a short-written key file must not parse as a valid key"
        );
    }
}

/// The safety property a torn write over *generation* actually needs —
/// weaker than "every leftover file is a whole valid key," which a
/// first run of this test found false (a crash between `create_new` and
/// `write_all` durably leaves a real, empty file, and an empty file is
/// not valid base64 — expected, not a defect: `create_new`'s own
/// no-overwrite guarantee is what matters next start, not this one).
/// What must hold: the key path is either absent, or — if `read` gives
/// back something — either it fails to parse (safely refused on the
/// next start, never silently accepted) or it parses back to exactly
/// the one value this run ever wrote (never a different, wrong key:
/// there is only one candidate here, unlike rotation's old/new pair,
/// but the check costs nothing and documents that this is what would
/// be compared if there were a second one).
#[test]
fn generate_in_every_power_loss_state_is_absent_unparseable_or_exactly_the_written_key() {
    let path = PathBuf::from("/virtual/t.key");
    let new_key = MasterKey::generate();
    let body = new_key.to_base64();
    let sim = Simulated::new(&[]);
    write_key_file_impl(&sim, "keyring.generate", &path, &body)
        .expect("write_key_file_impl should succeed against Simulated");
    sim.finish();

    let mut checked = 0usize;
    let mut saw_a_whole_key = false;
    for point in sim.points() {
        for disk in point.power_loss_states() {
            checked += 1;
            let Some(bytes) = disk.read(&path) else {
                continue; // no file at all: always safe
            };
            let Ok(text) = std::str::from_utf8(bytes) else {
                continue; // not UTF-8: fails to parse, safely refused
            };
            let Ok(parsed) = MasterKey::from_base64(text.trim()) else {
                continue; // fails to parse: safely refused, never a defect
            };
            saw_a_whole_key = true;
            assert_eq!(
                parsed.to_base64(),
                body,
                "{}: a power-loss state parsed as a key, but not the one this run wrote",
                point.label
            );
        }
    }
    assert!(
        checked > 0,
        "the enumeration must actually exercise some states"
    );
    assert!(
        saw_a_whole_key,
        "the enumeration must include at least the fully-written, successful state"
    );
}

#[test]
fn a_missing_key_over_a_sealed_database_touches_the_key_path_not_at_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key = MasterKey::generate();
    {
        let mut conn = rusqlite::Connection::open(&db_path).expect("open");
        sui_id_store::migrations::run(&mut conn).expect("migrate");
        let sealed = sui_id_store::crypto::seal(
            &key,
            b"private-key-material-for-a-test",
            b"sui-id/signing_key/v1",
        )
        .expect("seal");
        conn.execute(
            "INSERT INTO signing_keys (id, algorithm, private_key_enc, public_key, is_active, created_at) \
             VALUES ('k1', 'Ed25519', ?1, X'00', 1, '2026-01-01T00:00:00Z')",
            rusqlite::params![sealed],
        )
        .expect("insert sealed row");
    }

    let key_path = PathBuf::from("/virtual/t.key"); // never present in `sim`
    let sim = Simulated::new(&[]);
    let err = resolve_checked_impl(&sim, &key_path, &db_path)
        .map(|_| ())
        .expect_err("missing key over a sealed, unverified database must refuse");
    assert!(format!("{err:#}").contains("no master key file"), "{err:#}");
    assert!(
        sim.points().is_empty(),
        "no key-file operation may be attempted when refusing to generate"
    );
}

// ── RFC 100 stage 1: `resolve_checked` is the one way in ───────────────

/// Production source that could plausibly resolve a master key directly
/// — `main.rs` and `cli.rs` (every CLI command that opens the database),
/// and the two `runtime` modules that orchestrate opening it
/// (`database.rs`'s own [`resolve_checked`]-based [`open_checked`], and
/// `startup.rs`, which calls [`open_checked`] rather than `resolve` or
/// `resolve_checked` directly). `keyring.rs` itself is not listed: this
/// checks for calls *to* it, and its own body necessarily contains the
/// unqualified names `resolve`/`resolve_impl`/`resolve_checked_impl`,
/// never the qualified form this greps for.
#[test]
fn resolve_is_only_called_from_the_keyring_module() {
    // Every `.rs` file under `src/`, not a named few (review fold-in,
    // 0.93.0 O1): a call added to a handler or a new module must fail this
    // just as one in `cli.rs` does. Only the keyring module itself and its
    // tests, and other test sources, may name the unchecked resolve.
    // Rotation's old-key load calls
    // `keyring::resolve_checked(`, a different substring.
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let keyring_module = src_root.join("runtime").join("keyring.rs");
    let keyring_tests = src_root.join("runtime").join("keyring");
    let mut files = Vec::new();
    walk(&src_root, &mut files);
    assert!(
        files.len() > 50,
        "the scan found only {} files",
        files.len()
    );
    let mut offenders = Vec::new();
    for path in files {
        // The keyring module and its tests, and any test source (a
        // `tests.rs`, or a file under a `tests/` directory): tests read a
        // key file back after rotation, which is what the unchecked
        // resolve is for.
        let is_test_source = path.file_name().is_some_and(|n| n == "tests.rs")
            || path.components().any(|c| c.as_os_str() == "tests");
        if path == keyring_module || path.starts_with(&keyring_tests) || is_test_source {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read source");
        if src.contains("keyring::resolve(") {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "these call keyring::resolve( directly; every caller outside \
         runtime/keyring.rs goes through database::open_checked instead: {offenders:?}"
    );
}

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! RFC 142 stage 4 — the store-side half of `sui-id preview-upgrade`: a
//! migration's `name` cannot lie about its file or its version (§4), and the
//! preview's and the runner's definitions of "pending" cannot disagree (§3,
//! §6.7). The CLI-facing output and the "must not write" measurement are
//! tested beside the `preview-upgrade` command itself.

use crate::migrations::{self, MAX_SCHEMA_VERSION, StoredVersion};
use std::collections::BTreeSet;
use std::path::Path;

fn migrations_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/migrations")
}

/// RFC 142 stage 4 §4: every entry's `name` must name a real file whose
/// bytes are exactly what is embedded, whose numeric prefix is its
/// `version`, and every `.sql` file in the directory must be registered —
/// so an added-but-unwired migration file fails here instead of silently
/// never running.
#[test]
fn migration_names_match_their_files_and_cover_the_directory() {
    let dir = migrations_dir();
    let mut on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".sql"))
        .collect();

    for m in migrations::all() {
        let file_name = format!("{}.sql", m.name);
        let path = dir.join(&file_name);
        let on_disk_bytes = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("migration {}: {}: {e}", m.version, path.display()));
        assert_eq!(
            on_disk_bytes,
            m.sql,
            "migration {}: embedded sql differs from {}",
            m.version,
            path.display()
        );

        let prefix = m
            .name
            .split('_')
            .next()
            .unwrap_or_else(|| panic!("migration {}: name {:?} has no '_'", m.version, m.name));
        let prefix_num: i32 = prefix.parse().unwrap_or_else(|e| {
            panic!(
                "migration {}: name {:?}'s numeric prefix {:?} does not parse: {e}",
                m.version, m.name, prefix
            )
        });
        assert_eq!(
            prefix_num, m.version,
            "migration {}: name {:?}'s numeric prefix is {}, not the registered version",
            m.version, m.name, prefix_num
        );

        assert!(
            on_disk.remove(&file_name),
            "migration {}: {file_name} was already accounted for twice (duplicate name?)",
            m.version
        );
    }

    assert!(
        on_disk.is_empty(),
        "these .sql files exist in {} but are not registered in MIGRATIONS: {on_disk:?}",
        dir.display()
    );
}

/// RFC 142 stage 4 §3/§6.7: for every version the runner can be built to,
/// `pending`'s list and what `migrations::run` actually applies must be
/// exactly the same set, in the same order — the preview and the runner
/// cannot disagree about what will run.
#[test]
fn preview_and_runner_agree_on_every_pending_version() {
    for n in 0..MAX_SCHEMA_VERSION {
        let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable FK");
        migrations::run_up_to(&mut conn, n).expect("run_up_to(n)");

        let stored = migrations::read_stored_version(&conn).expect("read stored version");
        let preview_list: Vec<(i32, &'static str)> = migrations::pending(stored)
            .map(|m| (m.version, m.name))
            .collect();

        migrations::APPLIED.with(|applied| applied.borrow_mut().clear());
        migrations::run(&mut conn).expect("run to head");
        // What `run` actually committed, observed rather than re-derived: a
        // runner that skipped a migration nothing later depends on still
        // ends at MAX_SCHEMA_VERSION, so the version check below cannot
        // catch it on its own (measured: skipping 0045 passed without this).
        let applied: Vec<i32> = migrations::APPLIED.with(|applied| applied.borrow().clone());
        let previewed: Vec<i32> = preview_list.iter().map(|(v, _)| *v).collect();
        assert_eq!(
            applied, previewed,
            "n={n}: run() applied a different set of migrations from the one the preview listed"
        );
        let after: i64 = conn
            .query_row(
                "SELECT value FROM sui_meta WHERE key = 'schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            after,
            i64::from(MAX_SCHEMA_VERSION),
            "n={n}: run() did not reach MAX_SCHEMA_VERSION"
        );

        let expected: Vec<(i32, &'static str)> = migrations::all()
            .iter()
            .filter(|m| i64::from(m.version) > n as i64 && m.version <= MAX_SCHEMA_VERSION)
            .map(|m| (m.version, m.name))
            .collect();
        assert_eq!(
            preview_list, expected,
            "n={n}: preview's pending list disagrees with the full migration set above n"
        );
    }
}

/// Mutation evidence for `pending`'s boundary (§6's mutation table: "the
/// preview using its own `>` comparison instead of the shared function, off
/// by one (`>=`)"). `pending` must include a migration whose version equals
/// `stored` plus one, and must exclude one equal to `stored` itself.
#[test]
fn pending_boundary_is_strictly_greater_than_stored() {
    let list: Vec<i32> = migrations::pending(StoredVersion::At(44))
        .map(|m| m.version)
        .collect();
    assert_eq!(
        list,
        (45..=MAX_SCHEMA_VERSION).collect::<Vec<_>>(),
        "pending(44) must be exactly 45..=MAX, in order"
    );
    assert!(
        !list.contains(&44),
        "pending(44) must not include 44 itself (off-by-one toward >=)"
    );
}

/// The immutable open must name the right file whatever the directory is
/// called. SQLite reads `?`, `#` and `%` in a `file:` URI as query, fragment
/// and escape; unencoded, a database at schema 44 under `has#hash/` or
/// `has?q/` was opened as a different, nonexistent file and previewed as
/// **empty** (RFC 142 stage 4 review, measured). Each case is a cleanly closed
/// WAL database, the shape that takes the immutable path, and must also leave
/// the directory exactly as it was.
#[test]
fn preview_reads_the_right_file_whatever_the_directory_is_called() {
    for dir_name in [
        "plain",
        "has#hash",
        "has?q",
        "pct%41",
        "has space",
        "\u{e9}t\u{e9}",
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join(dir_name);
        std::fs::create_dir(&dir).expect("create dir");
        let db = dir.join("t.sqlite");
        {
            let mut conn = rusqlite::Connection::open(&db).expect("open db");
            conn.pragma_update(None, "journal_mode", "WAL")
                .expect("wal");
            conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
            migrations::run_up_to(&mut conn, 44).expect("run_up_to(44)");
        }
        let names = |d: &Path| -> BTreeSet<std::ffi::OsString> {
            std::fs::read_dir(d)
                .expect("read dir")
                .map(|e| e.expect("entry").file_name())
                .collect()
        };
        let before = names(&dir);
        assert!(
            !dir.join("t.sqlite-wal").exists(),
            "{dir_name:?}: precondition, a cleanly closed WAL database has no -wal"
        );
        let preview = migrations::preview(&db).expect("preview");
        match preview {
            migrations::Preview::Pending { current, .. } => assert_eq!(current, 44, "{dir_name:?}"),
            other => panic!("{dir_name:?}: previewed as {other:?}, not Pending at 44"),
        }
        assert_eq!(
            names(&dir),
            before,
            "{dir_name:?}: the preview changed the directory"
        );
    }
}

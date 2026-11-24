-- RFC 100 `0.94.0` P1 (database half): `master_key_rotation` gets its
-- full shape. Migration 0052 (stage 1) created it unused, with only
-- `started_at`/`updated_at` where RFC 100's own *State machine* names
-- *prepared*, *committed* and *completed* timestamps, and a free-text
-- `custody_disposition` with no structure and no check. Nothing has
-- ever written a row to it (K07 extended and K05, this stage, are the
-- first writers) — this migration rebuilds the table rather than
-- migrating data that cannot exist.
--
-- No `FK_DISABLE_REQUIRED`: nothing references this table by foreign
-- key (it is a standalone singleton, the same idiom
-- `master_key_sentinel` itself uses).
--
-- First, prove the table really is empty. A `CHECK` constraint cannot
-- itself reference another table (SQLite refuses "subqueries
-- prohibited in CHECK constraints"), so this uses a temporary trigger,
-- which can: `RAISE(ABORT, ...)` inside it fails the `INSERT` that
-- fires it, and `apply_migration`'s own transaction (migrations.rs)
-- rolls the whole migration back on any error from `execute_batch` --
-- nothing below this point has run yet, so "changes nothing" holds
-- without this needing `RAISE(ROLLBACK, ...)` or any special-casing of
-- its own.
CREATE TEMP TABLE _0053_guard (ok INTEGER);
CREATE TEMP TRIGGER _0053_guard_refuses_a_populated_table
BEFORE INSERT ON _0053_guard
WHEN EXISTS (SELECT 1 FROM master_key_rotation)
BEGIN
    SELECT RAISE(ABORT, 'refusing migration 0053: master_key_rotation already holds a row, but no release has ever written one; this upgrade path is not defined for a populated table');
END;
INSERT INTO _0053_guard VALUES (1);
DROP TRIGGER _0053_guard_refuses_a_populated_table;
DROP TABLE _0053_guard;

-- Proven empty above. The rebuild sequence 0022/0051 use for a table
-- that holds data to carry forward -- new table, drop the old, rename
-- the new one into place -- rather than a bare drop-and-recreate,
-- matching this schema's own established idiom even though there is
-- no data to copy here.
--
-- `disposition_kind`'s own `CHECK` ties it to exactly the fields RFC
-- 100's `BackupDisposition` carries for that kind (`journal.rs`'s own
-- enum, which this matches field for field): `retain_until` carries
-- `review_at` and `custody_reference`, never `verification_reference`;
-- `remove_after_verified` carries only `verification_reference`.
-- `review_at` is `TEXT`, same column type as every other `_at`
-- timestamp in this schema, but is never free text at the Rust layer
-- -- `sui_id_store::master_key_rotation::RotationDisposition::RetainUntil`'s
-- own field is `chrono::DateTime<Utc>`, so a caller cannot construct
-- one without an already-parsed, valid instant.
CREATE TABLE _master_key_rotation_new (
    id                      INTEGER PRIMARY KEY CHECK (id = 1),
    rotation_id             TEXT    NOT NULL,
    old_fingerprint         TEXT    NOT NULL,
    new_fingerprint         TEXT    NOT NULL,
    state                   TEXT    NOT NULL
                                CHECK (state IN ('db_committed_pending_activation', 'complete')),
    prepared_at             TEXT    NOT NULL,
    committed_at            TEXT    NOT NULL,
    completed_at            TEXT
                                CHECK (
                                    (state = 'complete' AND completed_at IS NOT NULL)
                                    OR (state = 'db_committed_pending_activation' AND completed_at IS NULL)
                                ),
    backup_filename         TEXT    NOT NULL,
    disposition_kind        TEXT    NOT NULL
                                CHECK (disposition_kind IN ('retain_until', 'remove_after_verified')),
    review_at               TEXT,
    custody_reference       TEXT,
    verification_reference  TEXT,
    CHECK (
        (
            disposition_kind = 'retain_until'
            AND review_at IS NOT NULL
            AND custody_reference IS NOT NULL
            AND verification_reference IS NULL
        )
        OR (
            disposition_kind = 'remove_after_verified'
            AND verification_reference IS NOT NULL
            AND review_at IS NULL
            AND custody_reference IS NULL
        )
    )
) STRICT;

DROP TABLE master_key_rotation;
ALTER TABLE _master_key_rotation_new RENAME TO master_key_rotation;

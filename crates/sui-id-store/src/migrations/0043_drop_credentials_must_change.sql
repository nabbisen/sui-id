-- RFC 115 D12: `credentials.must_change` is deleted.
--
-- The column was written in three places and read by nothing: no sign-in path
-- ever consulted it, so a flagged account was never forced to change its
-- password. A control that does not exist reads as one in review. Enforcing it
-- properly is its own piece of work (every raw session reader, step-up and
-- factor enrolment during the forced state, a flag on the session row), and
-- after RFC 115 the only writer of `true` was `sui-id setup` for the
-- operator's own account, so it is removed rather than left to mislead.
--
-- Migration 0022 attached `CHECK (must_change IN (0, 1))` to the column
-- itself; SQLite drops a column whose only reference is a check on that same
-- column. Verified on the bundled SQLite before this file was written
-- (`tests_rfc115.rs`, `drop_column_works_on_the_pinned_sqlite_…`).

ALTER TABLE credentials DROP COLUMN must_change;

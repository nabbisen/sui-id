-- 0018: Idle session timeout and concurrent session cap.
--
-- v0.25.0 adds two adjacent self-hardening knobs over the
-- existing UI-cookie session model:
--
-- 1. **Idle session timeout** — a session that has not been
--    presented for `idle_session_timeout_secs` becomes
--    invalid. Bounds the post-compromise window of a stolen
--    cookie when the legitimate user has stopped using it.
-- 2. **Concurrent session cap** — a single user may hold at
--    most `max_concurrent_sessions` *active* (un-expired,
--    un-revoked, un-idled) sessions at a time. New logins past
--    the cap evict the oldest existing session (FIFO).
--
-- Both are **opt-in**: the seeded defaults are zero, which the
-- application treats as "feature off". Operators turn them on
-- via the admin settings page (and via direct DB write at this
-- release; an admin UI for these two knobs is in the v0.25.x
-- scope-expansion entry).
--
-- ## sessions.last_used_at
--
-- The idle-timeout check needs a stable "most recent presentation"
-- timestamp. Adding a column to `sessions` and updating it on
-- every authenticated request is the simplest implementation;
-- the per-request UPDATE is throttled at the application layer
-- (write only when the cached value is more than a minute old)
-- so a busy session does not generate one DB write per HTTP
-- request.
--
-- The column is nullable. Existing rows from before this
-- migration get NULL, and the application treats NULL as "as
-- old as `created_at`" — a session that has never had its
-- last_used_at written is treated as if its last presentation
-- was when it was created. This is conservative: under the
-- new idle-timeout policy, pre-migration sessions get the same
-- treatment as a brand-new session that has not yet been
-- re-presented.
--
-- ## server_settings columns
--
-- Both new fields go on the existing `server_settings`
-- singleton row from migration 0016, alongside the v0.23.0 i18n
-- defaults and the v0.24.0 hibp_mode. No new table.
--
-- - `idle_session_timeout_secs INTEGER NOT NULL DEFAULT 0`
--   Number of seconds of idleness after which a session is
--   treated as expired. `0` means "no idle-timeout enforcement"
--   — sessions only expire at their `expires_at`.
--   Application-validated to be in range `[0, 30 * 86400]`
--   (30 days) so a fat-fingered config does not effectively
--   disable the feature by setting it past the absolute
--   `expires_at` ceiling.
-- - `max_concurrent_sessions INTEGER NOT NULL DEFAULT 0`
--   Maximum simultaneous active sessions per user. `0` means
--   "no cap". Application-validated to be in range `[0, 1000]`
--   to surface obviously-wrong values quickly.
--
-- Both are stored as INTEGER (seconds / count) rather than as
-- ISO-8601 durations so SQLite-level filtering and ordering
-- can use them directly. The CHECK ranges here are looser than
-- the application validation; we want the DB to refuse only
-- nonsensical values (negative numbers), and let the
-- application reject unhelpful-but-not-wrong ones (1 second
-- timeout, 1 million sessions) with a friendlier message.

ALTER TABLE sessions
    ADD COLUMN last_used_at TEXT;

CREATE INDEX IF NOT EXISTS idx_sessions_user_active
    ON sessions(user_id, created_at)
    WHERE revoked_at IS NULL;

ALTER TABLE server_settings
    ADD COLUMN idle_session_timeout_secs INTEGER NOT NULL DEFAULT 0
        CHECK (idle_session_timeout_secs >= 0);

ALTER TABLE server_settings
    ADD COLUMN max_concurrent_sessions INTEGER NOT NULL DEFAULT 0
        CHECK (max_concurrent_sessions >= 0);

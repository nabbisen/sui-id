-- 0007: per-user account lockout state.
--
-- Two columns on `users`:
--
--   * `failed_login_count` — running count of consecutive password
--     failures since the last success. Reset to 0 on a successful
--     password verification (and only on that — MFA is a separate
--     stage).
--
--   * `locked_until` — wall-clock time before which password
--     verification will be refused for this account, regardless of
--     whether the supplied password is correct. NULL means "not
--     locked". When a locked account's `locked_until` is in the
--     past, the lock has expired and is cleared on next sign-in.
--
-- The lockout decision is made *before* Argon2 verification (we
-- skip the work entirely on a known-locked account). To preserve
-- timing equivalence we still run a dummy Argon2 round on a fixed
-- decoy hash before returning, so a locked account doesn't respond
-- noticeably faster than an active one — see
-- `sui_id_core::session::login_password`.

ALTER TABLE users
    ADD COLUMN failed_login_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE users
    ADD COLUMN locked_until TEXT;

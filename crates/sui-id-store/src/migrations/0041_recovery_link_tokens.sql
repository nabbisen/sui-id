-- RFC 103 D2, D3: an administrator-issued recovery link is a row in
-- `password_reset_tokens`, the same table the forgot-password email uses.
--
-- - `issued_via` records where a token came from: 'email' (the
--   forgot-password flow, every existing row), 'web' (an administrator in the
--   admin panel) or 'cli' (`sui-id admin issue-recovery-link`).
-- - `issued_by` is the administrator who issued a 'web' token, and is NULL for
--   every other origin. The CHECK ties the two: a token has an issuing
--   administrator exactly when it was issued on the web. The FK cascades, so
--   a hard-deleted administrator takes their unused links with them rather
--   than leaving a row that breaks the CHECK.
-- - `revoked_at` invalidates a token without consuming it (D3): issuing a new
--   link, changing the password, disabling or deleting the user each revoke
--   the user's outstanding tokens. It is distinct from `consumed_at`, which
--   means "used": a revoked token is never reported as completed.
--
-- The two partial indexes serve D8's throttle, which counts a rolling hour of
-- issuances per issuing administrator and per CLI from this table.

ALTER TABLE password_reset_tokens
    ADD COLUMN issued_via TEXT NOT NULL DEFAULT 'email'
        CHECK (issued_via IN ('email', 'web', 'cli'));

ALTER TABLE password_reset_tokens
    ADD COLUMN issued_by TEXT REFERENCES users(id) ON DELETE CASCADE
        CHECK ((issued_by IS NOT NULL) = (issued_via = 'web'));

ALTER TABLE password_reset_tokens
    ADD COLUMN revoked_at TEXT;

CREATE INDEX idx_password_reset_tokens_issued_by
    ON password_reset_tokens (issued_by, issued_at)
    WHERE issued_by IS NOT NULL;

CREATE INDEX idx_password_reset_tokens_cli
    ON password_reset_tokens (issued_at)
    WHERE issued_via = 'cli';

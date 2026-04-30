-- 0005: Access token revocation list (RFC 7009).
--
-- Access tokens are signed JWTs verified offline at protected
-- endpoints. To support real revocation we keep a deny-list of JTI
-- values for tokens that have been revoked but have not yet hit
-- their natural expiry. Every protected-endpoint check that already
-- decodes the JWT (introspect, userinfo, etc) consults this list.
--
-- The list is purged in the GC sweep: once a row's exp has passed,
-- the underlying token is unverifiable anyway, so the deny entry
-- has done its job and can go away.
CREATE TABLE IF NOT EXISTS revoked_access_tokens (
    jti        TEXT PRIMARY KEY,
    revoked_at TEXT NOT NULL,
    -- The token's exp claim. Used for GC; once this passes, the
    -- entry is no longer needed because the JWT itself won't verify.
    exp        TEXT NOT NULL,
    -- Best-effort attribution for the audit trail. Both nullable
    -- because revocation can be done by client credentials or by
    -- an admin session, and we don't always have a user attached.
    revoked_by_user   TEXT REFERENCES users(id) ON DELETE SET NULL,
    revoked_by_client TEXT REFERENCES clients(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_revoked_access_tokens_exp
    ON revoked_access_tokens(exp);

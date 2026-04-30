-- 0008: refresh-token family for theft detection.
--
-- A refresh token "family" is a chain of rotations all rooted at one
-- original issuance. Each rotation creates a new row whose
-- `family_id` matches the root's `id`. We use the family to detect
-- token theft: if a *revoked* refresh token is presented at the
-- token endpoint, the most plausible explanation is that the
-- legitimate client already rotated it once and the attacker is
-- replaying the old captured copy. The defensive response is to
-- revoke every other token in the same family — the attacker
-- can no longer use the captured token, the legitimate client
-- forces a re-authentication on next refresh, and the user gets
-- to notice and rotate their password if they want.
--
-- See OAuth 2.1 §6.1, Best Current Practice draft §4.13.2, and
-- RFC 6819 §5.2.2.3 ("Refresh tokens may be revoked when used
-- after revocation as a strong heuristic for token theft").
--
-- Pre-migration rows get `family_id = id`, so they each form a
-- one-token family. That's the most conservative choice: a
-- pre-0.17.0 token is in its own family, behaves the same as
-- before, and the first rotation after the upgrade lengthens the
-- chain normally.

ALTER TABLE refresh_tokens
    ADD COLUMN family_id TEXT NOT NULL DEFAULT '';

UPDATE refresh_tokens
SET family_id = id
WHERE family_id = '';

-- Index for fast family-wide revoke. Refresh-token operations
-- already index by id, but we also lookup by family_id when we
-- need to revoke a whole chain at once.
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_family
    ON refresh_tokens(family_id);

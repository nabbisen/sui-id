-- 0006: record which authentication factors a session was created with.
--
-- Used to populate the `acr` (Authentication Context Class Reference)
-- and `amr` (Authentication Methods References) claims when issuing
-- ID tokens, per OpenID Connect Core §2 and RFC 8176.
--
-- We add the column on both `sessions` (for completeness, and for
-- future flows that read it directly) and `authorization_codes`
-- (because this is what the token endpoint actually consults at
-- ID-token issuance time — by the time the auth code is exchanged,
-- the session may have already been revoked, so we copy the
-- snapshot of factors at auth-code issuance time and read it back
-- here).
--
-- Refresh-token rotation reuses the same field through the
-- `refresh_tokens` table, similarly snapshot-at-issuance.
--
-- Default '[]' (empty JSON array) covers rows that pre-date this
-- migration. Issuance code treats an empty list the same as a
-- single-factor (password-only) session — the lowest LoA — so older
-- sessions, codes, and refresh tokens continue to work without
-- claim drift in either direction.

ALTER TABLE sessions
    ADD COLUMN auth_methods TEXT NOT NULL DEFAULT '[]';

ALTER TABLE auth_codes
    ADD COLUMN auth_methods TEXT NOT NULL DEFAULT '[]';

ALTER TABLE refresh_tokens
    ADD COLUMN auth_methods TEXT NOT NULL DEFAULT '[]';

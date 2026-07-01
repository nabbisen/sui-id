-- RFC 008: scope catalog and dynamic-client-registration token tables.

-- scope_definition — the deployment's declared scope catalog.
--
-- Each row describes one scope token.  requires_consent controls whether
-- the consent screen must show this scope to the user; is_default marks
-- scopes that are always included when a client requests openid.
--
-- The minimum catalog (openid, profile, email, offline_access) is seeded
-- below.  Operators extend via the admin scope-catalog page.
CREATE TABLE scope_definition (
    name             TEXT PRIMARY KEY,     -- 'openid', 'profile', 'email', …
    requires_consent INTEGER NOT NULL DEFAULT 1 CHECK (requires_consent IN (0, 1)),
    is_default       INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at       TEXT NOT NULL
) STRICT;

-- Seed the minimum catalog.
INSERT INTO scope_definition (name, requires_consent, is_default, created_at)
VALUES
    ('openid',         0, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('profile',        1, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('email',          1, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('offline_access', 1, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

-- client_registration_token — initial access tokens for RFC 7591 dynamic
-- client registration (P4/P5).
--
-- token_hash   — SHA-256 hex of the raw bearer token.  Never stored plain.
-- max_uses     — 0 means unlimited; positive integer caps use count.
-- used_count   — incremented on each successful registration.
-- expires_at   — TTL-bounded; NULL means does not expire.
-- revoked_at   — operator revocation timestamp.
CREATE TABLE client_registration_token (
    id           TEXT PRIMARY KEY,
    token_hash   TEXT NOT NULL UNIQUE,
    max_uses     INTEGER NOT NULL DEFAULT 0,
    used_count   INTEGER NOT NULL DEFAULT 0,
    expires_at   TEXT,
    revoked_at   TEXT,
    note         TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
) STRICT;

-- RFC 004: per-user federation links.
--
-- Each row records that a local user has authenticated via a specific
-- upstream provider.  The mapping key is (provider_id, upstream_sub) —
-- NEVER email (P1).  upstream_email is stored as metadata only; an
-- attacker changing their email at the upstream cannot hijack an existing
-- link.
--
-- UNIQUE (provider_id, upstream_sub) enforces P1: a given upstream
-- identity can link to at most one local user.  This prevents an attacker
-- who somehow obtains a sub from linking it to a second account.
CREATE TABLE federation_link (
    user_id        TEXT NOT NULL,
    provider_id    TEXT NOT NULL,
    upstream_sub   TEXT NOT NULL,    -- `sub` claim from upstream ID token
    upstream_email TEXT,             -- last-seen email; metadata only (P1)
    linked_at      TEXT NOT NULL,
    last_seen_at   TEXT NOT NULL,
    PRIMARY KEY (user_id, provider_id),
    UNIQUE (provider_id, upstream_sub),
    FOREIGN KEY (user_id)     REFERENCES users               (id) ON DELETE CASCADE,
    FOREIGN KEY (provider_id) REFERENCES federation_provider (id) ON DELETE CASCADE
) STRICT;

-- RFC 004: upstream OIDC identity-provider registry.
--
-- Each row represents one configured external IdP that sui-id can use as
-- a relying party.  The client secret is stored encrypted with the master
-- key (AAD = "federation_provider.client_secret") so a database leak never
-- exposes upstream credentials.
--
-- provision_mode controls what happens on first sign-in for a user who has
-- no existing federation_link:
--   'link_only'               — redirect to /auth/federated/link for
--                               local-password confirmation.
--   'provision_on_first_login' — create a password-less local user
--                               automatically (gated on email_verified=true;
--                               otherwise held state).
--
-- enabled = 0 means the provider is visible to admins but the "Sign in
-- with X" button is not shown and callbacks are rejected.
CREATE TABLE federation_provider (
    id                 TEXT PRIMARY KEY,
    slug               TEXT NOT NULL UNIQUE,   -- url-safe, e.g. 'google'
    display_name       TEXT NOT NULL,
    issuer             TEXT NOT NULL,          -- OIDC discovery base URL
    client_id          TEXT NOT NULL,
    client_secret_enc  BLOB,                   -- NULL for public clients
    scopes             TEXT NOT NULL DEFAULT 'openid email',
    provision_mode     TEXT NOT NULL DEFAULT 'link_only'
                       CHECK (provision_mode IN ('link_only','provision_on_first_login')),
    enabled            INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
) STRICT;

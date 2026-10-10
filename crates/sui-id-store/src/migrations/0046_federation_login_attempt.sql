-- RFC 096-B1 stage 1: RFC 096 :577-589's `federation_login_attempt`, one of
-- M2a's required controls and absent until now. This migration creates the
-- schema only -- creating a row is stage 2, claiming one is stage 3.
--
-- Status is forward-only (RFC 096's own text): pending -> exchanging ->
-- {completed, failed}; no attempt returns to pending once claimed. The
-- trailing CHECK makes `claimed_at` and `status` agree on *whether* the row
-- has been claimed rather than relying on every future writer to keep them
-- in sync by convention -- they are not redundant (`claimed_at` carries
-- *when*, `status` carries *which state*), but exactly one of "pending and
-- never claimed" or "non-pending and claimed" is a valid row.
--
-- No FOREIGN KEY on provider_id to federation_provider(id), unlike
-- federation_link's own `ON DELETE CASCADE` to the same table, and unlike
-- login_pending_mfa/webauthn_pending's CASCADE to users(id) (migrations
-- 0003, 0004): this table's purpose (RFC 096 :580-581) is for an attempt to
-- stay identifiable as superseded even after the provider's config changes,
-- and a CASCADE on provider deletion would instead make a still-
-- identifiable historical row vanish along with the provider -- exactly
-- what the version/generation columns exist to prevent. Matches
-- audit_log's own precedent (0001_initial.sql: no FK on `actor`/`target`)
-- for the same underlying reason: a historical/security record whose own
-- integrity must not be entangled with the lifecycle of what it describes.
-- Provider and version/generation agreement are re-checked at claim time by
-- the application (RFC 096's own exchange step, stage 3), which already has
-- to read all three off the row to do that matching -- a bare FK on
-- provider_id alone could not express the version/generation agreement
-- anyway, so it would add nothing a CASCADE wouldn't cost the
-- identifiability for.
CREATE TABLE IF NOT EXISTS federation_login_attempt (
    id                             TEXT    PRIMARY KEY,
    provider_id                    TEXT    NOT NULL,
    provider_config_version        INTEGER NOT NULL
                                        CHECK (provider_config_version >= 0),
    provider_activation_generation INTEGER NOT NULL
                                        CHECK (provider_activation_generation >= 0),

    -- RFC 096 :587-588: "Hashes are raw fixed-size values" -- SHA-256, 32
    -- raw bytes, not hex TEXT. The length CHECKs match 0022_boolean_checks.
    -- sql's own `CHECK (length(user_uuid) = 36)` idiom.
    state_sha256            BLOB NOT NULL UNIQUE
                                 CHECK (length(state_sha256) = 32),
    nonce_sha256            BLOB NOT NULL
                                 CHECK (length(nonce_sha256) = 32),
    browser_binding_sha256  BLOB NOT NULL
                                 CHECK (length(browser_binding_sha256) = 32),

    -- XChaCha20-Poly1305 sealed (nonce || ciphertext || tag) -- the
    -- project's `_enc`-suffixed-column format (0001_initial.sql's own
    -- header comment), despite this one column keeping the RFC's own
    -- `_sealed` name (:585) verbatim rather than being renamed to match,
    -- since it is the identical physical format. The AAD (attempt id,
    -- provider id, config version, activation generation -- :586-587) is
    -- not itself a column: every one of those values already lives on this
    -- same row, so stage 2's sealer and stage 3's opener reconstruct it
    -- from the row rather than duplicating it inside the envelope. No
    -- length CHECK here: unlike the fixed 32-byte hashes above, a PKCE
    -- verifier's sealed length varies with its plaintext length.
    pkce_verifier_sealed    BLOB NOT NULL,

    exact_redirect_uri      TEXT NOT NULL,
    -- RFC 096 :569-570: optional, parsed and canonicalised before storage;
    -- absent means "use the fixed local post-login target," never NULL
    -- reflected back to the caller as a raw value.
    next_path               TEXT,

    created_at   TEXT NOT NULL,
    expires_at   TEXT NOT NULL,

    status       TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'exchanging', 'completed', 'failed')),
    claimed_at   TEXT,

    CHECK (
        (status = 'pending' AND claimed_at IS NULL)
        OR (status != 'pending' AND claimed_at IS NOT NULL)
    )
) STRICT;

-- RFC 096: "Completed/failed/expired attempts are periodically deleted
-- after 24 hours" -- the same expiry-purge shape as auth_codes (migration
-- 0001) and login_pending_mfa/webauthn_pending (migrations 0003, 0004),
-- indexed the same way.
CREATE INDEX IF NOT EXISTS idx_federation_login_attempt_expires_at
    ON federation_login_attempt(expires_at);

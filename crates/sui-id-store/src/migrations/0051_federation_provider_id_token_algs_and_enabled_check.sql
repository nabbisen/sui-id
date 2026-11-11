-- MIGRATION:FK_DISABLE_REQUIRED
-- RFC 096-B2 stage 2a: `federation_provider` gains `id_token_algs`, and a
-- table `CHECK` that makes "enabled" mean "validated" at the database
-- layer, not merely by convention.
--
-- A table rebuild, not a plain `ALTER TABLE ADD COLUMN`: SQLite cannot
-- add a `CHECK` to columns that already exist (`config_version`,
-- `activation_generation`, `validation_status`, and the six `evidence_*`
-- columns, all from migrations 0050), only to a column it is adding in
-- the same statement -- 0050's own comment measured exactly this
-- boundary for a self-referential, single-column `CHECK`; this `CHECK`
-- spans eight columns at once, well outside it. `0022_boolean_checks.sql`
-- is the precedent for the evacuation technique below: `CREATE TABLE
-- _federation_provider_new`, `INSERT ... SELECT`, `DROP TABLE`, `ALTER
-- TABLE ... RENAME TO`.
--
-- The leading `MIGRATION:FK_DISABLE_REQUIRED` marker (read by
-- `migrations.rs`'s `apply_migration`, `:611`) makes the runner set
-- `PRAGMA foreign_keys = OFF` *before* this migration's own transaction
-- starts -- the only place SQLite honours that pragma. Without it, the
-- `DROP TABLE federation_provider` below would cascade through
-- `federation_link`'s `FOREIGN KEY (provider_id) REFERENCES
-- federation_provider (id) ON DELETE CASCADE` (`0038_federation_link.
-- sql:22`) and delete every link in the database. The runner re-enables
-- enforcement and runs `PRAGMA foreign_key_check` after `COMMIT`; an FK
-- violation there aborts startup.

CREATE TABLE _federation_provider_new (
    id                 TEXT PRIMARY KEY,
    slug               TEXT NOT NULL UNIQUE,
    display_name       TEXT NOT NULL,
    issuer             TEXT NOT NULL,
    client_id          TEXT NOT NULL,
    client_secret_enc  BLOB,
    scopes             TEXT NOT NULL DEFAULT 'openid email',
    provision_mode     TEXT NOT NULL DEFAULT 'link_only'
                       CHECK (provision_mode IN ('link_only','provision_on_first_login')),
    enabled            INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL,
    allowed_origins TEXT NOT NULL DEFAULT '',
    config_version INTEGER NOT NULL DEFAULT 0
        CHECK (config_version >= 0),
    activation_generation INTEGER NOT NULL DEFAULT 0
        CHECK (activation_generation >= 0),
    validation_status TEXT NOT NULL DEFAULT 'requires_review'
        CHECK (validation_status IN ('requires_review', 'validated')),
    evidence_config_version INTEGER
        CHECK (evidence_config_version IS NULL OR evidence_config_version >= 0),
    evidence_activation_generation INTEGER
        CHECK (evidence_activation_generation IS NULL OR evidence_activation_generation >= 0),
    evidence_policy_digest BLOB
        CHECK (evidence_policy_digest IS NULL OR length(evidence_policy_digest) = 32),
    evidence_observed_at TEXT
        CHECK (evidence_observed_at IS NULL OR strftime('%s', evidence_observed_at) IS NOT NULL),
    evidence_metadata_fingerprint BLOB
        CHECK (evidence_metadata_fingerprint IS NULL OR length(evidence_metadata_fingerprint) = 32),
    evidence_jwks_fingerprint BLOB
        CHECK (evidence_jwks_fingerprint IS NULL OR length(evidence_jwks_fingerprint) = 32),
    -- RFC 096 `:360`: `id_token_algs`, new this stage. Nullable --
    -- `NULL` means "not recorded"; RFC 096 `:1027`'s own upgrade rule
    -- ("writes no guessed ... algorithm") is why every row this
    -- migration copies gets `NULL` here, never a guessed value. The
    -- `CHECK` only refuses an empty string -- the dispatch's own rule:
    -- "the typed decoder is the authority, and a stored value it
    -- refuses makes the row runtime-ineligible, never a crash" (part D
    -- reads it back through `federation_policy::FederationTrustPolicy`,
    -- which refuses a malformed non-empty value there, not here).
    id_token_algs TEXT
        CHECK (id_token_algs IS NULL OR id_token_algs != ''),
    -- RFC 096 `:1034-1035`: "an enabled legacy-shaped or unvalidated row
    -- is runtime-ineligible" -- held by the database itself now, not by
    -- every writer remembering it. Written exactly as the dispatch's
    -- own expression, not a prose restatement of it. C17 enable (S4)
    -- stores evidence "bound to the new generation" (RFC 094 `:518-
    -- 520`), so these equalities hold for every row C17 will ever
    -- produce; nothing before S4 can write `validated` at all.
    CHECK (
        enabled = 0 OR (
            validation_status = 'validated'
            AND id_token_algs IS NOT NULL
            AND evidence_config_version = config_version
            AND evidence_activation_generation = activation_generation
            AND evidence_policy_digest IS NOT NULL
            AND evidence_observed_at IS NOT NULL
            AND evidence_metadata_fingerprint IS NOT NULL
            AND evidence_jwks_fingerprint IS NOT NULL
        )
    )
) STRICT;

-- Every row copied with `enabled = 0`: no row already in this table can
-- meet the invariant above, because nothing before this stage has ever
-- written `validation_status = 'validated'` -- a row that is enabled
-- today can only be one C16 created after migration 0050 shipped, on an
-- unreleased build (0050 itself unconditionally disables every existing
-- row; C16 does not re-enable one). Copying such a row as enabled would
-- fail the `CHECK` above and abort this migration outright, which is
-- exactly why it is not attempted: `0` is written explicitly here rather
-- than carried over from the source column.
INSERT INTO _federation_provider_new
SELECT id, slug, display_name, issuer, client_id, client_secret_enc,
       scopes, provision_mode, 0, created_at, updated_at,
       allowed_origins, config_version, activation_generation, validation_status,
       evidence_config_version, evidence_activation_generation, evidence_policy_digest,
       evidence_observed_at, evidence_metadata_fingerprint, evidence_jwks_fingerprint,
       NULL
FROM federation_provider;

DROP TABLE federation_provider;
ALTER TABLE _federation_provider_new RENAME TO federation_provider;

-- No separate `CREATE INDEX` to restore: 0037/0045/0050 (read in full,
-- not assumed) never added one beyond `slug`'s own `UNIQUE` column
-- constraint, which the `CREATE TABLE` above already repeats and SQLite
-- backs with its own automatic index.

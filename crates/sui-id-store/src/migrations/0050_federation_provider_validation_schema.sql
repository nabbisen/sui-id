-- RFC 096-B2 stage 1: the columns RFC 094's accepted C17/C18/C23 amendment
-- (rfcs/done/094-transactional-audit-registry.md:526-531) and architecture.md
-- (:236-250) specify for `federation_provider`, and the upgrade rule RFC 096
-- :1018-1036 requires when they are added to a database that already has
-- provider rows, links, and in-flight federated-login state.
--
-- No MIGRATION:FK_DISABLE_REQUIRED marker, and no table rebuild (0022's
-- evacuation technique, which 0047/0048 are the most recent precedent for
-- reading before touching this table): every CHECK added below refers only
-- to the column it is attached to, never to another column on the same
-- row, which is exactly the one restriction SQLite's own `ALTER TABLE ...
-- ADD COLUMN ... CHECK (...)` places on adding a checked column without a
-- rebuild. Measured directly against this project's own bundled SQLite
-- (3.54.1, RFC 143 stage 5), not assumed from the general rule: a column
-- added this way on a table with existing rows backfills every existing
-- row from the ADD COLUMN's own DEFAULT, and the CHECK is then enforced
-- the same as if the column had always existed.
--
-- ─── § 1: the trust-policy / activation columns ──────────────────────────
--
-- `config_version` and `activation_generation`: RFC 094 :528's "canonical
-- decimal text ... strictly parse to u64". 0046/0047 (federation_login_
-- attempt.provider_config_version/provider_activation_generation,
-- federation_mfa_pending's and federation_webauthn_ceremony's own copies)
-- already made this exact representational choice for the *copies* of
-- these two counters every other federation table carries, and this
-- stage's own dispatch requires the provider's own columns to "compare
-- exactly" against them -- so this is not a fresh choice, it is using the
-- same one again: `INTEGER NOT NULL CHECK (>= 0)`. A SQLite `INTEGER`
-- column physically cannot hold a non-canonical decimal (leading zeros,
-- whitespace, a sign on zero, anything non-digit) -- only `CAST`-able
-- integer values -- so "canonical decimal" is met by the column's type
-- alone, with no extra TEXT-form CHECK needed. The *range* this
-- represents is `0..=i64::MAX` (SQLite's `INTEGER` is signed 64-bit; there
-- is no native `u64` column type), not the full `u64` range -- the same
-- bound 0046/0047 already accepted. `i64::MAX` is therefore the concrete
-- value RFC 094 :528's "maximum state ... fails closed" means here: no
-- row can ever reach it through the checked increments C17/C23/C18 (S2+)
-- perform, because an increment that would exceed it must refuse rather
-- than wrap -- a property of that future Rust code, not of this column,
-- which merely cannot represent anything past it in the first place.
-- New and migrated rows both start at `0` (RFC 094 :528; RFC 096
-- :1024-1029 point 2) -- the `DEFAULT 0` below is what makes every
-- existing row's backfill satisfy that without a separate UPDATE.
ALTER TABLE federation_provider
    ADD COLUMN config_version INTEGER NOT NULL DEFAULT 0
        CHECK (config_version >= 0);

ALTER TABLE federation_provider
    ADD COLUMN activation_generation INTEGER NOT NULL DEFAULT 0
        CHECK (activation_generation >= 0);

-- `validation_status`: RFC 096 :1024-1029 point 2 sets every provider to
-- `requires_review` on upgrade; `validated` is the state a successful C17
-- enable (S4) moves a row to, and this stage never writes it. The
-- `DEFAULT 'requires_review'` below is what makes every existing row's
-- backfill already satisfy that, the same way `config_version`'s `DEFAULT
-- 0` does above -- no separate UPDATE needed for this column either.
ALTER TABLE federation_provider
    ADD COLUMN validation_status TEXT NOT NULL DEFAULT 'requires_review'
        CHECK (validation_status IN ('requires_review', 'validated'));

-- ─── § 2: the preflight-evidence columns ──────────────────────────────────
--
-- RFC 094 :506-512's binding list for `ValidatedProviderPreflight` (restated
-- at architecture.md :238-241, :300-306): "provider ID/version, the exact
-- disabled-state activation generation, a digest of the complete durable
-- policy, trusted observed_at, and bounded validated metadata/key-set
-- fingerprints." Provider ID is the row itself, so this adds the other
-- five as nullable columns with no writer of their own: RFC 094 :506 says
-- preflight "has no separately authoritative durable state", and this
-- stage's own dispatch says they are "written only by C17 enable" (S4).
-- Every one of the six `ALTER TABLE` statements in this section leaves
-- every existing row's new column `NULL` (SQLite's own default for a
-- nullable column with no explicit `DEFAULT`), which is this stage's own
-- requirement -- "this stage only creates them, always NULL."
--
-- `evidence_config_version`/`evidence_activation_generation`: the same
-- `INTEGER ... CHECK (>= 0)` representation as `config_version`/
-- `activation_generation` above, nullable rather than `NOT NULL` -- the
-- generation/version C17 actually enabled *against*, which can differ
-- from the row's own live columns the moment a later C23/disable moves
-- them on, and is never read as re-enable authority (architecture.md
-- :248-249).
ALTER TABLE federation_provider
    ADD COLUMN evidence_config_version INTEGER
        CHECK (evidence_config_version IS NULL OR evidence_config_version >= 0);

ALTER TABLE federation_provider
    ADD COLUMN evidence_activation_generation INTEGER
        CHECK (evidence_activation_generation IS NULL OR evidence_activation_generation >= 0);

-- `evidence_policy_digest`: the SHA-256 digest RFC 096 :424-435 defines in
-- full (domain-separated, fixed-order binary encoding of the complete
-- trust policy). Raw 32 bytes, not hex TEXT -- the same BLOB-plus-length-
-- CHECK idiom 0046 already established for every other hash in this
-- schema ("Hashes are raw fixed-size values", RFC 096 :646) and that
-- every federation migration since has repeated rather than re-deciding.
ALTER TABLE federation_provider
    ADD COLUMN evidence_policy_digest BLOB
        CHECK (evidence_policy_digest IS NULL OR length(evidence_policy_digest) = 32);

-- `evidence_observed_at`: the trusted `observed_at` RFC 096 :414 and :441
-- bind C17's `0 <= guard_at - observed_at < 600s` guard against. Same
-- nullable-timestamp text column as every other `_at` column in this
-- schema, guarded the same way 0047's own header comment states and
-- applies to every timestamp CHECK it adds: a `strftime('%s', ...)` of an
-- unparseable value is `NULL`, and a bare `CHECK` on `NULL` passes, so the
-- guard asserts parseability explicitly rather than trusting a future
-- caller never to write something SQLite cannot read back as a time.
ALTER TABLE federation_provider
    ADD COLUMN evidence_observed_at TEXT
        CHECK (evidence_observed_at IS NULL OR strftime('%s', evidence_observed_at) IS NOT NULL);

-- `evidence_metadata_fingerprint`/`evidence_jwks_fingerprint`: RFC 096
-- :414's "bounded validated metadata/key-set fingerprints" -- two
-- distinct values (discovery metadata, and the JWKS key set), per the
-- dispatch's own "metadata/key-set" phrasing. Neither RFC 096 nor
-- architecture.md fixes how a fingerprint is computed (that is S3's own
-- design, building `ValidatedProviderPreflight` itself) -- this stage
-- fixes only the *storage* representation, and does so by reusing the
-- schema's one existing convention for "a bounded, attacker-uncontrolled
-- hash of something larger": the same 32-byte raw-BLOB SHA-256 shape as
-- `evidence_policy_digest` above and every other hash column in this
-- database. "Bounded" is what rules out storing the metadata document or
-- the JWKS body itself (unbounded, attacker/upstream-controlled text) --
-- a fixed-size digest is the natural, minimal way to satisfy that without
-- inventing a second format. If S3 settles on a different fingerprint
-- construction that is not exactly 32 bytes, these two CHECKs are the one
-- place that needs to change to match it.
ALTER TABLE federation_provider
    ADD COLUMN evidence_metadata_fingerprint BLOB
        CHECK (evidence_metadata_fingerprint IS NULL OR length(evidence_metadata_fingerprint) = 32);

ALTER TABLE federation_provider
    ADD COLUMN evidence_jwks_fingerprint BLOB
        CHECK (evidence_jwks_fingerprint IS NULL OR length(evidence_jwks_fingerprint) = 32);

-- ─── § 3: the upgrade rule itself (RFC 096 :1018-1036) ────────────────────
--
-- Point 1, "preserve provider IDs, sealed secrets and links": nothing
-- above touches `id`, `client_secret_enc`, or `federation_link` at all --
-- preserved by omission, the same way 0045's own `allowed_origins` ADD
-- COLUMN preserved every existing column it did not mention.
--
-- Point 2, "set every provider enabled=0, validation_status=
-- requires_review, both counters to 0": `validation_status`,
-- `config_version` and `activation_generation` are new columns, and
-- their `DEFAULT` above already backfills every existing row to exactly
-- that triple -- no separate UPDATE needed for any of the three. `enabled`
-- is NOT a new column (0037), so its existing value survives an ADD
-- COLUMN untouched, and must be forced to `0` explicitly:
UPDATE federation_provider SET enabled = 0;

-- Point 3, "guess nothing (origin, algorithm, auth method, evidence)":
-- nothing above writes a value to any evidence column, or to any origin/
-- algorithm/auth-method column (none of which exist yet -- out of this
-- stage's scope entirely). `updated_at` is likewise left untouched: RFC
-- 096 :1024-1029 and architecture.md :236-250 both name exactly `enabled`,
-- `validation_status` and `activation_generation` as what this upgrade
-- sets, and inventing a fourth write this migration was not asked to make
-- is its own kind of guessing.
--
-- Point 4, "invalidate legacy browser-state and pending flows without
-- unlinking users": the three non-terminal-state tables, each using that
-- table's own existing invalidation shape (measured from its repository
-- module, not reinvented here) rather than a new one:
--
-- `federation_login_attempt` (migration 0046): forward-only `pending ->
-- exchanging -> {completed, failed}`; `repos::federation_login_attempt::
-- terminalize_failed` is the table's own `pending|exchanging -> failed`
-- transition, `SET status = 'failed', claimed_at = COALESCE(claimed_at,
-- <now>) WHERE status IN ('pending', 'exchanging')` -- scoped here to
-- every such row instead of one `id`. `COALESCE` leaves an already-
-- `exchanging` row's real claim time untouched and only fills the column
-- in for a row moving straight from `pending`, which is what the table's
-- own trailing CHECK (`status != 'pending'` pairs with `claimed_at IS NOT
-- NULL`) requires of every row this leaves non-pending. The timestamp
-- format is `0016`/`0036`'s own migration-time-literal idiom
-- (`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`), not a bare `datetime('now')`,
-- so a reader comparing it against this project's chrono-produced
-- timestamps via `strftime('%s', ...)` sees the same `NOT NULL`,
-- comparable value either way.
UPDATE federation_login_attempt
   SET status = 'failed',
       claimed_at = COALESCE(claimed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
 WHERE status IN ('pending', 'exchanging');

-- `federation_webauthn_ceremony` (migration 0047): no `status` column at
-- all -- a ceremony is in-flight exactly while its row exists. Every row
-- still present here is, by the table's own composite FOREIGN KEY onto
-- `federation_mfa_pending (id, provider_id, provider_config_version,
-- provider_activation_generation, user_id, expires_at)`, bound to a
-- parent row that is, at this point in the migration, still `pending`
-- (nothing before this statement has changed any `federation_mfa_pending`
-- row), because the application never leaves a ceremony attached to a
-- terminal parent (`repos::federation_webauthn_ceremony::
-- consume_within_tx`/`delete_by_pending_id_within_tx` always delete the
-- ceremony in the same transaction that terminalizes its parent, migration
-- 0047's own comment on the FK already establishes the second half: a
-- ceremony with no live, matching parent cannot be stored at all). So
-- every remaining row is in-flight, and this table's own existing
-- invalidation -- `DELETE FROM federation_webauthn_ceremony WHERE ...`,
-- the exact shape both repository functions above already use, generalised
-- from "one id"/"one pending_id" to "every row" -- is a plain unconditional
-- delete:
DELETE FROM federation_webauthn_ceremony;

-- `federation_mfa_pending` (migration 0047): only `status = 'pending'` is
-- non-terminal (`completed`, `exhausted` and `invalidated` all already
-- are); its own `invalidate_within_tx` is `SET status = 'invalidated',
-- continuation_sealed = NULL WHERE id = ?1 AND status = 'pending'`, reused
-- here the same way, scoped to every such row instead of one `id`. This
-- runs after the `DELETE` above, not before, so that statement's own read
-- of "still pending" still sees every row this UPDATE is about to
-- terminalize in its original state -- though, as the comment there
-- explains, the order cannot actually matter: the composite FOREIGN KEY
-- a ceremony depends on names six columns, and neither `status` nor
-- `continuation_sealed` is one of them, so this UPDATE could not have
-- orphaned a ceremony row even run first.
UPDATE federation_mfa_pending
   SET status = 'invalidated',
       continuation_sealed = NULL
 WHERE status = 'pending';

-- Point 5 in architecture.md's own numbering ("reports each provider for
-- operator configuration") is S5's admin surface, not this stage's: §D of
-- this stage's own dispatch instead adds one startup warning per
-- `requires_review` provider, in `crates/sui-id/src/runtime/startup.rs`,
-- not in this migration.

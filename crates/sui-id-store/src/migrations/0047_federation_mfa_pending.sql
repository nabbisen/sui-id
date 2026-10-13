-- RFC 096-B1 stage 8: RFC 096 :728-738's `federation_mfa_pending` and
-- `federation_webauthn_ceremony`, schema only -- the first row is stage 9
-- (F02), claimed/promoted by stages 10-11 (F03/F06). Neither table is
-- written here, and there is nothing to purge yet (:734's 24-hour
-- terminal-row purge is stage 9's, with the first writer).
--
-- Read 0046_federation_login_attempt.sql first: STRICT, BLOB digests with
-- a length CHECK rather than hex TEXT, and a table-level CHECK pairing a
-- nullable column with `status` are all reused verbatim below.
--
-- One rule stated once and held by both tables below: no timestamp column
-- may hold a value SQLite cannot parse. `strftime('%s', <unparseable>)` is
-- NULL, and a SQLite CHECK passes on NULL -- so every CHECK comparing two
-- timestamps also asserts `strftime('%s', ...) IS NOT NULL` on each one
-- first, to make the comparison two-valued rather than silently
-- succeeding on garbage. Demonstrated, not assumed: an unparseable
-- `created_at`/`expires_at` and an empty string were all accepted before
-- this guard was added.

CREATE TABLE IF NOT EXISTS federation_mfa_pending (
    id                             TEXT    PRIMARY KEY,

    -- ":730 provider/version/activation generation/link/user" -- the
    -- identity this challenge is bound to.
    provider_id                    TEXT    NOT NULL,
    provider_config_version        INTEGER NOT NULL
                                        CHECK (provider_config_version >= 0),
    provider_activation_generation INTEGER NOT NULL
                                        CHECK (provider_activation_generation >= 0),
    user_id                        TEXT    NOT NULL,

    -- ":730 sealed continuation" -- what a successful second factor
    -- promotes into (:721-722: "provider/link identity, next target,
    -- creation/expiry, and one-time consumption"), an opaque AAD-sealed
    -- blob rather than separate plaintext columns -- the same `_sealed`
    -- idiom as federation_login_attempt.pkce_verifier_sealed. NULL once
    -- the row is terminal (the trailing CHECK below): ":733-734 terminal
    -- rows retain no usable continuation" is enforced the same way 0046
    -- pairs `claimed_at` with `status`. A row CHECK cannot stop a later
    -- UPDATE from reviving a terminal row (0046's own stage 1 review
    -- measured that limit directly, by walking a claimed row back to
    -- pending), so single-use/no-revival-past-terminal stays a
    -- code-level property, owed by whichever stage writes the UPDATE
    -- that terminalizes a row (stage 10, F03).
    continuation_sealed             BLOB,
    -- ":730 next target" -- optional redirect target, the same plain
    -- (unsealed) column and the same "absent means the fixed default
    -- target" convention as federation_login_attempt.next_path. Not
    -- part of "usable continuation": a terminal row keeping its
    -- next_path is harmless on its own, since nothing can act on it
    -- without continuation_sealed.
    next_path                       TEXT,

    -- ":730-731 browser and CSRF binding digests" -- two separate
    -- SHA-256 digests, the same BLOB-not-hex-TEXT-plus-length-CHECK
    -- idiom as federation_login_attempt's three hash columns.
    browser_binding_sha256          BLOB    NOT NULL
                                        CHECK (length(browser_binding_sha256) = 32),
    csrf_binding_sha256             BLOB    NOT NULL
                                        CHECK (length(csrf_binding_sha256) = 32),

    -- ":731 primary_method=Fed". A one-valued column, kept rather than
    -- dropped: the RFC states it as a stored fact of the row, not a
    -- derived one, and the cost of a pinned CHECK over inlining the
    -- literal at every read site is one line. A future second value
    -- would mean a *different* upstream-identity technology sharing this
    -- same pending-MFA shape (RFC 096 is OIDC-only; a hypothetical
    -- future federation protocol with the same bindings and expiry
    -- mechanics would be the only thing that earns one) -- at that point
    -- this CHECK widens to an IN-list and nothing else about the table
    -- changes. It is not a placeholder for "any session's primary
    -- method": login_pending_mfa (migration 0003) already owns that case,
    -- and this table is deliberately not a generalisation of it
    -- (:728-729 says so explicitly).
    primary_method                  TEXT    NOT NULL DEFAULT 'fed'
                                        CHECK (primary_method = 'fed'),

    -- ":731 failure_count constrained 0-5" -- the schema's half of
    -- RFC 096 :740-751's state machine. The *semantics* (1-4 is
    -- RejectedStillPending, exactly 5 is AttemptsExhausted, no sixth
    -- increment) are stage 10's (F03); this CHECK only narrows the
    -- representable range to what that code is allowed to produce.
    failure_count                   INTEGER NOT NULL DEFAULT 0
                                        CHECK (failure_count BETWEEN 0 AND 5),

    -- ":731-732 status in pending|completed|exhausted|invalidated".
    status                          TEXT    NOT NULL DEFAULT 'pending'
                                        CHECK (status IN ('pending', 'completed', 'exhausted', 'invalidated')),

    created_at                      TEXT    NOT NULL,
    -- ":732-733 expires_at=created_at+300 seconds". Not string or
    -- datetime() equality: chrono's rusqlite ToSql formats a
    -- DateTime<Utc> as "YYYY-MM-DD HH:MM:SS.ffffff+00:00", and SQLite's
    -- datetime() normalises away both the fractional part and the
    -- offset, so the two sides would almost never compare equal as
    -- strings even for a correctly-computed row (checked directly
    -- against this project's exact stored format, not assumed).
    -- Whole-second Unix-epoch equality avoids that, and is guarded on
    -- both sides by `IS NOT NULL` per the rule stated at the top of this
    -- file -- without it, an unparseable value on either side makes the
    -- whole comparison NULL, and a CHECK that evaluates to NULL is
    -- treated as satisfied, not refused (demonstrated: a garbage
    -- created_at/expires_at pair, and an empty-string pair, were both
    -- accepted before this file added the two `IS NOT NULL` guards).
    expires_at                      TEXT    NOT NULL
                                        CHECK (
                                            strftime('%s', created_at) IS NOT NULL
                                            AND strftime('%s', expires_at) IS NOT NULL
                                            AND CAST(strftime('%s', expires_at) AS INTEGER)
                                              = CAST(strftime('%s', created_at) AS INTEGER) + 300
                                        ),

    -- ":733-734 'terminal rows retain no usable continuation'" -- same
    -- shape and same treatment as 0046's `status`/`claimed_at` CHECK: a
    -- pending row must still have something to promote into; a terminal
    -- row must not.
    CHECK (
        (status = 'pending' AND continuation_sealed IS NOT NULL)
        OR (status != 'pending' AND continuation_sealed IS NULL)
    ),

    -- ":730/:722 'link identity'" -- (user_id, provider_id) *names* a
    -- federation_link row (migration 0038's own PRIMARY KEY), but naming
    -- one does not make it exist: without this FK, a pending row was
    -- storable for a pair that was never linked, and survived an unlink.
    -- Binding it here is the same FOREIGN KEY-as-live-operational-state
    -- reasoning this table already applies elsewhere (below, and in the
    -- ceremony table) carried one step further: a user who revokes a
    -- provider connection mid-challenge must not leave a pending row that
    -- still promotes to a session, any more than a deleted user or a
    -- deleted provider should. This one FK is strictly stronger than
    -- separate FKs on provider_id -> federation_provider and
    -- user_id -> users would be (both of those cascades still reach,
    -- transitively, since federation_link itself cascades on both --
    -- migration 0038): it additionally refuses a pending row for a pair
    -- that was never linked, and cascades the pending row (and, through
    -- it, its ceremony) on an unlink specifically, which a pair of
    -- single-column FKs could not express. Checked against RFC 096
    -- :693-722 before relying on it: the only route to this table's
    -- pending-challenge branch (:719-722, "a user with local MFA enters a
    -- durable/bounded pending challenge") is an existing, already-linked
    -- identity -- an unknown identity's two branches (:699-712) either
    -- creates no link and denies (`link_only`) or provisions a
    -- passwordless user with no local MFA to challenge (
    -- `provision_on_first_login`, amended RFC 094 F04) -- so no RFC path
    -- reaches this table before its link exists. `federation_link::
    -- delete` (repos/federation_link.rs:162, self-service unlink) has no
    -- caller today -- grepped, not assumed -- so the unlink cascade is a
    -- property for when it is wired, not one exercised yet;
    -- `federation_provider::delete` (repos/federation_provider.rs:207) is
    -- live today.
    FOREIGN KEY (user_id, provider_id)
        REFERENCES federation_link (user_id, provider_id)
        ON DELETE CASCADE
) STRICT;

-- Same purge-by-expiry indexing precedent as federation_login_attempt
-- (migration 0046) and login_pending_mfa/webauthn_pending (migrations
-- 0003, 0004).
CREATE INDEX IF NOT EXISTS idx_federation_mfa_pending_expires_at
    ON federation_mfa_pending(expires_at);

-- ":736-737 'exact provider/version/activation generation/link/user
-- binding'" on the ceremony table, and ":737-738 no-later expiry" than
-- its parent, are both cross-table invariants -- a SQLite CHECK sees one
-- row, never another table's, so neither is expressible as a plain
-- CHECK. But a composite FOREIGN KEY compares two tables by construction,
-- and SQLite's own parent-key rule (the referenced columns must be
-- covered by a UNIQUE index or the PRIMARY KEY) is what this index gives
-- it: not a new invariant of its own, only what the FOREIGN KEY on
-- federation_webauthn_ceremony below needs to exist.
CREATE UNIQUE INDEX IF NOT EXISTS ux_federation_mfa_pending_binding
    ON federation_mfa_pending (
        id, provider_id, provider_config_version,
        provider_activation_generation, user_id, expires_at
    );

-- RFC 096 :735-738's `federation_webauthn_ceremony`: one in-flight
-- WebAuthn challenge bound to exactly one `federation_mfa_pending` row --
-- the federated analogue of webauthn_pending (migration 0004), except
-- that table's pending-state and ceremony-state live in the *same* row,
-- and the RFC asks for two rows here instead, linked by `pending_id`.
CREATE TABLE IF NOT EXISTS federation_webauthn_ceremony (
    id                              TEXT    PRIMARY KEY,

    -- ":736 unique parent-pending key" -- at most one ceremony per
    -- pending row at a time.
    pending_id                      TEXT    NOT NULL UNIQUE,

    -- ":736-737 'exact provider/version/activation generation/link/user
    -- binding'" -- these four columns, plus `pending_id` and
    -- `pending_expires_at` below, are the composite FOREIGN KEY's child
    -- side (trailing). Storing them again here rather than reading only
    -- through the join is what lets a consumer check this row's own
    -- binding directly, and the FOREIGN KEY is what makes that stored
    -- copy *agree* with the parent row rather than merely exist:
    -- `provider_id`/`user_id` no longer carry their own REFERENCES to
    -- federation_provider/users, because the composite FK below is
    -- strictly stronger than either -- it requires the exact parent row,
    -- which already guarantees those two are valid.
    provider_id                     TEXT    NOT NULL,
    provider_config_version         INTEGER NOT NULL
                                        CHECK (provider_config_version >= 0),
    provider_activation_generation  INTEGER NOT NULL
                                        CHECK (provider_activation_generation >= 0),
    user_id                         TEXT    NOT NULL,

    -- ":737 sealed ceremony state" -- webauthn-rs's own
    -- `PasskeyAuthentication` value, the same payload
    -- webauthn_pending.state_json holds, but sealed here rather than
    -- plain JSON: :737 asks for it sealed, and this is the first
    -- federation table to hold one of webauthn-rs's in-flight structures
    -- rather than a hash or a path.
    ceremony_state_sealed            BLOB    NOT NULL,

    -- ":737 RP/origin" -- the relying-party id and origin the ceremony
    -- was started against, stored rather than assumed, so a later
    -- config change cannot silently re-target an in-flight ceremony.
    rp_id                            TEXT    NOT NULL,
    origin                           TEXT    NOT NULL,

    created_at                       TEXT    NOT NULL
                                        CHECK (strftime('%s', created_at) IS NOT NULL),

    -- ":737-738 'no-later expiry'" than the parent row. A deliberate
    -- denormalization, not a convenience: this column exists so the
    -- cross-table invariant below can be a single-row CHECK instead of a
    -- convention, and the composite FOREIGN KEY is what makes the copy
    -- *truthful* -- a row whose pending_expires_at is not the parent's
    -- real, current expires_at has no matching parent tuple and is
    -- refused, exactly like the identity columns above. Neither half
    -- works without the other: the CHECK alone would trust a caller-
    -- supplied value, and the FK alone could not express "no later than",
    -- only "matches exactly".
    pending_expires_at               TEXT    NOT NULL,
    expires_at                       TEXT    NOT NULL
                                        CHECK (
                                            strftime('%s', expires_at) IS NOT NULL
                                            AND strftime('%s', pending_expires_at) IS NOT NULL
                                            AND CAST(strftime('%s', expires_at) AS INTEGER)
                                              <= CAST(strftime('%s', pending_expires_at) AS INTEGER)
                                        ),

    -- One further property falls out of this FK rather than being
    -- separately asked for, and it is one RFC 096 wants: while a
    -- ceremony exists, `UPDATE federation_mfa_pending SET expires_at = …`
    -- is refused, because the parent's old (id, …, expires_at) tuple the
    -- ceremony's pending_expires_at still names would no longer exist --
    -- a pending row cannot be extended out from under a live ceremony.
    -- Terminalizing the parent (`status`, `failure_count`,
    -- `continuation_sealed`) is unaffected: none of those columns are
    -- part of this FK.
    FOREIGN KEY (
        pending_id, provider_id, provider_config_version,
        provider_activation_generation, user_id, pending_expires_at
    )
        REFERENCES federation_mfa_pending (
            id, provider_id, provider_config_version,
            provider_activation_generation, user_id, expires_at
        )
        ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_federation_webauthn_ceremony_expires_at
    ON federation_webauthn_ceremony(expires_at);

-- :738 "generic password/WebAuthn pending rows cannot reference it":
-- login_pending_mfa (migration 0003) and webauthn_pending (migration
-- 0004) have no column of any kind that names another pending-MFA or
-- ceremony row, and this migration adds none to either -- so there is
-- currently nothing for either table to put such a reference in. That is
-- an absence, not a schema-level prohibition: nothing stops some future
-- migration from adding, say, a webauthn_pending.federation_ceremony_id
-- column, and SQLite has no constraint that forbids a column from
-- existing. `tests_rfc096_b1_stage8.rs`'s
-- `the_two_generic_pending_tables_gain_no_federation_reference` test pins
-- both tables' exact column sets so that migration would fail a named
-- test rather than rely on a reviewer remembering this comment -- the
-- test enforces what this comment says; it does not replace saying it.

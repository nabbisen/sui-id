# RFC 142 stage 1 — measurement record

**Dispatch.** [stage1-count-the-ranges-2026-10-29.md](./stage1-count-the-ranges-2026-10-29.md)
**Landed.** 2026-10-29. Measurement only — nothing in `crates/`
changed, as the dispatch required.
**Reviewed and accepted by the architect**, who independently
re-derived every load-bearing figure below: the 37-version set, the
12 unshipped migrations, the contiguity of `MIGRATIONS`, the
`NOT NULL ⇒ DEFAULT` invariant across all 47 `ADD COLUMN`s, the
13-table-to-boundary map, `consents` being unreferenced,
`append_within_tx`'s dependency on schema 9, and the `0.29.7` blob
census. All matched.

**One correction was made in review, and the body below predates it:**
**`revoked_access_tokens` is now in the seeded set** (14 tables, not
13), because losing it across an upgrade silently un-revokes every
access token still inside its validity window. It costs one boundary
at `0005`, splitting R3 into `{4}` and `{5,6,7,9–13}`. **The corpus
therefore needs 10 seed scripts, not 9.** The RFC's §Settled carries
the amended set; §5 of this report is superseded on that number alone.

---


**Handoff.** `rfcs/handoffs/142-upgrading-is-the-least-tested-thing-we-do/stage1-count-the-ranges-2026-10-29.md`
**RFC.** `rfcs/accepted/142-upgrading-is-the-least-tested-thing-we-do.md`, Accepted.
**Baseline.** `25ea2de` (`git log -1` at start).
**Candidate.** None — this stage writes no code. Nothing in `crates/` changed; nothing to copy back; working tree is clean.

**Headline: 9 seed scripts cover all 37 shipped schema versions** (10
structural shape-ranges exist, but one has zero shipped starting
points). This does not make the "cover all 37" decision look wrong —
it is the strongest confirmation available that ranges are far fewer
than versions, exactly as the RFC's revised recommendation assumed.

---

## 1 — the covered version set, re-derived independently

Mapped all 137 tags to the highest `version:` entry each one's
`crates/sui-id-store/src/migrations.rs` carries, via `git show
<tag>:crates/sui-id-store/src/migrations.rs` (the path is stable back
to the first commit — confirmed with `git log --follow`).

**37 distinct shipped schema versions**, matching the RFC's own
figure exactly:

> 1–7, 9–13, 15–18, 20–26, 28–34, 36, 38, 43, 44, 46, 48, 49

**12 of the 49 migrations never shipped alone**: 8, 14, 19, 27, 35,
**37**, 39, 40, 41, 42, 45, 47 — set difference of `1..=49` against the
37, also matching exactly. Migration 37 (`federation_provider`) is
notable: `0.76.3` shipped at schema 36 and `0.76.4` at schema 38 in the
same development cycle, so no tag ever shipped at exactly 37. This
matters below (§3, range R9).

`crates/sui-id-store/src/migrations.rs` itself was checked for gaps or
duplicates: `grep -oE "version:\s*[0-9]+"` returns exactly 49 entries,
`1..=49` in order, no duplicate. This is also the answer to §4 point 4
below.

## 2 — the seeded-table set, and why

**13 tables.** The corpus exists to prove *operator- or end-user-
authored data* survives an upgrade — not every row SQLite happens to
hold. Walked every `CREATE TABLE` across all 49 migrations
(`crates/sui-id-store/src/migrations/*.sql`) and classified each:

**Included** (durable identity, credential, or configuration data an
operator or user would be upset to lose):

| Table | Created | Why |
|---|---|---|
| `users` | 0001 | account identity |
| `credentials` | 0001 | password hash, 1:1 with a user |
| `clients` | 0001 | OAuth relying-party registrations, admin-managed |
| `signing_keys` | 0001 | token-signing material; losing it breaks every previously-issued token's verification |
| `user_totp` | 0003 | MFA secret + recovery codes |
| `user_webauthn_credentials` | 0004 | passkeys |
| `smtp_config` | 0014 | operator mail-relay configuration |
| `server_settings` | 0016 | operator-wide settings singleton |
| `user_consent` | 0025 | what a user actually granted (the live table — see below) |
| `scope_definition` | 0036 | the deployment's declared scope catalog |
| `client_registration_token` | 0036 | admin-issued, often non-expiring (`expires_at` is nullable) dynamic-registration credential |
| `federation_provider` | 0037 | admin-configured upstream OIDC provider |
| `federation_link` | 0038 | a local user's link to an external identity |

**Excluded as ephemeral / session-like** (expiring, re-derivable by
retrying the action, never a thing an operator is asked to back up):
`auth_codes`, `sessions`, `refresh_tokens`, `login_pending_mfa`,
`webauthn_pending`, `password_reset_tokens`,
`pending_settings_change` (5-minute TTL, single-use by its own design
comment), `federation_login_attempt`, `federation_mfa_pending`,
`federation_webauthn_ceremony`, `forgot_password_requests` (its own
migration comment calls the retention "transient... deleted the
moment the worker finishes").

**Excluded, flagged as judgement calls** (reasoning given because a
different call is defensible):
- `revoked_access_tokens` — a security *control* (an already-revoked
  token staying revoked), not identity/profile data, and it
  self-prunes against the token's own `exp`. Excluded as the same
  kind of thing as `sessions`/`auth_codes`, but the window it protects
  is real; the architect may want it included.
- `email_outbox` — a retry queue for outbound mail, not a durable
  record of anything. Excluded, but a password-reset email sitting
  queued at the moment of an upgrade is a real (if minor) case.

**Excluded, not a judgement call**: `sui_meta` (migration bookkeeping
itself, not data), `consents` (see finding below), `audit_log` (own
question, §4.2).

**Finding not asked for, but changes what stage 2 should seed**:
`consents` (created 0001, rebuilt 0021) has **zero production
references** anywhere in `crates/` (`grep -rn "\bconsents\b"
crates/ --include="*.rs"` outside tests/`user_consent` returns
nothing). It was superseded by `user_consent` (0025) and nothing reads
or writes it today. Seeding it would test a table no code touches —
stage 2 should not bother, and this is worth a separate note to
whoever next has free cycles, independent of this RFC.

## 3 — the range table

A migration opens a boundary **for this 13-table set** if it changes
whether an existing seed's `INSERT` would still work or still mean the
same thing, per the dispatch's own rule. Checked every `ADD COLUMN`
against this: every `NOT NULL` column added to a seeded table in this
codebase carries a `DEFAULT` (SQLite requires one for `ADD COLUMN` on
a populated table, so this is a codebase invariant, not a coincidence)
— so no plain column addition ever opens a boundary by itself.

| Range | Covered starting versions | Tables newly in scope at the range's start | Opens (file) | Closes (file) |
|---|---|---|---|---|
| R1 | 1, 2 | `users`, `credentials`, `clients`, `signing_keys` | `0001_initial.sql` | `0003_totp_mfa.sql` |
| R2 | 3 | + `user_totp` | `0003_totp_mfa.sql` | `0004_webauthn.sql` |
| R3 | 4,5,6,7,9,10,11,12,13 | + `user_webauthn_credentials` | `0004_webauthn.sql` | `0014_smtp_config.sql` |
| R4 | 15 | + `smtp_config` | `0014_smtp_config.sql` | `0016_i18n.sql` |
| R5 | 16,17,18,20,21 | + `server_settings` | `0016_i18n.sql` | `0022_boolean_checks.sql` |
| R6 | 22,23,24 | same tables, now rebuilt under boolean/consistency `CHECK`s | `0022_boolean_checks.sql` | `0025_consent.sql` |
| R7 | 25,26,28,29,30,31,32,33,34 | + `user_consent` | `0025_consent.sql` | `0036_scope_definition_and_reg_token.sql` |
| R8 | 36 | + `scope_definition`, `client_registration_token` | `0036_...sql` | `0037_federation_provider.sql` |
| **R9** | **none** | + `federation_provider` | `0037_federation_provider.sql` | `0038_federation_link.sql` |
| R10 | 38,43,44,46,48,49 | + `federation_link` | `0038_federation_link.sql` | *(none through 0049)* |

**10 structural ranges. R9 has zero shipped starting points** (§1) —
no release was ever tagged between 0037 and 0038 landing, so it needs
no seed script. **9 seed scripts actually needed.**

**The one judgement call inside this table**, not mechanical: `0043`
(`ALTER TABLE credentials DROP COLUMN must_change`) does **not** open
a boundary here, even though "a column dropped" is naively the kind of
thing that should. `must_change` had `DEFAULT 0`, no production
reader ever consulted it (the migration's own comment says so,
confirmed by `0043`'s own file), and no reasonable seed would set it
explicitly. A seed that never names the column is valid both before
and after it exists. **If this call is wrong, R10 splits into {38–42}
(covered: 38) and {43–49} (covered: 43,44,46,48,49), both non-empty —
10 seed scripts, not 9. The headline number is insensitive to this
call within one script either way.**

## 4 — the four points

### 4.1 — mechanical, or human judgement?

**Mostly mechanical, with three specific, rare exceptions that must be
flagged rather than auto-decided.**

A generator can derive range boundaries automatically from:
- `CREATE TABLE <t>` where `<t>` is in the seeded set — always opens.
- `CREATE INDEX` — never opens (confirmed: 0011, 0026, 0031, 0048 are
  index-only and correctly produce no boundary).
- `ADD COLUMN` to a table not in the seeded set — irrelevant, filtered
  by set membership.
- `ADD COLUMN ... NOT NULL DEFAULT ...` or a nullable column — never
  opens, and (per §3) this is every `ADD COLUMN` in this codebase.

**What cannot be automatic, because resolving it required judgement
in this round and will again:**
1. **Seeded-table-set membership itself.** Deciding `consents` is dead
   and `revoked_access_tokens` is a security control rather than
   profile data are judgement calls, not syntax. **A generator should
   surface "a new table was created — is it in the seeded set?" as a
   required question on every future migration that creates a table,
   not default it either way.**
2. **A new `CHECK` on an existing column (0022).** Mechanically, a new
   `CHECK` *might* not reject any seed's actual values (this
   codebase's booleans were already 0/1 before 0022 added the
   constraint that says so). I treated "a new CHECK could be
   violated" as *always* opening a boundary — a conservative proxy
   rule chosen once, by judgement, specifically so nobody has to
   re-prove non-violation by inspection years later. A generator
   should flag this and ask, not silently decide either way.
3. **`DROP COLUMN` (0043).** Resolved by judgement (§3), not syntax: a
   dropped column that had a default and no reader doesn't force a
   rewrite; a dropped column holding real data would. **Flag,
   don't auto-decide.**

So: cheap and safe to automate the common case; the three judgement
points above are exactly the ones a generator must surface as a
required decision each time they recur, not resolve silently — which
is also the answer to "which decisions need it; those are the ones
that rot."

### 4.2 — does the seeded set include `audit_log`?

**Recommendation revised after reading the real code, not just the
RFC's framing: give it one separate, minimal fixture — don't fold it
into the 13-table corpus, and don't duplicate the hash logic either.**

First pass (shape only): every `audit_log` column added across all 49
migrations (`0009`'s `prev_hash`/`hash`, `0028`'s `actor_role`) has a
default or is nullable — by the same mechanical rule as §3, `audit_log`
would cost **zero extra ranges**. That looked like a reason to include
it cheaply.

**But shape compatibility is the wrong question for this table.**
`crates/sui-id-store/src/repos/audit.rs`'s `append_within_tx` (`pub
fn`, reachable from the store crate's own test tree, same reachability
as `run_up_to`) is the real production writer and computes a genuine
`SHA-256(prev_hash || canonical_row_bytes)` chain — reusing it, rather
than duplicating `compute_hash` (private to that module, and
duplicating it risks exactly the drift this round's own `check-
publish-preconditions.py` round warned about). **But `append_within_tx`
unconditionally writes `prev_hash`/`hash`, which do not exist before
schema 9.** Calling it against a database seeded at a starting version
below 9 (covered: 1,2,3,4,5,6,7) would fail outright — "no such
column." A seed at those starting versions would need a second, raw
`INSERT` shape with no hash columns at all, which `append_within_tx`
cannot produce.

So `audit_log`'s real behaviour bifurcates **once**, at migration 9 —
independent of the 13-table range structure above — not because its
own shape is unstable, but because the one production function worth
reusing only works on one side of that line. **Recommendation: one
fixture using `append_within_tx` for a starting version ≥ 9, and one
using a plain `INSERT` (no hash columns) for a starting version < 9 —
two fixtures, not nine, and not zero.** Threading it through all 9
general ranges would re-test the same already-proven transition nine
times for no further coverage; a dedicated RFC-125 test suite already
covers the hash chain's own correctness, which is not this RFC's job
to re-prove.

### 4.3 — the `0.29.7` lineage, confirmed

Independently re-verified, not taken on the RFC's word:

```
$ git log --oneline --follow -- crates/sui-id-store/src/migrations/0021_schema_invariants.sql
8b4fadf Bug fix: migration 0021 FK cascade data loss (critical)
68ad4f3 RFC 021 to raise DB-layer guarantees
```

Hashed the blob at every one of the 137 tags for that one path:

- **43 tags** have no file at all (pre-`0021`, i.e. schema ≤ 20 —
  consistent with §1).
- **Exactly one tag, `0.29.7`**, has the original blob
  (`68ad4f3`'s content, `md5 136d5aea...`).
- **93 tags** (`0.29.8` onward) have the fixed blob (`8b4fadf`'s
  content, `md5 5c43d7b7...`, identical to `HEAD` today).

**Confirmed: the only divergent starting state is schema 21 reached
through `0.29.7`, and it is exactly one tag — not a range of tags, not
a guess.** `0.29.6` has no `0021` file at all (max schema 20, matching
§1's tag-version table), so there is no earlier tag that could also
carry the broken text. The hand-built fixture is exactly one fixture,
confirmed rather than assumed.

As a side check (not asked for, but the same method applies and the
RFC's "faithful only while SQL is immutable" claim deserves the same
scrutiny as its headline number): independently re-ran `git log
--oneline --follow` against all 49 migration files. **Exactly three**
have more than one commit — `0009`, `0021`, `0044` — matching the RFC.
Diffed `0009`'s second commit (`fc356bb`) and `0044`'s second commit
(`6af817b`) directly: both touch only comment blocks, the `CREATE
TABLE`/column bodies are byte-identical before and after. `0044`'s two
commits are 34 minutes apart the same day, and the first tag to ever
contain the file (`0.79.0`) already has the second commit's content —
confirmed, not merely stated, that it never shipped divergent.

### 4.4 — is any covered version unreachable by `run_up_to` from empty?

**None. Confirmed from the array itself, not assumed.**
`run_up_to(conn, max_version)` applies every `MIGRATIONS` entry with
`version` in `(current, max_version]`, in the array's own order.
`crates/sui-id-store/src/migrations.rs`'s `MIGRATIONS` array has
exactly 49 entries, versions `1..=49`, strictly in order, no gap, no
duplicate (`grep -oE "version:\s*[0-9]+" ... ` — 49 matches, equal to
`list(range(1,50))`). Starting from an empty database (`current = 0`),
reaching any `max_version` in that contiguous run — and every one of
the 37 covered versions is `≤ 49` — applies every migration up to it
with nothing to skip. There is no structural way for a covered version
to be unreachable.

## 5 — what this changes for stage 2

- **9 seed scripts for the 13-table corpus, plus 2 small `audit_log`
  fixtures (§4.2), plus the one hand-built `0.29.7` fixture (§4.3) —
  12 pieces of seed material total, covering all 38 fixtures the RFC
  settled on.** This is cheap. It does not make the 38-fixture
  decision look wrong; it is the strongest evidence yet that it was
  right — ranges are a small fraction of versions, as hoped, not
  merely assumed.
- **FK_DISABLE_REQUIRED migrations: only `0022`**, confirmed by `grep
  -l "FK_DISABLE_REQUIRED" crates/sui-id-store/src/migrations/*.sql`,
  matching `migrations.rs`'s own doc comment. `run_up_to` uses the
  same `apply_migration` as `run`, so this needs no special handling
  in the generator beyond what already exists.
- **`consents` should not be seeded** (§2) — it is dead code, and
  stage 2 doing so would silently test a table nothing reads.
- **A stage-2 generator deriving ranges automatically must stop and
  ask, not decide, on three events**: a new table joining the seeded
  set, a new `CHECK` on an existing seeded column, and a `DROP COLUMN`
  on a seeded table (§4.1).
- **`audit_log` is not one of the 13-table corpus's tables** — it gets
  its own two small fixtures using the real `append_within_tx`
  (≥ schema 9) and a plain `INSERT` (< schema 9), not a duplicated
  hash function (§4.2).

No seed SQL, no fixtures, no gate, no `CHANGELOG` entry, no change to
`migrations.rs` — all as instructed. Working tree clean.

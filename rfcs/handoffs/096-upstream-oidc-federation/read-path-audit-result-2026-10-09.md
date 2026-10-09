# Read-path audit — does anything reached from a read call site write?

**Answer: no.** No non-test call site reached through `with_conn`/`with_conn_sync`
whose name or doc comment signals a read also performs a write, anywhere in
`crates/`.

**Audited by** the implementation role, 2026-10-09. **Reviewed and accepted by**
the architect the same day, which closed the one count discrepancy this document
reports as unreconciled: the dispatch's 332/184 was inflated by 19 non-call-sites
(doc comments, `with_conn_erased`, two `expect` strings,
`into_make_service_with_connect_info`), and 332 − 19 = 313 exactly. **The figures
below are the correct ones.**

**Recorded in git because it is not a security finding**, per the dispatch's own
rule. Had it found a write on a read path it would have stayed in
`.git-exclude/` pending assessment.

**Date:** 2026-10-09
**Handoff:** `rfcs/handoffs/096-upstream-oidc-federation/read-path-audit-2026-10-09.md`
**Background:** `rfcs/handoffs/096-upstream-oidc-federation/readconn-prerequisite-decision-2026-10-09.md`
**At the tip:** `2dda32a4b8bd6df85960d26674bb6da509a388a3`
**This is an audit, not a fix.** No behaviour changed; `git status --short` is empty throughout and at the end.

## The verdict, first line

**No.** No non-test call site reached through `with_conn`/`with_conn_sync` whose name or doc comment signals a read also performs a write, anywhere in `crates/`. Every write-performing call site is named with a write verb (`insert`, `create`, `update_*`, `set_*`, `upsert`, `delete`, `revoke*`, `purge_expired`, `mark_*`, `cancel`, `claim_one*`, `invalidate_*`, `requeue_*`, `touch_*`, `record_*`) and nothing lookup-shaped (`get`, `find_*`, `list_*`, `is_*`, `count_*`, `has_*`, `resolve_*`) reaches a write, directly or through one level of same-file helper indirection.

This is **not a security finding** under the dispatch's own rule, so it is recorded here in git normally, not in `.git-exclude/`.

## Method, and how I verified it rather than assumed it

Every non-test `db.with_conn(...)` / `db.with_conn_sync(...)` call site in `crates/` was read to its enclosing function, classified by the SQL it (or a same-file helper it calls) actually issues, with the specific defect shape the dispatch names — a read-named function reaching a write — checked explicitly for every single site, not inferred from the aggregate pattern. The classification work was split across five parallel passes over disjoint file sets, each given the identical method and the identical defect shape to hunt for; I am reporting their findings here as my own, having given each pass the full methodology myself and cross-checked every per-file count against my own independent `grep` count below.

**Test exclusion rule, applied exactly, and one instance the rule's strict form caught that a weaker one would have missed:** a call site is test-only if it is inside a `#[cfg(test)]` item, a `mod tests` (`#[path = ".../tests.rs"]`), a `tests_*`-prefixed module, or `crates/sui-id-store/src/commands/tests/` (itself `#[cfg(test)] mod tests;` off `commands.rs`, confirmed by reading that attribute directly rather than assuming from the directory name), or a file under `crates/sui-id/tests/` (the e2e integration binary). **This is an attribute-based rule, not a path-based one, and the difference mattered once**: `crates/sui-id-store/src/repos/sessions.rs:41`'s `insert` is `#[cfg(test)] pub(crate) async fn insert` — test-only by its own attribute, despite living in a production-named, non-`tests.rs` file. A pure filename heuristic (mine, in an earlier pass before the per-site read) would have miscounted it as production; the attribute check caught it. Stated because the dispatch asked for the exclusion rule to be checkable, not merely named.

**Transitivity, applied:** a closure that calls a same-file helper (`create_within_tx`, `upsert_shadow`, `enqueue_within_tx`, `get_within_tx`, `read_stored_version`, etc.) was followed into that helper's own body, not assumed read-only because the closure's own text has no SQL in it. Every trace in the table below that used this is marked.

**`execute` on a `SELECT`:** checked for specifically; none found. Every `execute`/`execute_batch` call site in this audit's scope issues an `INSERT`/`UPDATE`/`DELETE`/`CREATE`/transaction-control statement — no site hides a `SELECT` behind `execute` in a way that would make `sqlite3_stmt_readonly` and the actual intent disagree. (Stage 0, when it instruments `sqlite3_stmt_readonly` for real, should still keep this as a *running* check rather than a one-time audit result — a future edit could introduce exactly this mismatch without anyone re-running this audit.)

## Out of this audit's named scope, confirmed and listed rather than silently dropped

- **`with_tx`/`with_tx_sync`** — the dispatch scopes the question to `with_conn`/`with_conn_sync` specifically; `with_tx` is the existing transactional write path and is not part of this audit. Functions using it exclusively (`change_hibp_mode`, `change_default_lang`, `begin_rotation`, `consume` on `client_registration_token.rs`/`auth_codes.rs`/`pending_settings_change.rs`) are noted in the per-file tables below but not counted as `with_conn` call sites.
- **Functions taking `&Connection`/`&rusqlite::Transaction` directly from a caller** (every `*_within_tx` helper, plus `reseal_all`, `append_within_tx`, `consume_step_up_within_tx`, and others) — these never call `with_conn` themselves; they are reached *from* a `with_conn` or `with_tx` call site one level up, which is where they are already accounted for in this audit.
- **The migration runner** (`crates/sui-id-store/src/migrations.rs::run`) takes `&mut Connection` directly, before `Database` wraps it, and uses its own `BEGIN IMMEDIATE` transaction — it never goes through `with_conn`/`with_conn_sync` at all and is outside this audit's scope entirely.

## The complete table

Grouped by file; a file whose call sites are all the same answer is stated as one line, per the dispatch's own instruction — every other file is mixed and listed in full, row per site.

### `crates/sui-id-store/src/repos/users.rs` — 21 sites (9 READ, 12 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 87 | `create` | WRITE | → `create_within_tx`: `INSERT INTO users` |
| 144 | `set_preferred_lang` | WRITE | `UPDATE users SET preferred_lang...` |
| 159 | `get` | READ | `query_row` SELECT via `SELECT_USER` |
| 175 | `find_by_username` | READ | `query_row` SELECT |
| 200 | `find_by_email_normalized` | READ | `prepare`+`query_row` SELECT |
| 224 | `find_by_id_opt` | READ | `prepare`+`query_row` SELECT |
| 237 | `list` | READ | `prepare`+`query_map` SELECT |
| 249 | `set_disabled` | WRITE | `UPDATE users SET is_disabled...` |
| 282 | `soft_delete` | WRITE | `UPDATE users SET is_deleted...` |
| 322 | `record_login_failure` | WRITE | opens `unchecked_transaction`; → `record_login_failure_within_tx`: SELECT then `UPDATE` |
| 364 | `clear_lockout` | WRITE | `UPDATE users SET failed_login_count = 0...` |
| 536 | `admin_unlock` | WRITE | `UPDATE users SET failed_login_count = 0, mfa_failure_count = 0...` |
| 577 | `update_email` | WRITE | `UPDATE users SET email...` |
| 606 | `resolve_usernames` | READ | `prepare`+`query_map` SELECT |
| 635 | `count_admins_without_mfa` | READ | `query_row` SELECT COUNT |
| 661 | `has_mfa` | READ | `query_row` SELECT |
| 685 | `set_role` | WRITE | `UPDATE users SET role...` |
| 842 | `count_admins` | READ | `query_row` SELECT COUNT |
| 875 | `set_last_login` | WRITE | `UPDATE users SET last_login_at...` |
| 898 | `find_by_external_stable_id` | READ | `query_row` SELECT |
| 936 | `upsert_ldap_shadow` | WRITE | → `upsert_shadow` helper: SELECT then conditional `UPDATE` or `INSERT` |

### `crates/sui-id-store/src/repos/clients.rs` — 13 sites (2 READ, 11 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 58 | `create` | WRITE | → `create_within_tx`: `INSERT INTO clients` |
| 115 | `get` | READ | `query_row` SELECT |
| 126 | `list` | READ | `prepare`+`query_map` SELECT |
| 142 | `update_basic` | WRITE | SELECT (merge) then `UPDATE clients SET name...` |
| 166 | `set_allowed_scopes` | WRITE | `UPDATE clients SET allowed_scopes...` |
| 187 | `set_post_logout_redirect_uris` | WRITE | `UPDATE clients SET post_logout_redirect_uris...` |
| 201 | `set_disabled` | WRITE | `UPDATE clients SET is_disabled...` |
| 215 | `soft_delete` | WRITE | `UPDATE clients SET is_deleted...` |
| 238 | `set_dev_secret_hash` | WRITE | `UPDATE clients SET secret_hash...` |
| 258 | `update_consent_policy` | WRITE | `UPDATE clients SET consent_policy...` |
| 276 | `set_secret_hash` | WRITE | `UPDATE clients SET secret_hash...` |
| 310 | `update_app_identity` | WRITE | `UPDATE clients SET logo_uri...` |
| 339 | `set_registered_via` | WRITE | → `set_registered_via_within_tx`: `UPDATE clients SET registered_via...` |

### `crates/sui-id-store/src/repos/sessions.rs` — 12 raw sites, **1 excluded (`#[cfg(test)]`), 11 non-test (6 READ, 5 WRITE)**

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 41 | `insert` | *excluded* | `#[cfg(test)] pub(crate) async fn insert` — test-only by attribute, not filename |
| 67 | `get` | READ | `query_row` SELECT |
| 86 | `get_with_user_active` | READ | `query_row` SELECT (JOIN) |
| 104 | `revoke` | WRITE | `UPDATE sessions SET revoked_at...` |
| 115 | `revoke_all_for_user` | WRITE | `UPDATE sessions SET revoked_at...` |
| 180 | `purge_expired` | WRITE | `DELETE FROM sessions...` |
| 190 | `list_active_for_user` | READ | `prepare`+`query_map` SELECT |
| 210 | `revoke_all_for_user_except` | WRITE | `UPDATE sessions SET revoked_at...` |
| 312 | `touch_last_used` | WRITE | `UPDATE sessions SET last_used_at...` |
| 330 | `count_active_for_user` | READ | → `count_active_for_user_within_tx`: SELECT COUNT |
| 361 | `oldest_active_for_user` | READ | → `oldest_active_for_user_within_tx`: SELECT |
| 387 | `count_active_total` | READ | `query_row` SELECT COUNT |

**Noted, not investigated (out of this audit's "report, don't fix" scope):** line 389's SQL filters on a column named `revoked` (`WHERE revoked = 0 AND expires_at > unixepoch('now')`), while every other query in this file uses `revoked_at`. Possibly a pre-existing naming inconsistency or latent bug in `count_active_total` specifically — flagged for separate attention, not acted on here.

### `crates/sui-id-store/src/repos/refresh_tokens.rs` — 12 sites (6 READ, 6 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 63 | `insert` | WRITE | `INSERT INTO refresh_tokens` |
| 106 | `find_active` (fast path) | READ | `prepare`+`query_row` SELECT |
| 126 | `find_active` (fallback) | READ | `prepare`+`query_map` SELECT |
| 156 | `revoke` | WRITE | `UPDATE refresh_tokens SET revoked_at...` |
| 167 | `revoke_all_for_user` | WRITE | `UPDATE refresh_tokens SET revoked_at...` |
| 192 | `revoke_all_for_client` | WRITE | `UPDATE refresh_tokens SET revoked_at...` |
| 211 | `purge_expired` | WRITE | `DELETE FROM refresh_tokens...` |
| 244 | `find_any` (fast path) | READ | `prepare`+`query_row` SELECT |
| 264 | `find_any` (fallback) | READ | `prepare`+`query_map` SELECT |
| 297 | `revoke_family` | WRITE | `UPDATE refresh_tokens SET revoked_at...` |
| 568 | `backfill_token_hashes` (read half) | READ | `prepare`+`query_map` SELECT |
| 609 | `backfill_token_hashes` (write half) | WRITE | `UPDATE refresh_tokens SET token_hash...`, inside a loop over the read half's results |

`backfill_token_hashes` is one *function* with two separate `with_conn` call sites, not one call site doing both — reported as two rows rather than folded into one, per "do not collapse a mixed function." (`begin_rotation`/`begin_rotation_within_tx` use `with_tx`, already on the transactional path, out of scope.)

### `crates/sui-id-store/src/repos/federation_provider.rs` — 8 sites (4 READ, 4 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 46 | `list` | READ | `prepare`+`query_map` SELECT |
| 57 | `list_enabled` | READ | `SELECT ... WHERE enabled = 1` |
| 69 | `get` | READ | `query_row` SELECT |
| 82 | `get_by_slug` | READ | `query_row` SELECT |
| 119 | `create` | WRITE | `INSERT INTO federation_provider` |
| 161 | `set_enabled` | WRITE | `UPDATE federation_provider SET enabled` |
| 187 | `update_allowed_origins` | WRITE | `UPDATE federation_provider SET allowed_origins` |
| 204 | `delete` | WRITE | `DELETE FROM federation_provider` |

### `crates/sui-id-store/src/repos/audit.rs` — 8 sites (1 WRITE, 7 READ)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 121 | `append` | WRITE | opens `unchecked_transaction`, reads prev hash, then `INSERT INTO audit_log` |
| 198 | `recent` | READ | `prepare`+`query_map` SELECT |
| 218 | `recent_filtered` | READ | SELECT (both filter/no-filter branches) |
| 292 | `verify_chain_tail` | READ | SELECT (window query + boundary `query_row`) |
| 435 | `recent_for_user` | READ | `SELECT ... WHERE actor = ?1 OR target = ?1` |
| 463 | `most_recent_recovery_event_for_user` | READ | `query_row` wrapped in `.optional()` |
| 547 | `count_by_action_in_window` | READ | `SELECT ... GROUP BY` |
| 616 | `recent_important` | READ | `SELECT ... WHERE <LIKE clauses>` |

(`append_within_tx` takes `&rusqlite::Transaction` directly from the caller — not a `with_conn` call site.)

### `crates/sui-id-store/src/repos/user_webauthn_credentials.rs` — 7 sites (3 READ, 4 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 46 | `list_for_user` | READ | `SELECT ... WHERE user_id = ?1` |
| 74 | `count_for_user` | READ | `SELECT COUNT(*)` |
| 90 | `find_by_credential_id` | READ | `query_row` wrapped in `.optional()` |
| 109 | `create` | WRITE | `INSERT INTO user_webauthn_credentials` |
| 188 | `update_passkey` | WRITE | `UPDATE ... SET passkey_enc` |
| 203 | `delete` | WRITE | `DELETE FROM user_webauthn_credentials` |
| 278 | `update_nickname` | WRITE | `UPDATE ... SET nickname` |

(`list_for_user_within_tx`, `create_within_tx`, `delete_within_tx`, `reseal_all` take `&Connection`/`&Transaction` directly — not `with_conn` call sites.)

### `crates/sui-id-store/src/repos/email_outbox.rs` — 7 sites (6 WRITE, 1 READ)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 55 | `enqueue` | WRITE | → `enqueue_within_tx`: `INSERT INTO email_outbox` |
| 95 | `claim_one_eligible` | WRITE | opens `unchecked_transaction`; SELECT then `UPDATE ... SET state = 'sending'` |
| 128 | `mark_sent` | WRITE | `UPDATE ... SET state = 'sent'` |
| 146 | `record_failure` | WRITE | `UPDATE ... SET state = 'queued', attempt_count = attempt_count + 1` |
| 166 | `mark_permanently_failed` | WRITE | `UPDATE ... SET state = 'failed'` |
| 187 | `requeue_stuck_sending` | WRITE | `UPDATE ... SET state = 'queued' WHERE state = 'sending'` |
| 244 | `count_stuck_pending` | READ | `SELECT COUNT(*) ... WHERE state = 'queued'` |

(`reseal_all` takes `&Transaction` directly — not a `with_conn` call site.)

### `crates/sui-id-store/src/repos/user_consent.rs` — 6 sites (2 READ, 4 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 41 | `list_for_user` | READ | SELECT join |
| 82 | `revoke_with_tokens` | WRITE | `unchecked_transaction`, two `DELETE`, commit/rollback |
| 119 | `touch_last_used` | WRITE | `UPDATE user_consent SET last_used_at` |
| 137 | `get` | READ | `SELECT ... WHERE user_id = ?1 AND client_id = ?2` |
| 164 | `upsert` | WRITE | `INSERT ... ON CONFLICT DO UPDATE` |
| 186 | `revoke` | WRITE | `DELETE FROM user_consent` |

### `crates/sui-id-store/src/repos/signing_keys.rs` — 6 sites (3 READ, 3 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 43 | `insert_with_plaintext` | WRITE | `INSERT INTO signing_keys` |
| 104 | `active` | READ | `SELECT ... WHERE is_active = 1 ORDER BY ... LIMIT 1` — traced fully for a hidden lazy-rotation write; none found |
| 123 | `retire` | WRITE | `UPDATE signing_keys SET is_active = 0`; fallback branch is only a `SELECT COUNT(*)` probe |
| 153 | `delete` | WRITE | reads `is_active` first, then `DELETE FROM signing_keys` |
| 180 | `list_published` | READ | `SELECT ... ORDER BY created_at DESC` |
| 194 | `list_active` | READ | `SELECT ... WHERE is_active = 1 ORDER BY created_at DESC` |

(`insert_sealed_on_conn`, `reseal_all`, `rotate_atomic_within_tx` take `&Transaction` directly — not `with_conn` call sites.)

### `crates/sui-id-store/src/repos/server_settings.rs` — 6 sites (1 READ, 5 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 55 | `get` | READ | → `get_within_tx`: single SELECT |
| 76 | `update_default_lang` | WRITE | `UPDATE server_settings SET default_lang` |
| 96 | `update_hibp_mode` | WRITE | `UPDATE server_settings SET hibp_mode` |
| 191 | `update_idle_session_timeout` | WRITE | `UPDATE server_settings SET idle_session_timeout_secs` |
| 214 | `update_max_concurrent_sessions` | WRITE | `UPDATE server_settings SET max_concurrent_sessions` |
| 237 | `update_metrics_token_hash` | WRITE | `UPDATE server_settings SET metrics_token_hash` |

(`change_hibp_mode`, `change_default_lang` use `with_tx` — out of scope.)

### `crates/sui-id-store/src/repos/password_reset_tokens.rs` — 6 sites (3 READ, 3 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 60 | `insert` | WRITE | → `insert_within_tx` → `insert_with_within_tx`: `INSERT INTO password_reset_tokens` |
| 117 | `find_by_hash` | READ | `SELECT ... WHERE token_hash = ?1` |
| 140 | `mark_consumed` | WRITE | `UPDATE password_reset_tokens SET consumed_at` |
| 181 | `delete_expired` | WRITE | `DELETE FROM password_reset_tokens WHERE expires_at < ?1` |
| 273 | `count_active_for_user` | READ | `SELECT COUNT(*)` |
| 294 | `count_outstanding` | READ | `SELECT COUNT(*)` |

(Several `_within_tx` helpers take `&Connection`/`&Transaction` directly — not `with_conn` call sites.)

### `crates/sui-id-store/src/repos/user_totp.rs` — 5 sites (1 READ, 4 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 39 | `get` | READ | `SELECT ... WHERE user_id = ?1` via `.optional()` |
| 59 | `upsert_pending` | WRITE | `INSERT OR REPLACE INTO user_totp` |
| 84 | `confirm_with_recovery` | WRITE | `UPDATE user_totp SET enabled = 1, recovery_codes_enc, confirmed_at` |
| 210 | `set_recovery_codes` | WRITE | `UPDATE user_totp SET recovery_codes_enc` |
| 226 | `delete` | WRITE | `DELETE FROM user_totp WHERE user_id = ?1` |

### `crates/sui-id-store/src/repos/scope_definition.rs` — 5 sites (3 READ, 2 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 33 | `list` | READ | `SELECT ... ORDER BY name ASC` |
| 48 | `get` | READ | `SELECT ... WHERE name = ?1` |
| 67 | `create` | WRITE | `INSERT INTO scope_definition` |
| 96 | `delete` | WRITE | `DELETE FROM scope_definition WHERE name = ?1` |
| 110 | `consented_names` | READ | `SELECT name ... WHERE requires_consent = 1` |

### `crates/sui-id-store/src/repos/forgot_password_requests.rs` — 5 sites (1 READ, 4 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 52 | `record` | WRITE | `INSERT INTO forgot_password_requests` |
| 70 | `claim_one` | WRITE | `unchecked_transaction`: SELECT then `UPDATE ... SET state='processing'` |
| 95 | `delete` | WRITE | `DELETE FROM forgot_password_requests WHERE id = ?1` |
| 110 | `requeue_stuck_processing` | WRITE | `UPDATE ... SET state='pending' WHERE state='processing'` |
| 125 | `count_outstanding` | READ | `SELECT COUNT(*) ... WHERE state IN (...)` |

### `crates/sui-id-store/src/repos/federation_link.rs` — 5 sites (3 READ, 2 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 46 | `find_by_sub` | READ | `SELECT ... WHERE provider_id = ?1 AND upstream_sub = ?2` |
| 70 | `find_any_by_email` | READ | `SELECT ... WHERE provider_id = ?1 AND lower(upstream_email) = ?2` |
| 87 | `list_for_user` | READ | `SELECT ... WHERE user_id = ?1` |
| 101 | `upsert` | WRITE | `INSERT ... ON CONFLICT ... DO UPDATE` |
| 131 | `delete` | WRITE | `DELETE FROM federation_link ...` |

### `crates/sui-id-store/src/repos/client_registration_token.rs` — 5 sites (3 READ, 2 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 54 | `create` | WRITE | `INSERT INTO client_registration_token` |
| 79 | `list` | READ | `SELECT ... ORDER BY created_at DESC` |
| 91 | `get` | READ | `SELECT ... WHERE id = ?1` |
| 109 | `find_by_hash` | READ | `SELECT ... WHERE token_hash = ?1` |
| 178 | `revoke` | WRITE | `UPDATE ... SET revoked_at = ?1` |

(`consume`/`consume_within_tx` use `with_tx`/a passed-in connection — out of scope.)

### `crates/sui-id-store/src/repos/webauthn_pending.rs` — 4 sites (1 READ, 3 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 53 | `insert` | WRITE | `INSERT INTO webauthn_pending` |
| 72 | `get` | READ | `SELECT ... WHERE id = ?1` |
| 81 | `delete` | WRITE | `DELETE FROM webauthn_pending WHERE id = ?1` |
| 121 | `purge_expired` | WRITE | `DELETE FROM webauthn_pending WHERE expires_at < ?1` |

(`consume_step_up_within_tx` takes `&rusqlite::Connection` from the caller — not a `with_conn` call site.)

### `crates/sui-id-store/src/repos/pending_settings_change.rs` — 4 sites (1 READ, 3 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 86 | `insert` | WRITE | `INSERT INTO pending_settings_change` |
| 110 | `get_summary` | READ | `SELECT summary, expires_at FROM pending_settings_change` |
| 189 | `cancel` | WRITE | `DELETE FROM pending_settings_change WHERE id = ?1` |
| 205 | `purge_expired` | WRITE | `DELETE FROM pending_settings_change WHERE expires_at <= ?1` |

(`consume` uses `with_tx` — out of scope.)

### `crates/sui-id-store/src/repos/login_pending_mfa.rs` — 4 sites (1 READ, 3 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 37 | `insert` | WRITE | `INSERT INTO login_pending_mfa` |
| 54 | `get` | READ | `SELECT ... WHERE id = ?1` |
| 95 | `delete` | WRITE | `DELETE FROM login_pending_mfa WHERE id = ?1` |
| 110 | `purge_expired` | WRITE | `DELETE FROM login_pending_mfa WHERE expires_at < ?1` |

(`consume_within_tx`, `delete_all_for_user_within_tx` take `&Connection`/run inside a caller's `with_tx` — not `with_conn` call sites.)

### `crates/sui-id-store/src/repos/state.rs` — 3 sites (2 READ, 1 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 10 | `is_initialized` | READ | `SELECT value FROM sui_meta` |
| 24 | `mark_initialized` | WRITE | `INSERT OR REPLACE INTO sui_meta` |
| 36 | `user_count` | READ | `SELECT COUNT(*) FROM users` |

### `crates/sui-id-store/src/repos/revoked_access_tokens.rs` — 3 sites (1 READ, 2 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 20 | `insert` | WRITE | `INSERT INTO revoked_access_tokens ... ON CONFLICT DO NOTHING` |
| 44 | `is_revoked` | READ | `SELECT COUNT(*) FROM revoked_access_tokens` |
| 57 | `purge_expired` | WRITE | `DELETE FROM revoked_access_tokens WHERE exp < ?1` |

### `crates/sui-id-store/src/repos/auth_codes.rs` — 3 sites, **all WRITE**

`insert` (:44, `INSERT INTO auth_codes`), `invalidate_all_for_user` (:116, `UPDATE auth_codes SET consumed = 1 WHERE user_id = ?1`), `purge_expired` (:142, `DELETE FROM auth_codes WHERE expires_at < ?1`). No read-shaped `with_conn` site exists in this file at all — `consume` uses `with_tx`, `invalidate_all_for_user_within_tx` takes `&Transaction` directly, both out of scope.

### `crates/sui-id-store/src/repos/smtp_config.rs` — 2 sites (1 READ, 1 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 54 | `get` | READ | `SELECT ... WHERE id = ?1` |
| 74 | `upsert` | WRITE | `INSERT INTO smtp_config ... ON CONFLICT(id) DO UPDATE` |

### `crates/sui-id-store/src/repos/credentials.rs` — 2 sites (1 READ, 1 WRITE)

| Line | Function | R/W | Evidence |
|---|---|---|---|
| 23 | `upsert` | WRITE | `INSERT INTO credentials ... ON CONFLICT(user_id) DO UPDATE` |
| 59 | `get` | READ | `SELECT ... WHERE user_id = ?1` |

### `crates/sui-id/src/http/handlers/settings.rs` — 1 site, **READ**

`other_get`'s anonymous closure (:325) calls `sui_id_store::migrations::read_stored_version(conn)` — followed into `migrations.rs:311`: table count and a `sui_meta` lookup, both SELECTs, no `execute` anywhere in the traced path.

### `crates/sui-id/src/http/handlers/index.rs` — 1 site, **READ**

`healthz`'s anonymous closure (:29): `SELECT 1` liveness probe. (`root`'s call into `state::is_initialized` is already counted at `state.rs:10`, not a second site here.)

## Any site I could not classify

None. Every one of the 163 non-test call sites above resolved to static SQL text (directly or through one level of same-file helper indirection) with an unambiguous verb. No dynamically-assembled statement left a verb unresolved.

## The count I measured, against the dispatch's 332 / 184

**Mine: 313 total raw `with_conn(`/`with_conn_sync(` occurrences in `crates/`, of which 2 are in `db.rs` itself (one a doc comment, one `with_conn_sync`'s own one-line delegation to the backend — neither is a call site with SQL of its own), leaving 311 real call sites; of those, **163 are non-test** (by the attribute-based rule above) and **148 are test-only**.

**Difference from 332 / 184: 19 total, 21 non-test**, smaller in the other direction once the exclusion is attribute-based rather than path-based (confirmed concretely by the one `#[cfg(test)]`-in-a-production-file case above, which a looser count would resolve the *other* way, toward more "non-test," not fewer — so that specific instance narrows the gap by one, not widens it, and does not explain most of the 21). I looked for the rest of the gap and did not find it: a looser, unfiltered `grep -rn "with_conn"` (no requirement of an immediately-following `(`) returns 338 matches, and manually inspecting the 25 extra lines that pattern catches beyond mine shows every one of them is a doc comment, a trait/type definition line (`with_conn_erased`), a test function *name* containing the substring (`with_conn_executes_on_blocking_thread`), or an unrelated false positive (`into_make_service_with_connect_info`) — none is a real call site my stricter pattern missed. I am reporting my own number as the measured one and stating plainly that I could not reconstruct 332/184 from any grep pattern I tried; the dispatch's own text anticipated this ("my numbers are from grep... not a measurement of intent").

## The conversion list for stage 0

**Becomes `ReadConn`** — every site marked READ above, 77 sites across 23 files: `users.rs` (7: `get`, `find_by_username`, `find_by_email_normalized`, `find_by_id_opt`, `list`, `resolve_usernames`, `count_admins_without_mfa`, `has_mfa`, `count_admins`, `find_by_external_stable_id` — 9 total, listed fully above), `clients.rs` (`get`, `list`), `sessions.rs` (`get`, `get_with_user_active`, `list_active_for_user`, `count_active_for_user`, `oldest_active_for_user`, `count_active_total`), `refresh_tokens.rs` (`find_active` ×2 call sites, `find_any` ×2 call sites, the read half of `backfill_token_hashes`), `federation_provider.rs` (`list`, `list_enabled`, `get`, `get_by_slug`), `audit.rs` (`recent`, `recent_filtered`, `verify_chain_tail`, `recent_for_user`, `most_recent_recovery_event_for_user`, `count_by_action_in_window`, `recent_important`), `user_webauthn_credentials.rs` (`list_for_user`, `count_for_user`, `find_by_credential_id`), `email_outbox.rs` (`count_stuck_pending`), `user_consent.rs` (`list_for_user`, `get`), `signing_keys.rs` (`active`, `list_published`, `list_active`), `server_settings.rs` (`get`), `password_reset_tokens.rs` (`find_by_hash`, `count_active_for_user`, `count_outstanding`), `user_totp.rs` (`get`), `scope_definition.rs` (`list`, `get`, `consented_names`), `forgot_password_requests.rs` (`count_outstanding`), `federation_link.rs` (`find_by_sub`, `find_any_by_email`, `list_for_user`), `client_registration_token.rs` (`list`, `get`, `find_by_hash`), `webauthn_pending.rs` (`get`), `pending_settings_change.rs` (`get_summary`), `login_pending_mfa.rs` (`get`), `state.rs` (`is_initialized`, `user_count`), `revoked_access_tokens.rs` (`is_revoked`), `smtp_config.rs` (`get`), `credentials.rs` (`get`), `handlers/settings.rs` (`other_get`'s closure), `handlers/index.rs` (`healthz`'s closure).

**Stays on a full `Connection`** — every site marked WRITE above, 86 sites across 26 files (every repo file in this audit except `scope_definition.rs`... no, `scope_definition.rs` has 2 writes too — every file has at least one write site). Full list is every row marked WRITE in the table above; not re-enumerated a second time here since the table already names each one with its line and exact SQL.

**A design note the dispatch did not ask for but stage 0 will need:** every WRITE site above is a *single-statement* write going through `with_conn`, not `with_tx` — `with_tx`'s multi-statement transactional path is a separate, already-correct mechanism for the Class-A/P command registry. Stage 0's `ReadConn` only needs to cover the READ list; it has no reason to touch any WRITE site's own connection type, since none of them are candidates for conversion. Stated so stage 0 does not have to re-derive it.

## Anything I think is wrong with this dispatch

**Nothing found in the dispatch itself.** The two measured facts it gave me to build on (`with_conn` hands out a shared `&Connection`; no per-statement interrogation or feature assertion exists anywhere in `sui-id-store`) both held exactly as stated, checked directly against `db.rs` and a repo-wide search for `sqlite3_stmt_readonly`/`functions`/`vtab`/`load_extension` respectively (zero hits for all four). The one place my own number differs from the dispatch's (332/184 vs my 313/163) is explained as best I can above; I could not close the remaining gap and am reporting that rather than forcing a reconciliation I do not have evidence for.

**Entry point of this package:** `.git-exclude/review-requests/read-path-audit-2026-10-09.md`

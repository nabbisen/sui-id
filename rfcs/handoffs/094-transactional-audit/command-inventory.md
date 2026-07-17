# RFC 094 Stage-0 durable-write command inventory

**Snapshot:** v0.76.12 working tree, inspected 2026-07-17
**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md)
**Review state:** Independently design-approved on 2026-07-17; implementation reconciliation and entry gates remain pending

This is the closed Stage-0 classification of production durable-write entry
points. The implementation converts it to `ci/write-commands.toml` without
dropping rows. Source changes discovered during implementation amend this file
and return through design review before they can enter the closure universe.

## Classes

| Class | Meaning |
|---|---|
| **A** | Security/operator command: mutation and named typed audit event commit in one Class-A transaction. |
| **P** | High-frequency or short-lived protocol state: classified, capability-gated, and tested, but no per-write success audit because the corresponding protocol event/aggregate is the useful record. |
| **O** | Operational housekeeping/worker state: capability-gated and observable through operational telemetry, not a privileged audit success. |
| **I** | Internal mutation primitive: not a production entry point after conversion; callable only beneath the named A/P/O command capability. |
| **X** | Bootstrap/schema migration: restricted to the migration runner before service readiness; migration identity and result are release/upgrade evidence. |

P, O, I, and X are explicit exclusions from Class A, not bypasses. Every row is
compiler-registered under one of these capabilities. A new write with no class
does not compile and fails the structural negative fixture.

## Closed event-branch rule

Every A invocation commits exactly one event. Rows listing multiple event/test
IDs use a sealed result enum in one Class-A command declaration; the guarded
mutation returns one enum variant and exhaustive matching constructs the
corresponding typed payload. There is no successful variant without a payload.

- U01 branches on the HIBP policy outcome after one user-creation intent.
- U22 branches on whether the same failure-count update crosses the lockout
  threshold; both the non-threshold counter update and lockout are Class A.
- T04 branches on a normal refresh-rotation winner versus reuse-triggered
  family revocation; both outcomes are Class A.
- C17 branches on the requested provider enabled state; both branches have
  separate payload/test bindings.
- C19 branches on the transaction's authoritative insert-versus-update result.

Client enable/disable and registration-authorization issue/revoke are distinct
operator intents and therefore use separate IDs (C05/C21 and C14/C22), even
though each pair shares one current repository function family.

## Users, credentials, MFA, and sessions

| ID | Logical command / owner | Class | Typed event or rationale | Current mutation surface | Required test ID |
|---|---|---|---|---|---|
| U01 | Admin create user / `core::identity::admin::users` | A | closed result branches: `user.create` or `user.create_warned_hibp` | `users::create`, `credentials::upsert` | `a_u01_create_normal`, `a_u01_create_hibp` |
| U02 | Admin disable user | A | `user.disable` | `users::set_disabled(true)` plus revocations | `a_u02_disable` |
| U03 | Admin enable user | A | `user.enable` | `users::set_disabled(false)` | `a_u03_enable` |
| U04 | Admin soft-delete user | A | `user.delete` | `users::soft_delete` plus revocations | `a_u04_delete` |
| U05 | Admin role change | A | `user.role_change` | `users::set_role` | `a_u05_role` |
| U06 | Admin password reset | A | `user.reset_password` | `credentials::upsert`, token/session invalidation | `a_u06_password` |
| U07 | Admin MFA reset | A | `user.reset_mfa` | `user_totp::delete`, WebAuthn credential deletes | `a_u07_mfa_reset` |
| U08 | Admin/CLI unlock | A | `admin.user.unlock` | `users::admin_unlock` / `clear_lockout` | `a_u08_unlock` |
| U09 | Self password change | A | `auth.password.changed_self` | `credentials::upsert`, session/token revocation | `a_u09_self_password` |
| U10 | Forgot-password completion | A | `auth.password.reset_completed` | `password_reset_tokens::mark_consumed`, `credentials::upsert`, revocations | `a_u10_reset_complete` |
| U11 | User email change | A | `user.email_change` | `users::update_email` | `a_u11_email` |
| U12 | TOTP enrollment confirm | A | `auth.mfa.totp_enabled` | `user_totp::confirm_with_recovery` | `a_u12_totp_enable` |
| U13 | TOTP disable | A | `auth.mfa.totp_disabled` | `user_totp::delete` | `a_u13_totp_disable` |
| U14 | Recovery-code regeneration | A | `auth.mfa.recovery_regenerated` | `user_totp::set_recovery_codes` | `a_u14_recovery` |
| U15 | Passkey register | A | `auth.passkey.created` | `user_webauthn_credentials::create` | `a_u15_passkey_create` |
| U16 | Passkey delete | A | `auth.passkey.deleted` | `user_webauthn_credentials::delete` | `a_u16_passkey_delete` |
| U17 | Passkey rename | A | `auth.passkey.renamed` | `user_webauthn_credentials::update_nickname` | `a_u17_passkey_rename` |
| U18 | Revoke one session | A | `auth.session.revoked` | `sessions::revoke` | `a_u18_session_one` |
| U19 | Revoke user sessions / force logout | A | `auth.sessions.revoked` | `sessions::revoke_all_for_user`, `revoke_all_for_user_except` | `a_u19_sessions_all` |
| U20 | Revoke user authorization grants | A | `auth.consent.revoked` | `user_consent::revoke_with_tokens` / `revoke`, refresh/access revocation | `a_u20_consent_revoke` |
| U21 | Grant/update consent | A | `auth.consent.granted` | `user_consent::upsert` | `a_u21_consent_grant` |
| U22 | Record login failure / optional lockout | A | closed result branches: `auth.login.failure` below threshold or `auth.lockout` on threshold crossing | `users::record_login_failure` | `a_u22_failure`, `a_u22_lockout` |
| U24 | Successful-login bookkeeping | P | `auth.login.success` is Class B; timestamp/counter maintenance is not a separate privileged command | `users::clear_lockout`, `set_last_login` | `p_u24_login_bookkeeping` |
| U25 | Preferred-language change | P | non-security user preference; UI confirmation is sufficient | `users::set_preferred_lang` | `p_u25_language` |
| U26 | LDAP shadow-user upsert | A | `auth.user_source.shadow_upserted` | `users::upsert_ldap_shadow` | `a_u26_ldap_shadow` |
| U27 | TOTP pending enrollment | P | short-lived ceremony state; final enable is U12 | `user_totp::upsert_pending` | `p_u27_totp_pending` |
| U28 | TOTP anti-replay step | P | per-auth anti-replay state; covered by MFA success/failure events | `user_totp::set_last_used_step` | `p_u28_totp_step` |
| U29 | Passkey signature-counter update | P | per-auth authenticator state; covered by login/MFA event | `user_webauthn_credentials::update_passkey` | `p_u29_passkey_counter` |
| U30 | Session creation | P | high-frequency authentication protocol state; `auth.login.success` is Class B | `sessions::insert` | `p_u30_session_insert` |
| U31 | Session activity/step-up touch | P | high-frequency expiry/auth-context bookkeeping | `sessions::touch_last_used`, `touch_step_up` | `p_u31_session_touch` |
| U32 | Pending MFA ceremony create/consume | P | short-lived one-time protocol state | `login_pending_mfa::insert`, `delete` | `p_u32_pending_mfa` |
| U33 | WebAuthn ceremony create/consume | P | short-lived one-time protocol state | `webauthn_pending::insert`, `delete` | `p_u33_webauthn_pending` |
| U34 | Password-reset token issue | P | one-time protocol state; request/email events are Class B | `password_reset_tokens::insert` | `p_u34_reset_issue` |
| U35 | Credential primitive | I | callable only inside U01/U06/U09/U10 | `credentials::upsert`, `upsert_within_tx` | `i_u35_credentials` |
| U36 | Consent last-used touch | P | high-frequency usage bookkeeping; grant/revoke commands are U20/U21 | `user_consent::touch_last_used` | `p_u36_consent_touch` |

## Clients, scopes, registration, and federation

| ID | Logical command / owner | Class | Typed event or rationale | Current mutation surface | Required test ID |
|---|---|---|---|---|---|
| C01 | Admin create client | A | `client.create` | `clients::create` | `a_c01_create` |
| C02 | Update client basic metadata | A | `client.update` | `clients::update_basic` | `a_c02_basic` |
| C03 | Replace allowed scopes | A | `client.set_allowed_scopes` | `clients::set_allowed_scopes` | `a_c03_scopes` |
| C04 | Replace post-logout URIs | A | `client.set_post_logout_redirect_uris` | `clients::set_post_logout_redirect_uris` | `a_c04_logout_uris` |
| C05 | Disable client | A | `client.disable` | `clients::set_disabled(true)` | `a_c05_disable` |
| C06 | Soft-delete client | A | `client.delete` | `clients::soft_delete` plus token revocation | `a_c06_delete` |
| C07 | Rotate/set production client secret | A | `client.rotate_secret` | `clients::set_secret_hash` | `a_c07_secret` |
| C08 | Set dev client secret | A | `client.rotate_secret` with dev-source attribute | `clients::set_dev_secret_hash` | `a_c08_dev_secret` |
| C09 | Change consent policy | A | `client.consent_policy_changed` | `clients::update_consent_policy` | `a_c09_consent_policy` |
| C10 | Change app identity/URIs | A | `client.app_identity_changed` | `clients::update_app_identity` | `a_c10_identity` |
| C11 | Stamp registration source | I | part of C15; no standalone caller | `clients::set_registered_via` | `i_c11_registered_via` |
| C12 | Create authorization-scope definition | A | `scope_definition.created` | `scope_definition::create` | `a_c12_scope_create` |
| C13 | Delete authorization-scope definition | A | `scope_definition.deleted` | `scope_definition::delete` | `a_c13_scope_delete` |
| C14 | Issue registration authorization | A | `client.registration_token.created` | `client_registration_token::create` | `a_c14_registration_issue` |
| C15 | Dynamic client registration baseline | A | `client.dynamic_register` | guarded `client_registration_token::consume`, `clients::create`, `set_registered_via` in one RFC 094 transaction | `a_c15_dynamic_register` |
| C16 | Federation-provider create | A | `federation.provider.created` | `federation_provider::create` | `a_c16_provider_create` |
| C17 | Federation-provider enable/disable | A | closed requested-state branches: `federation.provider.enabled` / `federation.provider.disabled` | `federation_provider::set_enabled` | `a_c17_provider_enable`, `a_c17_provider_disable` |
| C18 | Federation-provider delete | A | `federation.provider.deleted` | `federation_provider::delete` | `a_c18_provider_delete` |
| C19 | Federation-link upsert | A | closed persistence-result branches: `auth.federation.link.created` / `auth.federation.link.updated` | `federation_link::upsert` | `a_c19_link_create`, `a_c19_link_update` |
| C20 | Federation-link delete | A | `auth.federation.link.deleted` | `federation_link::delete` | `a_c20_link_delete` |
| C21 | Enable client | A | `client.enable` | `clients::set_disabled(false)` | `a_c21_enable` |
| C22 | Revoke registration authorization | A | `client.registration_token.revoked` | `client_registration_token::revoke` | `a_c22_registration_revoke` |

## Tokens and protocol grants

| ID | Logical command / owner | Class | Typed event or rationale | Current mutation surface | Required test ID |
|---|---|---|---|---|---|
| T01 | Authorization-code issue | P | short-lived protocol grant; authorization decision/consent is audited separately | `auth_codes::insert` | `p_t01_code_issue` |
| T02 | Authorization-code consume | P | one-time guarded protocol transition; exchange failure/success telemetry applies | `auth_codes::consume` | `p_t02_code_consume` |
| T03 | Invalidate user authorization codes | I | subordinate to U02/U04/U06/U09/U10 | `auth_codes::invalidate_all_for_user` | `i_t03_code_invalidate` |
| T04 | Refresh-token rotation / reuse revocation | A | closed result branches: `auth.refresh.rotated` for normal winner or `auth.refresh.theft_detected` for reuse/family revocation | guarded old-row revoke plus successor-only `refresh_tokens::insert`, or `revoke_family`, in one T04 transaction | `a_t04_refresh_normal`, `a_t04_refresh_reuse` |
| T06 | Administrative/user refresh revocation | I | subordinate to U02/U04/U06/U09/U10/U19/C06 | `refresh_tokens::revoke`, `revoke_all_for_user`, `revoke_all_for_client` | `i_t06_refresh_revoke` |
| T07 | Access-token revocation | I | subordinate to U20/U19/C06 or another inventoried revoke command | `revoked_access_tokens::insert` | `i_t07_access_revoke` |
| T08 | Refresh hash backfill | O | idempotent startup migration/maintenance with structured telemetry | `refresh_tokens::backfill_token_hashes` | `o_t08_hash_backfill` |
| T09 | Initial root-family refresh-token issue | P | high-frequency protocol issuance; authorization/login events are separate and this exclusion is not audit-equivalent | initial-issuance call site of `refresh_tokens::insert` | `p_t09_refresh_initial_issue` |

## Settings, setup, keys, and operational state

| ID | Logical command / owner | Class | Typed event or rationale | Current mutation surface | Required test ID |
|---|---|---|---|---|---|
| S01 | Change default language | A | `settings.default_language.changed` | `server_settings::update_default_lang` | `a_s01_language` |
| S02 | Change HIBP mode | A | `settings.hibp_mode.changed` | `server_settings::update_hibp_mode` | `a_s02_hibp` |
| S03 | Change idle timeout | A | `settings.idle_timeout.changed` | `server_settings::update_idle_session_timeout` | `a_s03_idle` |
| S04 | Change concurrent-session limit | A | `settings.max_sessions.changed` | `server_settings::update_max_concurrent_sessions` | `a_s04_sessions` |
| S05 | Rotate metrics token | A | `settings.metrics_token.rotated` | `server_settings::update_metrics_token_hash` | `a_s05_metrics_token` |
| S06 | Change SMTP configuration | A | `auth.smtp_config.changed` | `smtp_config::upsert` | `a_s06_smtp` |
| S07 | Create pending sensitive change | A | `settings.pending_change.created` | `pending_settings_change::insert` | `a_s07_pending_create` |
| S08 | Apply pending sensitive change | A | `settings.pending_change.applied` with typed intent/changed-field attributes | `pending_settings_change::consume` and setting mutation | `a_s08_pending_apply` |
| S09 | Cancel pending sensitive change | A | `settings.pending_change.cancelled` | `pending_settings_change::cancel` | `a_s09_pending_cancel` |
| S10 | Complete first setup | A | `admin.setup.completed` | initial admin/client/config writes and `state::mark_initialized` | `a_s10_setup` |
| K01 | Rotate signing key | A | `signing_key.rotate` | `signing_keys::rotate_atomic` | `a_k01_signing_rotate` |
| K02 | Retire signing key | A | `signing_key.retire` | `signing_keys::retire` | `a_k02_signing_retire` |
| K03 | Delete signing key | A | `signing_key.delete` | `signing_keys::delete` | `a_k03_signing_delete` |
| K04 | Master-key DB reseal phase | A | `admin.master_key.database_resealed` | all `reseal_all` helpers plus rotation-state row | `a_k04_master_db` |
| K05 | Master-key activation completion | A | `admin.master_key.activated` | rotation-state completion after atomic file replacement | `a_k05_master_activate` |
| K06 | Sealed-key insert/reseal primitives | I | subordinate to S10, K01, or K04 only | `signing_keys::insert_with_plaintext`, `insert_sealed_on_conn`, all `reseal_all` functions | `i_k06_key_primitives` |
| O01 | Enqueue email | O | delivery queue state; originating security command owns its event | `email_outbox::enqueue` | `o_o01_enqueue` |
| O02 | Claim/update/requeue email work | O | worker lifecycle with structured operational telemetry | `claim_one_eligible`, `mark_sent`, `record_failure`, `mark_permanently_failed`, `requeue_stuck_sending` | `o_o02_outbox_worker` |
| O03 | Purge expired protocol rows | O | retention housekeeping; count/error telemetry | purge functions in auth codes, sessions, pending MFA/WebAuthn, reset tokens, pending changes, refresh/access tokens | `o_o03_purge` |
| O04 | Create SQLite backup snapshot | O | operator-invoked backup evidence owns outcome; no domain mutation in the live database | `backup::ops` `VACUUM INTO` destination | `o_o04_backup_snapshot` |
| X01 | Schema migrations | X | ordered migration identity/result in upgrade evidence | `Database` migration runner and migration SQL only | `x_x01_migrations` |
| X02 | Development seed/reset | X | dev-only, unreachable in production mode; dev warning/summary | `runtime::dev_mode` writes | `x_x02_dev_seed` |

## Completeness reconciliation

The implementation-generated manifest expands every grouped mutation surface
above to exact function and SQL-write-site identifiers. Stage 0 is complete
only when:

1. every current public repository writer named by source inspection maps to
   exactly one row above or becomes private beneath one row;
2. every direct SQL `INSERT`, `UPDATE`, `DELETE`, schema DDL, and write pragma
   outside migrations maps to a registered capability;
3. setup, CLI, worker, startup/backfill, and dev-only direct writes map to A,
   P, O, X, or become impossible;
4. the independent reviewer compares the generated source-site list with this
   inventory and records every discrepancy before RFC acceptance.

Read-only repository functions and pure helpers are outside the durable-write
universe. `audit::append` is not a domain command: Class-A callers reach
`append_within_tx` only through the transaction runner; Class-B callers reach a
must-attempt emitter. Neither raw append API remains public to domain code.

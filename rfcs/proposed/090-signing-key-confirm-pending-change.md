# RFC 090 — Signing-Key Rotation Confirm Page and Settings Pending-Change Object

**Status.** Proposed
**Tracks.** UI/UX handoff v2.3 §3.6, §4 (Signing Keys), §6 — unit 4.
Category A. Prerequisite: RFC 089 (step-up allowlist) must ship first.
**Touches.** `sui-id-store` (new `pending_settings_change` table + migration),
`sui-id-core/src/settings.rs` (pending-change create/apply/cancel/purge),
`sui-id/src/handlers/admin/signing_keys.rs` (rotate-confirm page),
`sui-id/src/handlers/settings.rs` (high-risk fields use pending-change),
`sui-id-web/src/pages/` (new confirm page, updated settings pages), i18n.

## Summary

Two closely related deliverables in unit 4, both gated on step-up:

1. **Signing-key rotation confirm page.** The rotation flow gains a GET
   confirm page (with step-up gate) before the POST that issues a new key,
   matching the v2.3 contract that reclassified key rotation as a sensitive
   operation.

2. **Settings pending-change object.** High-risk settings that include a
   secret (currently: SMTP password) never pass through hidden form fields.
   Instead, on the first POST the handler stores an encrypted, session-bound,
   expiring `pending_settings_change` row and redirects to a confirm page
   that carries only a `pending_change_id`. The final confirm POST reads the
   pending row, revalidates all invariants, then applies and deletes it.

## Motivation

### Signing-key rotation

The current `POST /admin/signing-keys/rotate` handler performs the rotation
immediately without a confirm page. v2.3 §4 reclassifies this as a sensitive
operation: it affects all OIDC clients (invalidates active sessions / cached
JWKS) and should require step-up + explicit confirmation.

### Secrets in hidden fields

v2.2 P1-1 (blocking before implementation): "High-risk settings that include
secrets (SMTP password) now use a **server-side pending-change object**; the
confirm page carries only a `pending_change_id` and a non-secret summary."
The current SMTP settings form has no confirm page at all — it applies
immediately. This RFC adds the pending-change path.

## Background

From §3.6 of the v2.3 handoff:
> "The confirm page carries only a `pending_change_id` and a non-secret
> summary. Non-secret high-risk fields may still use hidden fields,
> revalidated on final POST."
>
> "Pending changes are single-use, deleted on apply, invalidated on cancel,
> purged on expiry."
>
> "Audit entries record creation, confirmation, cancellation, expiry, and
> application using non-secret summaries only."

From Appendix E (acceptance criteria):
> - SMTP password never appears in hidden fields.
> - Confirm page uses `pending_change_id` only.
> - Pending payload is encrypted at rest.
> - Pending change is bound to session, actor, CSRF, expiry, and intent.
> - Apply consumes and deletes the pending change.
> - Reuse returns a neutral expired-or-invalid error.
> - Cancel invalidates the pending change when possible.
> - Expired pending changes are ignored and purged.

## Target code areas

### New store: `pending_settings_change`

```sql
-- Migration 0032
CREATE TABLE pending_settings_change (
    id          TEXT PRIMARY KEY,    -- UUID, the pending_change_id
    session_id  TEXT NOT NULL,       -- bound to the creating session
    actor_id    TEXT NOT NULL,       -- bound to the creating admin
    intent      TEXT NOT NULL,       -- e.g. "smtp_password_update"
    payload_enc BLOB NOT NULL,       -- MasterKey-encrypted JSON payload
    summary     TEXT NOT NULL,       -- non-secret human-readable summary
    csrf_token  TEXT NOT NULL,       -- CSRF token for the confirm POST
    expires_at  TEXT NOT NULL,       -- 5-minute TTL
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_pending_settings_change_expires
    ON pending_settings_change (expires_at);
```

### Core: `pending_change.rs` (new in `sui-id-core`)

```rust
pub struct PendingChange {
    pub id: PendingChangeId,      // newtype UUID
    pub intent: String,
    pub summary: String,          // non-secret, for display
    pub expires_at: DateTime<Utc>,
}

pub async fn create(
    db: &Database,
    actor: &AdminActor,
    session_id: SessionId,
    intent: &str,
    payload: &impl Serialize,  // encrypted with MasterKey before storage
    summary: &str,
    csrf_token: &str,
    clock: &SharedClock,
) -> CoreResult<PendingChange>;

pub async fn apply<T: DeserializeOwned>(
    db: &Database,
    id: PendingChangeId,
    actor: &AdminActor,
    session_id: SessionId,
    csrf_token: &str,
    clock: &SharedClock,
) -> CoreResult<T>;  // decrypts, validates, returns; deletes row

pub async fn cancel(db: &Database, id: PendingChangeId) -> CoreResult<()>;
pub async fn purge_expired(db: &Database, clock: &SharedClock) -> CoreResult<usize>;
```

### Handler changes

**Signing-key rotation** (`handlers/admin/signing_keys.rs`):
- Add `GET /admin/signing-keys/rotate-confirm` (already has a delete-confirm
  route as the pattern; follow the same structure). Step-up gated via
  `require_fresh_step_up`. The RFC 088 fix already guards this GET with
  `can_write()`.
- The existing `POST /admin/signing-keys/rotate` becomes the confirm POST
  (after rendering the confirm page). Step-up is verified again on the
  final POST (revalidate).

**SMTP settings** (`handlers/settings.rs`):
- On `POST /admin/settings/email` when the payload includes a new password:
  call `pending_change::create(...)`, redirect to
  `/admin/settings/email/confirm?pending_change_id={id}`.
- Add `GET /admin/settings/email/confirm` — renders the non-secret summary.
- Add `POST /admin/settings/email/confirm` — calls `pending_change::apply`,
  applies the stored settings.

**Other high-risk settings** (secure cookies, idle timeout, max sessions,
HIBP mode, lockout policy) use non-secret hidden fields, revalidated on
the final POST per the field-level risk matrix. These do **not** use the
pending-change object (only secrets require it).

## Security properties / invariants

- **P1 (no secret in form fields).** The SMTP password never appears in
  any HTML `<input>`, hidden or otherwise, after the initial entry form.
  The pending-change object encrypts it at rest.
- **P2 (binding).** A pending change is valid only for the session and actor
  that created it. A different session or actor receiving the `pending_change_id`
  cannot apply it.
- **P3 (single-use).** Apply deletes the row. Reuse returns a neutral
  `expired-or-invalid` error without revealing which condition triggered it.
- **P4 (expiry).** Pending rows expire after 5 minutes (matching the
  step-up freshness window). `purge_expired` runs at startup and periodically
  via a background task.
- **P5 (final-POST revalidation).** The confirm POST revalidates: admin
  role (via `AdminActor`), CSRF token, step-up freshness, expiry, and
  current-state bounds (e.g. the new SMTP config is still valid).
- **P6 (non-secret audit log).** Audit entries for pending-change events
  carry only the intent string and non-secret summary. The encrypted payload
  is never logged.

## Data model impact

New table `pending_settings_change` (migration 0032). No changes to
existing tables.

## API impact

Two new routes:
- `GET /admin/settings/email/confirm`
- `POST /admin/settings/email/confirm`

One new confirm-page flow for signing-key rotation (route already exists
as `GET /admin/signing-keys/rotate-confirm`; the handler body gains the
step-up gate and confirmation rendering).

## Testing strategy

- Unit (`sui-id-store`): pending change insert, apply, cancel, expiry purge.
- Unit: binding validation (wrong session_id → error).
- Unit: single-use (apply twice → second returns error).
- Integration (core): `pending_change::apply` decrypts and returns correct
  payload; audit row emitted with non-secret summary.
- Handler tests are deferred to the binary-crate; behaviour verified
  end-to-end in real-environment soak.

## Migration strategy

Migration 0032 is additive (new table). No existing rows change.

## Rollout plan

Ships as v0.72.0. Depends on RFC 089 (step-up allowlist) having shipped
first so the confirm pages can be added to the allowlist.

## Risks and mitigations

- *Risk:* pending-change TTL (5 min) is shorter than the time a slow admin
  might take to review the confirm page. Mitigation: 5 min matches the
  step-up freshness window and Appendix E explicitly recommends it.
  The confirm page shows the expiry time; on expiry the user is asked to
  restart the flow.
- *Risk:* master-key rotation while a pending change is open leaves an
  undecryptable payload. Mitigation: on `apply`, decryption failure returns
  the neutral expired-or-invalid error; the user restarts the flow.

## Acceptance criteria

All items from Appendix E §"Pending settings changes" are met.
Signing-key rotation requires a step-up-fresh session before the confirm
page renders. The final confirm POST revalidates authorization, CSRF, and
step-up freshness. Non-secret fields still work with hidden fields.

## Open questions

- Should `pending_settings_change.payload_enc` use the same
  `MasterKey::encrypt_blob` helper used for `smtp_config`? Yes — reuse.
- Is a background purge task needed in v0.72.0 or is startup-time purge
  sufficient? Startup-time purge is sufficient for minimal production.
  Background purge is a follow-on improvement.

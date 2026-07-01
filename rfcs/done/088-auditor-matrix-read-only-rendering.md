# RFC 088 — Auditor Authorization Matrix and Static Read-Only Rendering

**Status.** Implemented (v0.70.0)
**Tracks.** UI/UX handoff v2.3 §3.5 — unit 2. Category B.
**Touches.** `sui-id/src/handlers/admin/` (clients, users, signing_keys,
settings, audit, dashboard, webauthn), `sui-id/src/handlers.rs` (error
path for auditor on mutation-only GET routes), `sui-id-web/src/pages/`
(client edit/detail rendering mode), i18n keys.

## Summary

Complete the v2.3 auditor authorization matrix contract (§3.5): all
mutation-only GET routes return a 403 page (not a session redirect) when
reached by an auditor session; client edit renders static read-only rows
when the actor is an auditor; auditors never see mutation controls on any
admin surface. The structural split (`CurrentAdmin` vs
`CurrentAdminOrAuditor`) already exists from RFC 081 — this RFC wires the
remaining routes and adds the read-only rendering mode.

## Motivation

RFC 081 established `AdminActor`/`ReadOnlyAdminActor` and updated the
extractors so `CurrentAdmin` can only succeed for Admin-role sessions.
That already enforces **POST-route protection** structurally. What remains:

1. `GET /admin/users/new`, `GET /admin/clients/new`,
   `GET /admin/signing-keys/rotate-confirm` — currently gated with
   `CurrentAdmin`, so auditors get a 401/redirect rather than the correct
   403 page. The extractor should be changed so these GET routes also
   return a proper 403 response.

2. `GET /admin/clients/{id}/edit` — the v2.3 contract requires this route
   to render *editable* fields for admins and *static text rows* for
   auditors, sharing the same URL. The render function must accept a
   `mode: ClientDetailMode` discriminant derived from the actor's role.
   Auditor title: "App details"; admin title: "Edit app".

3. `GET /admin/settings/*` — auditors may currently see disabled form
   controls. v2.3 §3.5 requires static/definition-list rows, not disabled
   inputs (which are not keyboard-focusable and carry implicit "editable
   but not right now" semantics the auditor should not see).

4. **Auditor 403 copy and page.** The 403 error page needs i18n keys for
   the auditor context ("You have read-only access. This action requires
   administrator privileges.") distinct from generic Forbidden.

## Background

From §3.5 of the v2.3 handoff:
> "The server enforces every mutation restriction in the handler, independent
> of whether UI controls were hidden. Hidden controls are a UX nicety, not
> the security boundary."

RFC 082's `authz::authorize(role, action)` table is the decision source.
RFC 081's `ReadOnlyAdminActor` is proof of read-only status — it is already
produced by the `CurrentAdminOrAuditor` extractor and propagated into
domain read functions.

## Target code areas

1. **`handlers/admin/users.rs`** — `users_new_get` changes extractor from
   `CurrentAdmin` to `CurrentAdminOrAuditor`; if `!actor.can_write()` return
   `HttpError::html(CoreError::Forbidden)`. Same for any other user-mutation
   GET routes.

2. **`handlers/admin/clients.rs`** — `clients_new_get` and
   `clients_edit_get` updated. Edit GET: extract `CurrentAdminOrAuditor`,
   derive `mode = if actor.can_write() { Edit } else { ReadOnly }`, pass
   to renderer.

3. **`handlers/admin/signing_keys.rs`** — `signing_keys_rotate_confirm_get`
   changes to `CurrentAdminOrAuditor` + 403 for auditor.

4. **`handlers/admin/settings.rs`** — all `*_get` handlers pass role to
   renderer; renderer emits `<dl>` static rows for `ReadOnly` mode instead
   of `<form>` with fields.

5. **`sui-id-web/src/pages/clients.rs`** (and related render functions) —
   add `ClientDetailMode { Edit, ReadOnly }` to the `ClientDetailData`
   struct; renderer conditionally emits edit form vs. definition list.

6. **i18n** — add `error_403_auditor_title`, `error_403_auditor_body`,
   `client_detail_readonly_title` ("App details") across all locales.

## Security properties / invariants

- **P1 (matrix completeness).** Every row in the v2.3 §3.5 matrix is
  enforced server-side. The audit-matrix CI gate (`scripts/check-audit-matrix.sh`)
  is not directly relevant here; a separate handler-coverage grep will verify.
- **P2 (no auditor mutation).** No state-changing handler is reachable with
  a `ReadOnlyAdminActor` — already guaranteed by RFC 081 for POST routes;
  this RFC closes the remaining GET mutation paths.
- **P3 (read-only rendering).** Auditors never receive a form with a submit
  button on admin surfaces; they receive static rows. The rendering is
  discriminated by the `mode` value derived from the actor, not from a
  query parameter.
- **P4 (403 not 401).** Auditors reaching a mutation-only route get a 403
  ("read-only access") page, not a re-authentication prompt. The
  distinction matters: 401 implies "try again with better credentials";
  403 means "your credentials are correct but insufficient."

## Non-goals

- No changes to step-up (that is unit 3 / RFC 089).
- No changes to the auditor role definition or to how sessions acquire
  auditor role (those are RFC 071, already done).
- No new admin routes.

## Proposed design

`ReadOnlyAdminActor::can_write() -> bool` (added in RFC 081) is the
discriminant. All places that branch on write capability use this method
rather than re-checking the role. This keeps the logic single-sourced in
the capability type.

```rust
// In a handler that currently gates a mutation-only GET:
async fn clients_new_get(
    state: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(HttpError::html_403_auditor()); // new helper
    }
    // ... existing render logic
}
```

```rust
// In client edit handler:
async fn clients_edit_get(…) -> Result<Response, HttpError> {
    let mode = if actor.can_write() {
        ClientDetailMode::Edit
    } else {
        ClientDetailMode::ReadOnly
    };
    // pass mode to renderer
}
```

Settings tabs: each GET handler already fetches the settings and passes
them to a render function. Add `can_write: bool` to the data structs;
renderers emit `<form>` when `can_write` and `<dl class="settings-readonly">`
when not.

## Data model impact

None.

## API impact

None (routes unchanged, HTTP status codes corrected for auditor on mutation-
only GETs: 403 instead of 401-redirect).

## Testing strategy

- Unit: `CurrentAdminOrAuditor` with `Role::Auditor` hitting `clients_new_get`
  → 403 status.
- Unit: same route with `Role::Admin` → 200 (render).
- Unit: `clients_edit_get` with Auditor actor → `ClientDetailMode::ReadOnly`
  in rendered HTML (mode-discriminated heading "App details" present).
- Existing matrix: grep that no admin mutation POST uses
  `CurrentAdminOrAuditor` (they must all use `CurrentAdmin`).
- 403-vs-401 test: auditor session hitting a mutation-only GET returns
  status 403, not 302/401.

## Migration strategy

No data migration. Auditors already cannot reach mutation POSTs. This
RFC extends the protection to mutation-only GETs and corrects the error
type from redirect-to-login to 403.

## Rollout plan

Ships as v0.70.0. Small surface: ~10 handlers modified, 1 new data-struct
field, 3 i18n keys per locale, 1 renderer mode added.

## Risks and mitigations

- *Risk:* missed mutation-only GET routes. Mitigation: grep for
  `CurrentAdmin` on GET handlers after implementation; all should have
  been converted to `CurrentAdminOrAuditor` + can_write guard.
- *Risk:* read-only rendering breaks settings layout. Mitigation: `<dl>`
  rows with consistent key/value CSS already exist in the codebase (audit
  detail uses them); reuse the same tokens.

## Acceptance criteria

- Every row in the v2.3 §3.5 matrix table is enforced server-side per
  the table's "Auditor behavior" column.
- Auditor session hitting any mutation-only GET returns HTTP 403 with
  the auditor-specific copy, not a login redirect.
- `GET /admin/clients/{id}/edit` with auditor session renders "App details"
  heading and static rows; with admin session renders "Edit app" and form.
- Settings GET routes render `<dl>` for auditors, `<form>` for admins.
- 3 new i18n keys in all locale files; CI text-leaks gate still passes.
- 0 warnings; baseline test suite green.

## Open questions

- Should `GET /admin/users/{id}/delete-confirm` and
  `GET /admin/users/{id}/reset-mfa-confirm` also return 403 for auditors?
  They are mutation-only confirm pages — yes, per the matrix (rows marked
  ✗). Confirm in implementation.

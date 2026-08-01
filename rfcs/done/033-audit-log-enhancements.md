# RFC 033 — Audit log enhancements

**Status.** Implemented (v0.36.0)
**Priority.** Medium. The audit log is described as a "forensic surface"
in the design document. Hash-chain verification status must appear on the
audit screen itself, not only in Settings → Logs.  
**Source.** UI/UX design document P.12; RFC 017 § 8.  
**Touches.** `crates/sui-id-web/src/pages.rs` (`render_audit`),
`crates/sui-id/src/handlers/admin.rs` (audit handler),
`crates/sui-id-i18n` (filter/export labels).

## Missing features

### 1. Hash-chain status banner

**Design requirement (RFC 017 § 8):**
> A persistent banner at the top of the audit log screen shows
> "Audit chain verified through row N (last checked HH:MM)."
> A mismatch shows red and links to the operator runbook.

**Current state:** hash-chain status is only in Settings → Logs tab
(`render_settings_logs`). The audit log screen has no such indicator.

**Fix:** The audit handler already has access to the chain verification
result (it's computed in the settings handler). Extract the verification
into a shared helper and call it from both. Pass the result to
`render_audit` as `chain_ok: bool, verified_through: Option<i64>`.

### 2. Event-name prefix filter

**Design requirement:** "Filters are minimal: time range, event-name prefix,
actor/target user."

**Implementation:** A simple `<input type="search">` with `name="q"` submitted
as a GET query parameter. The handler calls
`audit::list_filtered(db, actor_id, query, limit)` which adds a
`WHERE event LIKE ?1 || '%'` clause.

### 3. Copy row ID button

**Design requirement:** "A 'copy row ID' button on each row."

**Implementation:** Use the existing `copy_btn()` helper (RFC 028) on the
`id` field of each audit entry. The audit DTO already carries the row `seq`
(sequence number); expose the UUID as the copyable identifier.

### 4. CSV export of filtered rows

**Design requirement:** "A CSV export of the filtered rows."

**Implementation:** A `GET /admin/audit.csv?q=...` route that runs the same
query and returns `Content-Type: text/csv` with columns:
`seq, when, actor, event, target, result, note`.

### 5. `lang` parameter

Aligned with RFC 029: pass `lang: Locale` to `render_audit` and use
`t.audit_*` keys for all column headers and UI labels.

## `render_audit` signature change

```rust
pub fn render_audit(
    entries: Vec<AuditLogEntryDto>,
    chain_ok: bool,
    verified_through_seq: Option<i64>,
    filter_query: Option<String>,
    flash: Option<Flash>,
    lang: Locale,
) -> String
```

## New `Strings` fields

```
audit_chain_ok_banner / audit_chain_broken_banner
audit_filter_label / audit_filter_placeholder
audit_export_csv / audit_copy_row_id
```

## Tests

- E2E: Audit page shows hash-chain OK banner after a clean chain.
- E2E: Filter `?q=auth.login` returns only login events.
- E2E: `/admin/audit.csv` returns valid CSV with correct headers.

## Version

Patch bump (no schema migration; new read-only query and export endpoint).

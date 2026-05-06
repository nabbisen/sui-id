# RFC 002 — i18n scope expansion (post-v0.23.0)

**Status.** Proposed
**Tracks.** ROADMAP / Medium term — "i18n scope expansion (post-v0.23.0)".
**Touches.** `sui-id-i18n` (new locales, formatting helpers),
`sui-id-web` (RTL CSS pass), `sui-id-core` (per-recipient locale
resolution for outbound mail), `docs/` (translator's guide).

## Summary

v0.23.0 shipped the typed `sui-id-i18n` foundation — `Locale` enum,
`Strings` struct with compile-time exhaustiveness, two locales (Ja, En).
v0.29.0 and v0.29.1 widened the per-screen coverage. This RFC plans
the *next* expansion axis: more locales, polished formatting, polished
mail templates, audit-event labels, and right-to-left layout. Each
sub-thread is independently shippable; this document is the umbrella
that ties them together so the maintainer can sequence them without
re-deciding the same questions five times.

## Sub-threads (each ships separately)

The work has five threads. Implementer takes them one at a time.

### A. More locales

**Adding `Locale::Zh`, `Locale::Ko`, etc.**

Mechanical. The compile-time `match` in `Locale::strings()` enforces
that every `Strings` field has a translation; the type system tells
the contributor exactly what's missing. No schema migration, no new
crate dependency, no API change for callers.

Procedure (also captured in `docs/contributing/translators.md`):

1. Add the variant to `Locale` (`crates/sui-id-i18n/src/lib.rs`)
   plus its BCP-47 tag in `tag()` and a native-script display name
   in `native_name()`.
2. Add the variant to `Locale::ALL` so it appears in language
   pickers.
3. Create `crates/sui-id-i18n/src/<lang>.rs` with the
   `pub static STRINGS_<LANG>: Strings = Strings { … };` literal,
   matching the field-by-field shape of `ja.rs` / `en.rs`.
4. Add the variant arm in `Locale::strings()` and `Locale::parse()`.
5. Update tests in `crates/sui-id-i18n/src/tests.rs` that iterate
   `Locale::ALL` (most already do this — there is no per-locale test
   list to maintain).

The first non-CJK locale will surface latent assumptions about
field ordering and capitalisation; the second non-CJK locale
won't. Plan for the first to take real review effort and the
rest to be near-mechanical.

Specific languages will be prioritised by deployment demand, not
selected speculatively.

### B. Locale-aware date and number formatting

Today everything renders through the same `chrono::DateTime::to_rfc3339()`
or equivalent ISO-ish format. This is fine for operators and
auditors; it's wrong for end-user-facing screens like
`/me/security`'s "last sign-in" timestamp.

**Approach.** Add a thin `Formatters` struct alongside `Strings`
in `sui-id-i18n`:

```rust
pub struct Formatters {
    pub fmt_date:           fn(DateTime<Utc>) -> String,
    pub fmt_time:           fn(DateTime<Utc>) -> String,
    pub fmt_date_time:      fn(DateTime<Utc>) -> String,
    pub fmt_relative:       fn(DateTime<Utc>, DateTime<Utc>) -> String, // "3 hours ago"
    pub fmt_count:          fn(u64) -> String,
}
```

Each locale provides its own `static FORMATTERS_<LANG>: Formatters`.
For the common case (Ja, En, Zh) we can use `chrono`'s built-in
`%Y/%m/%d` style strings; for Arabic we'd use the locale's
preferred numerals etc. via a minimal helper.

We deliberately do **not** depend on `icu` or `rust-icu` at this
tier. The data we need is small, locales we ship are bounded,
and the audit trail of "why does this date render this way" is
easier to read with hand-written formatters than with a
data-table-driven library.

`Strings` doesn't grow — formatters are a peer struct. View
code calls `lang.formatters().fmt_date_time(when)` instead of
the current ad-hoc `chrono` calls.

### C. Per-recipient locale for outbound email

Today all outbound mail (forgot-password link, password-changed
notification) is rendered in a single locale — the one resolved
from the *requesting* request, which is approximately right but
gets the wrong answer when an operator triggers a password reset
on behalf of a user with a different `preferred_lang`.

**Approach.** Mail-sending code paths take a `recipient_user_id`
and look up `users.preferred_lang` to pick the locale. The
resolution chain becomes:

1. `users.preferred_lang` of the *recipient* (not the requester).
2. `server_settings.default_lang`.
3. Hardcoded `Locale::Ja`.

Cookie / `Accept-Language` are skipped — they're attributes of
the request, and the recipient may not be the requester.

Touches `sui-id-core::mail`. The function that today takes
`(to: &str, locale: Locale, …)` becomes
`(to: &str, recipient_user_id: Option<UserId>, …)` and resolves
locale internally. `Option` because forgot-password is allowed
to fire against an unknown email (the user-enumeration-safe
shape from v0.22.0); in that case we fall back to step 2.

This thread should *follow* RFC 001 (email outbox) — the
outbox row needs to record the resolved locale so the worker
renders against the same answer the request would have. If the
outbox lands first, this thread gains a `locale` column on
`email_outbox` and the worker uses it; if this thread lands
first, mail still renders inline and the locale is computed at
the request thread.

### D. Audit-event human labels

Audit *event names* are stable English identifiers — operators
query against them and translation would break the contract.
Audit *human-readable labels* (the words shown in the admin UI's
audit log table) and *long-form descriptions* are translatable.

Today the labels exist in `sui-id-i18n` but the long descriptions
("This event is recorded when an administrator creates a new
client") are still English-only and mostly inline as comments
rather than as a translation table.

**Approach.** Add a new field group to `Strings`:
`audit_desc_<event_name_dotted_to_underscore>`. Example:
`audit_desc_admin_clients_created`. Each locale fills these
in. The audit log UI renders the description on row hover or
in an expanded detail view.

Estimated scope: ~80 audit events × 2 locales = 160 short
descriptions. Mechanical to add but real translator effort.

### E. Right-to-left support

When a RTL locale (Arabic, Hebrew, Persian) lands, the design
language needs a `[dir="rtl"]` pass:

- Mirror flex/grid alignments that are currently `flex-start`
  / `flex-end` to be logical (`start` / `end` map to physical
  in CSS, so `text-align: start` already does the right thing —
  the work is finding spots that hard-coded `left` / `right`).
- Mirror padding-left / padding-right where appropriate. Use
  `padding-inline-start` / `padding-inline-end` going forward.
- Verify keyboard-shortcut hint icons (none currently, but if
  added).
- `<html lang>` already reflects the resolved locale; add a
  parallel `dir` attribute set from `Locale::direction()`
  (a new method returning `"ltr"` or `"rtl"`).

This thread is a CSS audit, not a functional change. Best done
as a single review pass once the *first* RTL locale is being
added — there's nothing to verify against until then.

## Tests

- Per locale: assert `Locale::ALL.iter().for_each(|l|
  assert!(l.strings().button_save.len() > 0))` already exists;
  no further test infrastructure needed for sub-thread A.
- Sub-thread B: snapshot tests on `fmt_date_time` for one
  representative date per locale. Catches accidental ICU-style
  dependencies sneaking in.
- Sub-thread C: e2e test that POSTs `/forgot-password` for a
  user with `preferred_lang='en'` from a request that
  `Accept-Language: ja` — assert the captured mail body is in
  English.
- Sub-thread D: fixture tests that load each locale's
  `audit_desc_*` set and assert no field is empty (catches
  half-finished translation work).
- Sub-thread E: visual regression deferred — no automated CSS
  regression infrastructure in sui-id today; covered by manual
  review at first RTL locale add.

## Security considerations

None new. Translation tables are static `Strings` and cannot
contain HTML — the view layer escapes everything. The audit-event
*identifiers* remain English and stable, so log search and
intrusion-detection queries are unaffected by translation work.

The one subtle concern is sub-thread C (per-recipient mail
locale): leaking which locale a given user prefers is
information disclosure, but `users.preferred_lang` is already
visible to administrators and the recipient sees their own
preferred locale anyway — net new exposure is zero.

## Open questions

- For sub-thread B, is "3 hours ago" relative formatting worth
  the complexity in admin-facing screens, or do we keep
  absolute timestamps everywhere there?
  Recommendation: absolute timestamps in admin UI (operators
  want exact times for audit), relative timestamps optionally
  in `/me/security`.
- For sub-thread D, ~160 description strings is a lot of
  translator work. Is the long-form description necessary, or
  can the admin UI show just the event name + the audit log
  row's `result` field?
  Recommendation: punt on long-form descriptions until an
  actual user request surfaces. The label (already translated)
  is enough.

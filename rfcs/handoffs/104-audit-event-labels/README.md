# Audit event labels — show the translated name, bound to the registered events

**Authorized by.** [`ROADMAP.md`](../../../ROADMAP.md) §Non-RFC work packages — owner
ruling, 2026-09-16: **wire the labels in** (option (a)), under the standing
principle of a finally clean, safe and secure, robust and sophisticated design,
and the project rule that the GUI supports multiple languages. The rejected
alternative was to delete them.
**Implementer.** Mid-capability model. **Baseline.** `49a2897` or later.

## Why

The audit log page and the dashboard show the raw action string —
`auth.login.failure` — in every language (`crates/sui-id-web/src/pages/audit.rs:36`,
`crates/sui-id-web/src/pages/dashboard.rs:329`). Translated labels for audit events
were added with RFC 002 on 2026-06-05 and never read by any code. Unenforced, they
drifted like every other unchecked copy of the event vocabulary. Measured at the
ruling against `ci/audit-coverage-matrix.md` (55 registered events):

| | Count |
|---|---|
| `audit_event_*` fields in `Strings` | 29 |
| registered events with a label | 25 |
| **registered events with no label** | **30** |
| **labels for no registered event** | **4** — `auth_login_locked`, `auth_login_mfa_required`, `auth_logout`, `auth_session_revoked` (old or never-written names) |

## Design — decided; do not re-derive

**One source for each fact.** The registered event set is
`ci/audit-coverage-matrix.md` (G13 keeps it true against the code). The label text
lives in `crates/sui-id-i18n` locale files. A test binds the two, so neither can
drift from the other.

1. **Lookup.** In `sui-id-i18n`, a method on `Strings` —
   `pub fn audit_event_label(&self, action: &str) -> Option<&'static str>` — mapping
   a registered action name to its label. A `match` on the action string is
   acceptable and clear; keep it in one place.
2. **The label set equals the registered set.** Remove the four stale fields. Add
   the thirty missing ones to `strings.rs` and to `en.rs`, `ja.rs` and
   `zh_hans.rs`. `zh_hant.rs` is a documented stub that delegates to Simplified
   Chinese; leave it delegating. Follow `docs/src/contributing/translators.md` and
   the wording style of the existing 25 labels: short, noun-phrase, no trailing
   punctuation.
3. **Binding test, both directions,** in `sui-id-i18n`'s sibling test file. It
   reads `ci/audit-coverage-matrix.md` from the workspace root and extracts the
   event names from the first column of every table (the same rule G15 check (D)
   uses: a first cell that is exactly one inline-code dotted name). It fails if a
   registered event has no label in any of `en`, `ja` or `zh_hans`, and if
   `audit_event_label` returns a label for any name that is not registered.
   Mutation-test both directions.
4. **Display.** On the audit page and the dashboard, show the localized label, with
   the raw action string beside it in `<code>` — the identifier operators query,
   filter and alert on stays visible and unchanged. If `audit_event_label` returns
   `None` (an unregistered action in an old row), show the raw string alone; never
   an empty cell.
5. **Unchanged, on purpose.** The audit filter matches on the raw action prefix; the
   copyable row identifier and the CSV export carry the raw action. Do not localize
   any of them — they are machine-facing.
6. **The reader reference.** `docs/src/reference/audit-events.md` has a *Label*
   column, currently `—` for nineteen rows. Fill it from the English labels, and
   extend the binding test so the reference's Label cell must equal the English
   label for its event. That keeps `en.rs` the single source and the reference a
   checked copy, not a hand-maintained one.

## Commits

1. `sui-id-i18n`: lookup, label set, locale text, binding test.
2. `sui-id-web`: audit page and dashboard display.
3. `docs/src/reference/audit-events.md`: Label column, with the test's reference
   assertion.

## Evidence

- Binding test: passes; each direction mutation-tested and shown failing.
- G12 (UI invariants, including text leaks) green — the new strings must not
  appear as raw keys in rendered HTML.
- A rendered audit page in Japanese and English showing label + raw action
  (paste the relevant HTML fragment for one row of each).
- G13 55/55, G15 (check (D) still green after the Label column fill), G10a/b, G11,
  G14; fmt, both clippy scopes, `cargo test --workspace` count before and after,
  MSRV 1.95.

## Stop and report if

- a registered event's meaning is unclear enough that its label would be a guess —
  read the emitting code; if still unclear, list it rather than invent;
- any other page renders an audit action string not listed above.

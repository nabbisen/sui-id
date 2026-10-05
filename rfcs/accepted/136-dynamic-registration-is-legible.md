# RFC 136 — Dynamic client registration is legible

**Status.** Accepted
**Accepted on.** 2026-10-05
**Approved by.** `@nabbisen`, 2026-10-05: "On both, your recommendations are accepted." — accepting this RFC **and settling its one open question in favour of a marker on the exception** rather than a sortable column.
**Security review.** Required — [security review 2026-10-05](../handoffs/136-dynamic-registration-is-legible/security-review-2026-10-05.md), **by the architect, which authored this RFC, and therefore not independent.** No required changes; it verified that C15 actually stamps `registered_via`, without which D2's marker would never appear.
**Independent design review.** [Security review 2026-10-05](../handoffs/136-dynamic-registration-is-legible/security-review-2026-10-05.md) — same document, same limitation. **The field's name overstates it**, as on RFCs 124, 128, 130, 132, 133, 134 and 135.
**Handoff.** [`../handoffs/136-dynamic-registration-is-legible/README.md`](../handoffs/136-dynamic-registration-is-legible/README.md)

**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted.
**Closure prerequisites.** Both consent-policy defaults are recorded with their reasoning in a place a reader of the code will find; the administrator-facing client list distinguishes a self-registered client from an administrator-created one; and a test fails if the distinction stops being shown.
**Tracks.** Follows the `consent_policy` persistence fix (RFC 094 M2a, `b35adc0`), which closed the bug and left two questions open.
**Touches.** `crates/sui-id-store/src/models.rs` (doc comments), `crates/sui-id-core/src/identity/admin/clients.rs` (doc comment), `crates/sui-id/src/http/handlers/dynamic_register.rs` (doc comment), `crates/sui-id-web/src/pages/clients.rs`, three locale files.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

Two facts about a dynamically registered client are true, deliberate, and
written down nowhere a reader will find them: it gets `ConsentPolicy::FirstTime`
while an administrator-created client gets `None`, and it is marked
`registered_via = 'dynamic'`. **Neither is visible** — the first to anyone
reading the code, the second to the administrator deciding whether to enable the
client. This RFC records the first and shows the second. **`@nabbisen` approved
both on 2026-10-05**, as items 4 and 5 of the open-decisions list.

## Background

The `consent_policy` persistence bug (fixed in `b35adc0`) was that
`clients::create` silently dropped the column, so a dynamically registered
client fell back to the table default of `'none'` and its users were never asked
for consent. The fix was correct and the bound was narrow — RFC 008 P4 means
such a client starts disabled, so it could only bite after an administrator
enabled one.

**What the fix exposed is that the administrator making that decision cannot see
what they are deciding about.** `registered_via` appears **zero times** in
`crates/sui-id-web/src/pages/clients.rs`; the client list shows Name, Client ID,
Kind and Status, and nothing distinguishes a client that registered itself.

## Decisions

### D1 — The two defaults are recorded where the code is

No behaviour change. Doc comments at the three sites that set or define the
defaults, each stating the value **and the reason**:

| Site | Value | Reason to record |
|---|---|---|
| `models.rs:211-219` (`ConsentPolicy`) | `None` is `#[default]` | its own doc already says "first-party default" — say why that makes it wrong for a self-registered client |
| `admin/clients.rs:91` | `ConsentPolicy::default()` → `None` | an administrator-created client **is** the first-party case |
| `dynamic_register.rs:198` | `ConsentPolicy::FirstTime` | a client that registered itself through a protocol endpoint is by construction not first-party |

**Why this is worth a decision rather than a comment someone adds in passing:**
a security-relevant default with no stated rationale invites a future reader to
"simplify" it to the enum's `#[default]`. That is the same shape as the bug this
followed — a value that mattered, held in one place, with nothing saying it
mattered.

**`Always` is rejected, and the reason belongs in the record:** it asks the user
again after they have already decided, which trains people to click through the
screen that exists to make them stop and read.

### D2 — The administrator can see which clients registered themselves

**Mark the exception, not every row.** Self-registered clients get a visible
marker in the client list; administrator-created ones get nothing. The
administrator created those, so the fact is not news.

This is deliberately **not** a fifth data column. The list already carries four
plus actions, and a column that reads "admin" on almost every row costs width on
every screen to tell the reader something they already know.

**Where:** the client list, so the distinction is visible at the moment of
scanning, and the client detail view.

**Not in scope:** the enable-confirmation page. `@nabbisen` placed that after
the federation work on 2026-10-05, and it depends on this — a confirmation that
says "this client registered itself" is meaningless while nothing else in the UI
ever says so.

**i18n cost, measured:** three locale files, not four. `en`, `ja` and `zh_hans`
each carry 717 keys; `zh_hant` is a documented placeholder stub that delegates
to `zh_hans` and is excluded from `Locale::ALL` until translated. **There is no
locale-completeness gate** (`contracts/gate-inputs.toml` has no locale lane), so
a missed key is caught by review or not at all.

## Tests

- A test that **fails if the marker stops being rendered** for a client with
  `registered_via = 'dynamic'`, and one asserting it is absent for `'admin'`.
  Without the second, a marker rendered unconditionally would pass.
- The D1 comments are not testable and are not pretended to be. They are
  checked by review.

## Security considerations

This RFC does not change who may register a client, what policy they get, or
when consent is shown. **It changes only what an administrator can see**, and
the threat it addresses is an administrator enabling a self-registered client
without knowing that is what it is.

**It does not close that path** — RFC 008 P4's disabled-on-creation control is
what stands between dynamic registration and a live client, and dismissing that
control is still one click. Making the fact visible is a precondition for the
confirmation step, not a substitute for it.

## Open questions

**None. Settled at acceptance.**

*Marker or column?* — **marker on the exception**, approved by `@nabbisen` on
2026-10-05. A fifth data column reading "admin" on nearly every row would cost
width on every screen to tell the reader something they already know; marking
only the self-registered clients puts the ink where the information is.

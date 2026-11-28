# The audit CSV export could be forged, and could carry formulas (fixed in `0.95.0`)

**Found.** 2026-11-28, by the architect, in Q5's review (`0.95.0`).
**Status.** **Fixed in `0.95.0`**, released 2026-11-28 (`bc6a16e`, tag `0.95.0` at `93cc6a5`). Held outside the repository
under `.github/SECURITY.md` until that release was published, and published here with it. How it was released:
[`release-0.95.0-instead-of-0.94.1-2026-11-28.md`](release-0.95.0-instead-of-0.94.1-2026-11-28.md).

## What

`GET /admin/audit.csv`, readable by an administrator or an auditor, writes each audit field quoted. The quoting had two
defects:
1. **A `"` inside a value was replaced by three quotes** (`s.replace('"', "\"\"\"")`), not two. A value containing a quote
   ends its field early. The rest of the value is read as more fields, or as a new record, by any CSV reader.
2. **A value beginning with `=`, `+`, `-` or `@` was written as is.** A spreadsheet evaluates it as a formula when the
   export is opened (CWE-1236). One example is `=HYPERLINK("https://…", "…")`, which can carry data out when clicked.
   Others depend on the spreadsheet's settings.

## Who controls the text

**Anyone who can reach the sign-in page.** A failed sign-in for an unknown username records the submitted username, as
typed, as the audit row's `target` (`crates/sui-id-core/src/authn/session.rs`, `record_login_failure`, called for
*"unknown user"*). No filtering happens before it is stored.

So, before `0.95.0`, an unauthenticated visitor could:
- **forge rows or columns in the export**, which an auditor reading it would take as real records. The audit log itself is
  untouched, and its hash chain still verifies; only the export misrepresents it;
- **plant a formula** that runs when an administrator or auditor opens the export in a spreadsheet.

## Since when

The CSV export arrived in `1b0080b` (2026-06-05), and the first release tag containing it is **`0.36.0`**. Every release
from `0.36.0` through `0.94.0` is affected.

## The fix (found and written in Q5's review)

`crates/sui-id/src/http/handlers/admin/audit.rs`, `csv_escape`:
- a `"` is doubled, as RFC 4180 requires;
- a field that begins with `=`, `+`, `-`, `@`, a tab or a carriage return is prefixed with `'`, so a spreadsheet shows it as
  text.

**Test**: `e2e::audit_csv_export::the_csv_export_cannot_be_forged_or_turned_into_formulas_by_a_username`. It drives the real path, four
failed sign-ins with crafted usernames. It then parses the export as RFC 4180 does: every record has six fields, and each
username reads back intact, with the formula ones inert.

**Mutations**, each caught: the triple quote put back; and the neutralisation removed.

## Severity, as the architect reads it

**Moderate.**
- Exploiting it needs an administrator or auditor to export the CSV and open it in a spreadsheet.
- Formula execution beyond `HYPERLINK` depends on the spreadsheet's settings and its warnings.
- Row forgery needs no more than a person reading the export.

It requires no authentication to plant.

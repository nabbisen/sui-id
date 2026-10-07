# Developer Handoff — RFC 136 step 2: the test that holds the marker

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`.**

## Why this exists — a prerequisite sweep, not a new idea

RFC 136 has three closure prerequisites. I measured all three against the code
today:

| # | Prerequisite | Result |
|---|---|---|
| 1 | *"Both consent-policy defaults are recorded with their reasoning in a place a reader of the code will find"* | **met** — `identity/admin/clients.rs:91` records the admin default (`ConsentPolicy::default()` → `None`) and `dynamic_register.rs:264` records the dynamic one (`FirstTime`, *"not `ConsentPolicy::default()`"*), each citing RFC 136 D1 with its reasoning |
| 2 | *"the administrator-facing client list distinguishes a self-registered client from an administrator-created one"* | **met** — `pages/clients.rs:37` renders `StatusKind::SelfRegistered` when `registered_via == "dynamic"`, and nothing for `"admin"` |
| 3 | *"a test fails if the distinction stops being shown"* | **NOT met** |

**There is no such test.** `grep` for `registered_via` finds tests only in
`sui-id-store` and `sui-id-core`, all about persistence and redirect-URI
matching. **`sui-id-web` has three tests in the entire crate**, all in
`tokens/tests.rs`, all about colour contrast.

**Step 1's review said "accepted, no required changes", and that was right for
step 1** — defaults and marker. The test was never dispatched. This is that.
It is the only thing between RFC 136 and closure.

## Scope — one test file

### Where

`crates/sui-id-web/src/pages/clients/tests.rs`, declared from
`pages/clients.rs` as:

```rust
#[cfg(test)]
mod tests;
```

**Plain, no `#[path]`** — `crates/sui-id-web/src/pages.rs:17` declares
`pub mod clients;` plainly, so RFC 137's rule resolves to the plain form here.

### What to assert

**This is cheaper than it looks.** `render_clients` and `render_client_edit`
both return `String`. No SSR harness is needed: call them and assert on the
output.

| Case | Expect |
|---|---|
| `render_clients` with a `ClientSummary` whose `registered_via == "dynamic"` | output **contains** the self-registered marker |
| `render_clients` with `"admin"` | output **does not** contain it |
| `render_client_edit` with `ClientEditData.registered_via == "dynamic"` | **contains** |
| `render_client_edit` with `"admin"` | **does not** |

**Assert against `lang.strings().status_self_registered`, not a literal
string.** The marker's text is an i18n value (`components/badges.rs:118`), and
a test hard-coding the English would fail on a translation change — which is a
false alarm, not a defect.

### Both sites, deliberately

The distinction is rendered **twice** — `pages/clients.rs:37` in the list row
and `:280` in the edit view — and the expression
`(registered_via == "dynamic")` is written out at both. **That duplication is a
latent bug**: change one and the other silently disagrees. Testing both sites
is what makes it safe, which is why the table has four rows and not two.

**Do not refactor the duplication away in this step.** RFC 136's scope is
legibility, the prerequisite asks for a test, and a test covering both sites
discharges it. If you think the extraction is worth doing, say so in the
package and it becomes its own decision.

## Gates

`G01`–`G08`, `G21`. **Not G19 or G20** — nothing here touches federation egress
or the store's rollback registries. Clean tree, clone on `/home`.

## Package

Per-hunk SHA-256 against the tip when you start, a full-content hash for the
new file, gate results, entry point. **State the test count for
`crates/sui-id-web` before and after** — it is 3 today, and a crate that small
makes the delta worth reading.

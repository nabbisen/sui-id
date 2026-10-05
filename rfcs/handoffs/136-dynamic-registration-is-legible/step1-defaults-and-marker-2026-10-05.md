# RFC 136 step 1 — the recorded defaults and the registration marker

**RFC status: Accepted** 2026-10-05, with its open question settled: **a marker
on the exception**, not a sortable column.

**Dispatched.** This is the whole of RFC 136; there is no stage 2.

## D1 — Record the two defaults where the code is

Doc comments only. **No behaviour change, and none is wanted** — if this step
changes what any client gets, something has gone wrong.

| Site | What to state |
|---|---|
| `crates/sui-id-store/src/models.rs:211-219` (`ConsentPolicy`) | `None` is the `#[default]` and its doc already calls it the "first-party default". Say **why that makes it wrong for a self-registered client** |
| `crates/sui-id-core/src/identity/admin/clients.rs:91` | why `ConsentPolicy::default()` is right here: an administrator-created client **is** the first-party case |
| `crates/sui-id/src/http/handlers/dynamic_register.rs:198` | why `FirstTime`: a client that registered itself through a protocol endpoint is by construction not first-party |

**Also record why `Always` was rejected**, at the enum: it asks the user again
after they have already decided, which trains people to click through the screen
that exists to make them stop and read.

**Write these so they survive a reader who disagrees.** The failure this guards
against is someone seeing `FirstTime` next to an enum whose `#[default]` is
`None` and "simplifying" it. A comment that only states the value does not stop
that; one that states the reason does.

## D2 — Mark self-registered clients in the admin UI

**A marker on `registered_via = 'dynamic'` only.** Administrator-created clients
get nothing — the administrator created them, so it is not news. This was
settled at acceptance; do not add a column.

**Where:** the client list (`crates/sui-id-web/src/pages/clients.rs`, the table
beginning at `:136`) and the client detail view.

**The premise is verified, so build on it rather than re-checking:**
`clients::create` does **not** write `registered_via` — its doc comment says so
at `repos/clients.rs:66-68`, and `dynamic_register.rs:199` sets the field on the
row struct where `create` drops it. **C15's transaction stamps it separately**
at `crates/sui-id-store/src/commands.rs:3013,3021`. So the column is correct for
dynamically registered clients and the marker will appear. **If you find
otherwise, stop and report it** — that would mean the security review was wrong
and the RFC's premise with it.

**i18n: three locale files, not four.** `en`, `ja` and `zh_hans` carry 717 keys
each; `zh_hant` is a documented placeholder stub delegating to `zh_hans` and
excluded from `Locale::ALL`. **There is no locale-completeness gate**
(`contracts/gate-inputs.toml` has no locale lane), so a missed key is caught by
review or not at all — which is why this is stated rather than left to be found.

## Not in scope

**The enable-confirmation page.** `@nabbisen` placed it after the federation
work. It depends on this step: a confirmation reading "this client registered
itself" is meaningless while nothing else in the UI ever says so.

## Tests

- The marker **renders** for a client with `registered_via = 'dynamic'`.
- The marker is **absent** for `'admin'`. Without this second test, a marker
  rendered unconditionally would pass the first.
- D1's comments are not testable and are not pretended to be; they are checked
  by review.

## Return

Per-hunk SHA-256 against a stated baseline, the removal evidence, and the full
local gate set — **including A3.2, G19 and G20**.

# RFC 134 step 7 — D3's maintenance path

**RFC status: Accepted** (re-accepted 2026-10-05 **on this re-scope**).

**Dispatched.** This is the only outstanding implementation work in RFC 134;
steps 1–6 have landed. With it, all nine closure criteria are met **and**
satisfied in code.

## The defect, measured

The origin set D3 introduced is **write-once at provider creation**:

1. `crates/sui-id/src/runtime/startup.rs:303-304` is `Ok(_) => {}` — an existing
   provider is left untouched on every boot;
2. `crates/sui-id-store/src/repos/federation_provider.rs` has `create`,
   `set_enabled` and `delete` — **no `update`**;
3. so the only way to change the set is delete-and-recreate, and
   `crates/sui-id-store/src/migrations/0038_federation_link.sql:22` is
   `ON DELETE CASCADE` — **that destroys every federation link to the provider**
   and every federated user loses their account linkage.

**An administrator adding a second origin — the Google case D3's own text cites
— currently has no non-destructive option.**

## What to build

### 7a — Reconcile the origin set at startup

`startup.rs`'s `Ok(_) => {}` arm becomes: if the stored `allowed_origins`
differs from the configured one, **update it**, and log that it changed, with
the slug and both values.

This needs an `update_allowed_origins` in `repos/federation_provider.rs`. **Not
a general `update`** — one column, so a future caller cannot reach the others by
accident.

**Config validation already exists and must not be duplicated.** `Config::load`
rejects a non-`https` entry, an unparseable one, and more than
`MAX_ALLOWED_ORIGINS`, each naming the slug. The reconciliation path runs after
that, on a value already validated.

### 7b — Warn when any *other* config field is being ignored

**This is the part that keeps the fix from reproducing the bug.**

Reconciling only `allowed_origins` means an administrator who edits `scopes` or
`issuer` still sees nothing happen — the same silence, moved one field over. The
documentation makes this worse: `docs/src/reference/configuration.md:230-251`
presents `[[federation_providers]]` as a table of configuration fields with
types and defaults, and **says nothing about any of them being applied only at
creation.**

So: on boot, for every existing provider, compare each config-managed field
against the stored row and **`tracing::warn!` for each that differs**, naming
the slug, the field, and both values, with one sentence saying the stored value
is in force and how to change it.

Fields to compare: `display_name`, `issuer`, `client_id`, `scopes`,
`provision_mode`. **Not `enabled`** — the row's own comment at `startup.rs:304`
says "admin manages enabled state", so warning every boot about a deliberate
runtime state would be noise. **Not the client secret** — it is encrypted at
rest and comparing it means decrypting on every boot to produce a log line.

**Do not silently reconcile these.** Changing `issuer` on a live provider is a
different and larger decision than changing its origin set, and it is not
dispatched.

### 7c — Documentation

`docs/src/reference/configuration.md`'s `[[federation_providers]]` section gains
`allowed_origins`, and **a sentence stating which fields are applied only at
creation**. That sentence is the actual fix for the documentation gap; the
warning in 7b is what makes the behaviour discoverable without reading docs at
all.

## Tests

- A provider whose stored origin set differs from config **has it updated on
  boot**; one that matches is **not written** (assert no write, not just the end
  state — an unconditional update would pass the first test alone).
- The update touches **only** `allowed_origins`: snapshot the whole row before
  and after and assert every other column is unchanged.
- **The federation links survive.** Seed a provider with a link row, change the
  origin set, boot, and assert the link still exists. This is the defect's whole
  point and the test that proves it is fixed.
- A differing `scopes` produces a warning and **does not** change the stored
  value.
- The removal check per control.

## Return

Per-hunk SHA-256 against a stated baseline, the removal evidence, and the full
local gate set — **including A3.2, G19 and G20**.

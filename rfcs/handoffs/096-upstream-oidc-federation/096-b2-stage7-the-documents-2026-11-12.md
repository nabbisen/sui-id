# RFC 096-B2 stage 7 — the documents

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S7 —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-12, by the architect.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
S6b must be in it.
**Why this stage exists.** Cycle L's fourth protection
(`cycle-l-and-0.90.0-2026-11-11.md:132`): *"an upgrade note that leads with
this, before anything else in the release."* **This** is that, upgrading to
0.90.0 stops federated sign-in until each provider is checked and
re-enabled. Each stage so far corrected the sentences it falsified; this
stage writes what an operator reads end to end.

**No code changes**, except text the documents require you to correct. If
you find the product doing something the documents cannot honestly
describe, **stop and report it**. Do not word around it.

## A — the upgrade note: `docs/src/guides/upgrade.md`

A new `### v0.90.x`, first under *"Version-specific notes"*. It **leads**
with the consequence, then gives the steps, in this order:
1. **What happens.** Federated sign-in through every provider stops on
   upgrade. Users keep their accounts and links. Password and passkey
   sign-in are unaffected.
2. **Why**, in two sentences: older versions could not prove a provider was
   checked against its upstream (RFC 096 `:1016-1029`).
3. **Before upgrading:** check the configuration against the stricter rules
   startup now enforces (§D's list). A configuration that fails them stops
   sui-id from starting, and the message names the provider and the field.
4. **After upgrading:** for each provider, open
   **Admin → Federation providers**, then **Check and enable**. If it is
   refused, read what the message says to do.
5. **What else changed for an operator**, briefly, each with a pointer:
   - a trust-field edit in the config file now applies on restart and
     disables the provider until it is re-checked;
   - removing a block disables the provider;
   - the login page shows a "Sign in with …" button for each enabled
     provider;
   - delete and unlink exist.

**Each step is written as something to do, not something that happened.**
Use the real page name, button labels and log text. Copy them from the
running product, not from earlier stage documents.

## B — the operator guide: `docs/src/guides/operators.md`, *"Federated sign-in"*

The section (`:1684`) predates every 096-B2 command. Rewrite it so an
operator can run a provider's whole life from it, in this order:
1. **Configure** a block: the fields, and the rules startup enforces
   (§D's list), linking to the configuration reference rather than
   repeating it.
2. **Check and enable**: what the check does, its 10-minute window, and
   "Check only".
3. **What a refusal means**: every refusal, with its next step, as the UI
   shows it.
4. **Change a provider.** An edit to a trust field disables the provider
   until it is re-checked, and the startup log line names the changed
   fields. `display_name` does not disable. Reordering a list is not a
   change.
5. **Disable**, **remove from configuration** (C25), and **delete** (C18):
   when each is the right one, and that delete needs the block removed
   first.
6. **Unlink a user** (C20): what it ends, what it does not (open sessions,
   issued tokens), how to end those, and that it cannot be undone in this
   release.
7. **The audit events** these write, linking to the audit-event reference.

Keep the existing `provision_mode` guidance. Check it is still true.

## C — the reference pages, re-read for truth

Re-read each page end to end. **Correct anything false, and say what you
corrected:**
- `docs/src/reference/configuration.md`'s `[[federation_providers]]`
  section;
- `docs/src/reference/audit-events.md`'s `federation.*` rows and *Retired
  events*;
- `docs/src/reference/oidc-api.md`'s federation paragraph;
- `docs/src/getting-started/overview.md:79`.

## D — the CHANGELOG, read as release notes

`[Unreleased]` grew one stage at a time. Read it as an operator would on
release day, and **reorder it, without dropping any fact**:
- **The first bullet is the upgrade consequence**, with a link to §A's note.
- **The newly enforced configuration rules form one list**, which §A and §B
  both point to:
  - issuer, including the host rule;
  - client id;
  - scopes;
  - `allowed_origins`;
  - `provision_mode`;
  - `slug`;
  - `id_token_algs`.
- **Bullets that later stages made false or redundant** are merged into what
  is now true, not left beside it. Earlier stages each corrected one, and
  this pass checks the rest.

The release preparation that renames `[Unreleased]` is later, and not this
stage.

## E — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed` (no Rust scope expected), and
  quote the scope line and any failing check names. G10a builds the real book
  (`mdbook build docs`) and must pass.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage7-the-documents-2026-11-12.md`,
with:
- the baseline;
- §A's note, in full;
- §B's section outline, and every refusal as written there;
- §C, each correction with the false sentence and the true one;
- §D's new order, and each merge, naming the bullets merged;
- **anything the product does that the documents could not honestly
  describe.**

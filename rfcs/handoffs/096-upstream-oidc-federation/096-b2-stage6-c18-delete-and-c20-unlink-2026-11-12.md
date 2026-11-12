# RFC 096-B2 stage 6 — C18 delete, and C20 unlink

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S6 —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-12, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, §096-B2. That covers amendment A1, under which C19 is
withdrawn and only C20's unlink remains, and **A3 as narrowed 2026-11-12**
(`:167-170`). Also RFC 096 `:398-401`, on ids never reused, and `:452-454`,
C18's transaction. RFC 094: `rfcs/done/094-transactional-audit-registry.md:532-534, :543`.
Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S5b
must be in it.
**Read first:** RFC 096 `:167-170` in full. And S4's dispatch §C, for the
seam pattern this stage repeats.

## A — C18, delete a provider

**Preconditions.** Each one is a refusal with a next step (§C):
1. **An administrator, through the admin UI only** (A3), behind a fresh
   step-up.
2. **The provider is disabled** (A3).
3. **No `[[federation_providers]]` block names it** (A3 as narrowed). The
   store cannot see the configuration, so this check lives in the one
   `sui-id` function that calls the store command. **The source-scope
   pattern from S4 pins that**: across `crates/*/src`, the store command's
   name may appear only in its definition and in that file. Run its mutation.

**The transaction** (RFC 096 `:452-454`: *"checked-increments activation
generation before marking/removing the row and invalidates the same evidence
and flows"*):
- re-read the row, and refuse if it is missing or enabled;
- checked-increment `activation_generation`;
- invalidate flows through S2b's `invalidate_provider_flows_within_tx`;
- **count the links**, then delete the row. The links cascade
  (`0038_federation_link.sql:22`), and the MFA continuations and ceremonies
  cascade from them;
- `federation.provider.deleted`, with the old and new generation, **the
  number of links deleted** (A3: *"the event records that count"*), and the
  invalidated counts. The administrator is the actor, with step-up evidence
  as in K01. No issuer, client id or slug value goes in it, since the row
  itself is gone.

**The page.** On the federation-providers list, a provider that is
disabled **and** has no configuration block offers **Delete**, which
nothing else does. Its confirm page, behind step-up, states:
- the provider's display name;
- **how many users' federated sign-in ends**, in words: *"N users will no
  longer be able to sign in through this provider; their sui-id accounts
  remain"*;
- that it cannot be undone.

**Tests.**
- each precondition's refusal;
- the counts, with three links seeded;
- links, continuations and ceremonies gone, and users kept;
- rollback through the failure injector;
- the page, end to end, with its audit row asserted;
- **a configured provider's row offers no Delete, and its POST is refused.**

## B — C20, unlink one user from one provider

- **Where:** the user's admin page (`/admin/users/{id}`, `users_detail_get`)
  gains a *Federated sign-in* section, listing that user's links
  (`federation_link::list_for_user`): the provider's display name, when the
  link was made, and when it was last used. **Each link has Unlink.**
- **The confirm page**, behind step-up, says what happens: *"{user} will no
  longer be able to sign in through {provider}; their account and their
  other sign-in methods remain."* It also says **plainly that sessions
  already open stay open**, and links to that user's sessions to end them.
  Unlinking does not end sessions, because sessions do not record how they
  were opened. The page must not let an administrator assume otherwise.
- **The transaction:** delete the link, so the pair's MFA continuations
  cascade (`0047:152-154`), then `federation.link.deleted`. The target is
  the user, and the note carries the provider's id and the number of
  continuations removed. Administrator actor and step-up evidence.
- **The same seam rule** as C18: one `sui-id` caller, pinned by the scope
  test.
- **Tests.**
  - the unlink, and the user kept;
  - a sign-in through that provider afterwards takes the no-link path
    (`link_only` refuses with the link-required message; `provision_on_first_login`
    with an existing local user is the takeover-blocked case);
  - rollback;
  - the page end to end, with the audit row;
  - an auditor sees the section read-only and gets 403 on the rest.

## C — refusals, in both languages

Every refusal of both commands gets a reason **and** a next step, in English
and Japanese, through one exhaustive `match` per error type, as in S5. In
particular:
- *"this provider is still enabled; disable it first"*;
- *"a [[federation_providers]] block still names this provider; remove it
  and restart sui-id first, or it would be created again on restart"*.

## D — records

- **Contract:** C18's and C20's rows go from `target-absent` to `implemented`,
  and the registry, `write-authority.toml` and `audit-coverage-matrix.md`
  follow. G20's rollback coverage and G23 stay green.
- **Docs:** add both events to `audit-events.md`.
- **`CHANGELOG.md`**, under `### Added`: deleting a provider and unlinking a
  user, in operator words, including that unlinking does not end open
  sessions.
- Any `docs/src` sentence that says deleting a provider is done *"directly"*
  or *"by editing the database"* is now false. `grep -rn` for both.

## E — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`, the complete matrix, and
  quote the scope line and any failing check names.
- **Don't edit files while the matrix reads them.**
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage6-c18-delete-and-c20-unlink-2026-11-12.md`,
with:
- the baseline;
- every precondition with its refusal text and test;
- both transactions step by step, each with its `file:line`;
- the counts test;
- both scope tests and their mutations;
- both pages, element by element;
- the two end-to-end tests, with their audit rows.

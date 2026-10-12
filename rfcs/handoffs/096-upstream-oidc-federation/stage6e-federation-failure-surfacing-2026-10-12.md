# RFC 096-B1 stage 6e — federation failure surfacing, and `link_only` said plainly

**Dispatched.** 2026-10-12 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
amended and authorized 2026-10-12. This stage was number 13 and moved to the
front.
**Prior.** Stages 0–5, 6a, 6b, 6c complete. 6c landed `87a52aa`.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.
**This is `0.80.0`'s last precondition.** The release is cut when this lands.

## Why this is first now, and not stage 13

It was scheduled after the legacy-path removal, as a tidy. Reviewing the open
decisions against the project's own philosophy — *"APIs and UI/UX **for users
not to be confused or misunderstand** are also very important"* — moved it, and
the reasoning is in
[`five-open-decisions-reviewed-2026-10-12.md`](five-open-decisions-reviewed-2026-10-12.md),
items 3 and 4.

**What a user meets today.** `link_only` is the **default** `provision_mode`.
An upstream identity with no existing local link is refused with
`/admin/login?fed_error=link_required` — and `LoginGetQuery`
(`handlers/admin/auth.rs:37-40`) has one field, `next`. **No handler and no
template reads `fed_error`.** So the user is redirected to a login page that
says nothing at all, with an unexplained query parameter in the address bar,
and there is no self-service linking anywhere in the product to send them to.
Every federation failure behaves this way, not only this one.

## Scope

### 1. A bounded map onto user-visible messages

**The design constraint comes first, because getting it wrong is worse than
doing nothing: do not render `fed_error`.** Rendering the internal value would
publish distinctions the product spent stage 5 removing — `email_collision`
versus `link_required` is exactly the pair whose observable difference was a
local-account enumeration oracle. The surface is a **map**, from the internal
family onto a small set of messages, and the map is the security-relevant part
of this stage.

- **Take the census yourself**, do not trust mine: every distinct `fed_error`
  value the handlers can produce, including the ones
  `claim_and_nonce_fed_error` and stage 6c's claim refusals generate
  dynamically. Report the list.
- **Collapse every collision-adjacent value onto one message.** Anything whose
  distinctness would tell a caller something about local accounts — at least
  `email_collision` and `link_required`, and justify any others you fold in or
  leave out.
- **The message set should be small.** A user needs to know whether to try
  again, wait, or contact an administrator. Three or four messages probably
  carry all twenty-odd values; if you reach more than that, say why.
- **Nothing upstream-controlled reaches the page.** `:714`'s own rule —
  no raw email, `sub`, ID token or upstream error text — applies to rendered
  output at least as strongly as to logs.

### 2. Where the message comes from, and i18n

The login page is rendered by `login_get` through `render_login`, with
`RequestLocale` already threaded. **Follow the project's existing i18n
convention rather than inventing one** — find how other user-facing error
copy on that page is keyed and localized, and match it. Say which convention
you matched. An English-only string added beside a localized surface is the
kind of inconsistency this stage exists to remove, not add.

### 3. The query parameter itself

`fed_error` is caller-supplied as far as `login_get` is concerned — anyone can
visit `/admin/login?fed_error=anything`. So:

- **An unknown value gets the generic message**, never an echo, never a blank
  page that differs from the known-value case.
- **The parameter is never reflected into the page**, in any form, including
  attributes and hidden fields. Say how you ensured that and what test proves
  it — a reflected-value test with a payload shape is the right instrument.

### 4. `link_only` said plainly, in the operator guide

`docs/src/guides/operators.md` and `docs/src/reference/configuration.md` were
corrected in stage 5 to stop promising a self-service link flow. They now say
an administrator creates the link record directly. **That is accurate and
still not enough for a release**: an operator choosing between the two
provision modes needs to know what each costs them.

- Say that under `link_only` **this release has no self-service linking**, so a
  first link exists only by an administrator's direct action or by using
  `provision_on_first_login`.
- Say **which to choose** and why, in a sentence or two — not a
  recommendation dressed as neutrality.
- Keep the RFC's "may" about a later design (`:701-703`); invent no date.

## Not in scope

**No new route, no linking flow.** `:700-701` removed the skeleton and M4 does
not create a link; nothing here restores it.

**No change to which `fed_error` value any handler produces.** This stage reads
them and maps them; it does not renumber them. If the census finds a value
whose *existence* is wrong, report it rather than changing it.

**F04, the MFA continuation, the legacy removal.** Stages 7–12, after the
release.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The census**, every distinct value with where it is produced.
2. **The map**, value by value, with the collapsing justified — especially
   which values share a message to avoid being an oracle, and why the ones you
   kept distinct are safe to keep distinct.
3. **The rendered surface**, with the i18n convention you matched named.
4. **The unknown-value and no-reflection tests**, including a payload-shaped
   value.
5. **The two documentation passages**, with the mode recommendation stated.
6. **Mutation evidence**: collapse two values that must stay distinct and show
   what fails; make one collision-adjacent pair distinguishable again and show
   what fails. The second is the security-relevant one — if nothing fails, the
   map is not tested.
7. **Per-hunk SHA-256** against the tip you named, full-content hashes for new
   files.
8. **Gate evidence**: the full matrix from a throwaway clone with its own
   `target/`, `cargo test --workspace` on both pinned toolchains, and **G10a/
   G10b/G15 owed** since `docs/` moves. **G06 matters more than usual** — a
   template or i18n change is where `--all-features` diverges.
9. **The count reconciliation** from `sui-id-store` **393**, `sui-id` lib
   **556**, e2e **521**, `scripts/tests` **349**.
10. **Anything you think is wrong with this dispatch.** The one claim of mine
    to check first is that nothing reads `fed_error` today —
    `handlers/admin/auth.rs:37-40` is where I looked. The census is
    deliberately yours rather than mine: my own count of this family has been
    wrong once already, and a number I cannot reproduce exactly does not belong
    in a dispatch.

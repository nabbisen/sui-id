# M3 stage 2 — the field rules

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. RFC 095 is **Accepted** and its entry gate has
> been open since 2026-10-05, six of six conditions met.

**Dispatched.** 2026-10-20 JST, by the architect.
**Plan.** [`m3-stage-plan-2026-10-20.md`](m3-stage-plan-2026-10-20.md) §C.
**Normative source.** [`metadata-validation.md`](metadata-validation.md),
§*Supported fields* and §*Name and scope corpus*. **Where this dispatch and
the matrix differ, the matrix wins** — and tell me, because then one of them
is wrong.
**Baseline.** Read the tip with `git log -1`.

## 1 — where the work goes, measured

`crates/sui-id/src/http/dynamic_registration_validation.rs`, 564 lines, is
stage 1 and 1b's module and is where these rules belong. It already holds the
envelope bounds, `parse_envelope`, bearer-token extraction, the closed
redirect profile and `reject_http_post_logout_uris`.

**Follow its existing idiom rather than inventing one.** `RedirectRejection`
is the shape: a `#[derive(Debug, PartialEq, Eq)]` enum whose variants carry
the offending value and a `&'static str` reason, with a doc comment on each
variant saying what is safe to surface. Match it.

**Two sections of the matrix are already done** — *Envelope and member policy*
and *Redirect corpus*. Do not re-do them and do not alter them.

## 2 — the rules

From §*Supported fields*. Defaults are in bold because an absent field is not
an error for three of these.

| Field | Rule |
|---|---|
| `client_name` | trimmed, 1–128 scalars, ≤512 bytes |
| `token_endpoint_auth_method` | `none` \| `basic` \| `post`; **default `basic`** |
| `grant_types` | `code`, or `code`+`refresh`; **default `code`** |
| `response_types` | `code` only; **default `code`** |
| `scope` | 1–32 unique catalog tokens, **including `openid`** |
| application URI fields | absent, or one canonical HTTPS URI |

Rejections, from the same table: `client_name` blank, control, bidi, oversize;
`grant_types` empty, duplicate, **refresh without code**, any other grant;
`response_types` empty, duplicate, token, hybrid, other; `scope` empty,
malformed, unknown, duplicate, oversize; URI fields HTTP, userinfo, fragment,
non-canonical, oversize.

**And the coupling, both directions:** `refresh_token` requires
`offline_access`; `offline_access` requires `refresh_token`.

### Three things I measured so you do not have to

- **`response_types` has no implementation at all.** Zero occurrences in
  `crates/sui-id/src/http/handlers/dynamic_register.rs`. It exists in
  `discovery.rs` for the provider's own metadata, which is a different thing —
  do not wire one to the other without saying why.
- **The scope catalog is a table, not a constant**: `scope_definition`
  (migration `0036`), with `crates/sui-id-store/src/repos/scope_definition.rs`
  over it. So "unknown catalog value" is a lookup, not a match against a list
  in this module — which is what makes the corpus's *"concurrent catalog
  deletion/default change"* case meaningful. **Decide where that lookup
  happens** relative to the C15 transaction and say why; a validation function
  that queries the database is a different shape from the pure ones already in
  this module, and I am not dictating which it should be.
- **`post_logout_redirect_uris` is partly done** —
  `reject_http_post_logout_uris` exists. The matrix also requires 1–16,
  unique, canonical HTTPS, **independent of the authorization profile**.
  Check what the existing function actually covers before adding to it.

## 3 — the corpus is the test plan

§*Name and scope corpus* is not background; it enumerates the cases:

- **Name:** leading/trailing whitespace, blank, 128 and 129 scalars, 512 and
  513 bytes, combining characters, emoji, C0 and C1 controls, bidi override
  and isolate, HTML, quotes, log delimiters.
- **Scope:** every allowed RFC 6749 scope-token character, space,
  quote/backslash exclusions, duplicate tokens, unknown catalog values,
  concurrent catalog deletion or default change, 32 and 33 tokens, 64 and
  65-byte token, 1,024 and 1,025-byte total.

**The paired boundaries are the point** — 128/129, 512/513, 32/33, 64/65,
1024/1025. A test that only exercises the accepting side of a boundary proves
nothing about where the boundary is. Both sides, each one.

**Mutation evidence is asked for here, and specifically**: for each of the five
paired boundaries, move the limit by one in the implementation and show the
rejecting-side test failing. Five mutations, five failures. Not a
representative — this engagement has twice shipped a test over a set that
passed while testing nothing, and a boundary table is the same shape.

*Scalars, not bytes, for the 1–128 count, and bytes for the 512 — the matrix
distinguishes them and so should the code. `"é"` is one scalar and two bytes;
an emoji with a modifier is one grapheme and several scalars. Say which
definition you used where.*

## 4 — what to return

The usual package, plus:

- **Where the catalog lookup happens** relative to the C15 transaction, and
  why.
- **The five boundary mutations**, each shown failing.
- **Anything in `metadata-validation.md` you found wrong or unimplementable** —
  reported, not silently reinterpreted. It is a 2026-10-05 document and this
  is the first stage to read most of it closely.
- The full matrix; this touches `crates/`, so Level B scope. Report it as
  local — Level B on a tagged commit is the architect's.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the last commit
  of the round, baselined on the tip at that moment.

**A `CHANGELOG` entry: yes.** Unlike the last two rounds, this changes what the
registration endpoint accepts and rejects, which is operator- and
integrator-visible. Under `### Added` or `### Changed` as you judge; **not**
`### Security` — these are validation rules being completed, not a weakness
being closed, and that distinction has its own gate.

## 5 — what this stage does not do

Not the response surface (stage 3), not the authentication/grant matrix
(stage 4, which gets an independent design review first), not browser
presentation (stage 5), not the legacy-candidate report (stage 6). It does not
close M3.

Leave the working tree uncommitted. One open dispatch at a time; this is it.

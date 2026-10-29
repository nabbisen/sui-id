# RFC 140 — Application identity the operator vouches for

**Status.** Accepted
**Accepted on.** 2026-10-27
**Approved by.** `@nabbisen`, 2026-10-27: *"Accepted."* — on the architect's
review of whether this was over-engineering, which he asked for and whose
measurement is this RFC's body. **What the acceptance authorizes is §4's
stage 1**: by this RFC's own terms §4 leaves stage 2 unscoped until §5 is
settled, so acceptance cannot authorize what the document does not yet
specify. §7's question is open.
**Independent design review.** [Design review 2026-10-27](../handoffs/140-application-identity-the-operator-vouches-for/design-review-2026-10-27.md),
by **the architect, which also wrote this RFC** — so it is **not**
independent of it, and the field says which. `ROADMAP.md` §S1c prohibits
architect self-review by default and permits it where `@nabbisen` asks;
he asked, 2026-10-27: *"Review it with my fundamental philosophy."* The
review records that. §S1a places design review with him or an external
architect he engages, not with the implementation role; no external
architect was engaged.
**Implementation owner.** The mid-capability model (implementation and
testing agent).
**Security review.** Required — the consent screen is where an end user
makes a trust decision, and this RFC changes what that screen may show.

**Design prerequisites.** None.
**Implementation prerequisites.** None for stage 1. **Stage 2 may not be
built** until §5 is settled — not because of RFC 000's status rule, which
this RFC now satisfies, but because §4 does not specify it.
**Closure prerequisites.** The three link fields are settable by an
administrator, render on the consent screen for administrator-registered
clients and for no others, and a test asserts both halves; the logo
decision in §5 is settled and either implemented or recorded as declined.
**Tracks.** Arises from RFC 094 M2b wave 4, which found the C10 inventory
row dormant. Not part of M2b.
**Touches.** `crates/sui-id-web/src/pages/oidc.rs`,
`crates/sui-id/src/http/handlers/admin/clients.rs`,
`crates/sui-id-core/src/identity/admin/clients.rs`,
`crates/sui-id-store/src/{commands.rs,repos/clients.rs}`,
`crates/sui-id-i18n`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** The architect (high-capability model).

## Summary

sui-id stores four application-identity fields on every client —
`logo_uri`, `homepage_uri` (RFC 7591's `client_uri`), `policy_uri` and
`tos_uri`, added by migration `0035` for RFC 008. **The feature is dead
in both directions.** Only dynamic registration can set them; the
administrator path hardcodes them to `None`. And the consent screen
renders the logo only for administrator-registered clients, because RFC
095 M3 stage 5 gated the registrant-controlled one. So the branch that
may render can never hold a value, and the branch that holds values
never renders.

This RFC builds the missing half: an administrator-side write path, so
the vetted branch has something to show. It does **not** relax the
provenance gate — the point of showing application identity on a consent
screen is that an operator vouched for it.

## Background

The gate is right and the write path was never built. RFC 008 specified
the fields; RFC 095 M3 added the metadata-validation matrix that
dynamic registration uses; stage 5 added the provenance gate. Nobody
added the admin form, and `repos::clients::update_app_identity` has sat
with no caller — which is how RFC 094's wave-4 census found it.

## Design

### 1 — three link fields, settable by an administrator

`homepage_uri`, `policy_uri` and `tos_uri` join the existing admin
client-edit form. Validation **reuses
`http::dynamic_registration_validation::validate_application_uri`**,
already written and already tested: `https` only, well-formed, length
bounded. One function, one rule, both callers — an administrator-typed
URL is held to exactly the standard a registrant-supplied one is.

The write goes through the **C10** sealed command
(`contracts/write-commands.toml`), emitting
`client.app_identity_changed` in the same transaction, per RFC 094's
Class-A seam. C10 is already declared in the inventory and emitted by
nothing; this gives it its caller.

### 2 — the consent screen renders them by provenance

The existing gate pattern in `sui-id-web/src/pages/oidc.rs` extends to
all of them: an administrator-registered client's values render; a
dynamically registered client's do not. The gate stays **in the
renderer**, not at the caller, for the reason the current comment gives
— so the next caller of `render_consent` cannot silently re-break it.

Two of the three are new to the screen: policy and terms links belong
beside the scope list, where a user deciding whether to grant access can
reach them. `homepage_uri` already renders as the client name's link.

### 3 — what does not change

- **Dynamic registration keeps accepting and storing all four.** They
  remain available for operator and protocol inspection, as RFC 095
  `:269-270` intends; they simply are not rendered.
- **No CSP change.** See §5.
- **No change to `validate_application_uri`.**

> **Stage 1 LANDED 2026-10-28**, `d5f033e`, Level B 27/0. An
> administrator can set the three link fields and the consent screen
> renders them for administrator-registered clients only. **Stage 2 —
> the logo — is unbuilt and §5's decision is still open.** Dispatch —
> [`../handoffs/140-application-identity-the-operator-vouches-for/stage1-the-three-link-fields-2026-10-28.md`](../handoffs/140-application-identity-the-operator-vouches-for/stage1-the-three-link-fields-2026-10-28.md).
> **§1's claim that validation "reuses `validate_application_uri` — one
> function, one rule, both callers" was written without reading the
> store.** `clients::update_app_identity` already enforces its own,
> laxer rule (`is_valid_app_uri`: HTTPS, or `http://localhost`, or
> `http://127.0.0.1`). Two rules exist for these four fields. The
> dispatch resolves it — validate administrator input at the HTTP layer
> with the stricter rule, leave the store's backstop alone, and do not
> harmonise by loosening — but §1's wording overstated what was there.

## 4 — stages

**Stage 1 — the three link fields.** Everything above except the logo.
Small: three form inputs, one sealed command with a caller, three i18n
labels, the renderer arms, and tests.

**Stage 2 — the logo, or a decision not to.** §5. Separable, and
materially larger than stage 1.

## 5 — the logo needs a decision, and it is not a form field

**Correcting my own estimate.** When I first costed this for
`@nabbisen` I said four fields on a form with validation already
written. That holds for three of them and **not** for the logo.

`crates/sui-id/src/http/security_headers.rs:70` sets
`img-src 'self' data:`, and an e2e test asserts it
(`r095_m3_stage5_browser_presentation.rs:118`). **An administrator-set
`logo_uri` would not load** — the browser blocks the cross-origin image
regardless of who set it. Building the form field alone delivers
nothing visible.

Three ways out, with my assessment:

- **(a) Relax `img-src` to allow arbitrary origins.** Rejected. It
  weakens a header that currently protects every page, to serve one
  optional decoration, and it would make the consent screen issue an
  outbound request to a third-party origin on every render — leaking
  which user is consenting to which application, to that origin.
- **(b) Fetch and store the image when the administrator sets it**,
  serve it from `'self'`. No CSP change, no render-time outbound
  request. Costs an outbound fetch on an administrator-supplied URL —
  an SSRF surface, which this project already has a policy for in RFC
  134's federation egress rules, and which would have to be reused
  rather than reinvented. Plus storage, a size limit, a content-type
  allowlist, and a serving route.
- **(c) Upload instead of a URL.** The administrator uploads an image;
  it is stored and served from `'self'`. No outbound request at all, so
  no SSRF surface. Costs an upload path, the same storage and limits,
  and a different admin UI control. **My recommendation**, because it
  removes the hardest part rather than policing it.

**Why (c), stated properly — reviewed 2026-10-29.** The SSRF argument
above is correct but it is not the strongest one. **(b) is (c) plus an
outbound fetch**: both need storage, a size limit, a content-type
allowlist and a serving route, and they are otherwise identical, so (b)
is strictly dominated — it delivers nothing (c) does not.

The decisive argument is about meaning. A fetched image is a cached copy
of someone else's resource, and (b) has no good answer to *what happens
later*. **If it re-fetches**, a URL that was a logo when the
administrator approved it can become something else afterwards — the
operator vouched for what they saw, and the consent screen then shows
something they never saw, which is this RFC's own premise failing
silently. **If it does not re-fetch**, the stored copy quietly diverges
from the URL still shown in the admin form, and the administrator
believes the URL is live when it is decoration. Both readings confuse,
and the choice between them is invisible in the UI. **Upload has no such
ambiguity**: what was uploaded is what is served, and the administrator
can see that from the control itself. §5 lands on the consent screen —
the one place in this system where an end user makes a trust decision —
so *APIs and UI/UX for users not to be confused or misunderstand* is the
governing half here, not the secondary one.

**Honest cost: there is no multipart or upload machinery in the tree
today** (the only `multipart` hit is `mail-builder` emitting
`multipart/alternative`). (c) introduces a new capability. Three
constraints for stage 2, should `@nabbisen` settle on (c):

- **Store the image in the database, not on the filesystem**, so the
  existing encrypted backup/restore path covers it unchanged. A
  filesystem blob falls silently outside backup — which is how an
  operator restores a working system and finds the logos gone.
- **Allowlist raster formats only — PNG, JPEG, WebP; reject SVG.** An
  SVG is a document that can carry script. It does not execute through
  `<img>`, but the stored file is also reachable directly from the
  serving route, and the margin is not worth one optional decoration.
- **The upload endpoint is admin-only and authenticated**, stated as a
  requirement rather than inherited by accident.

**No CSP change under (b) or (c)** — `img-src 'self' data:` stands,
which was the point of rejecting (a).

**Stage 2 is not scoped until this is settled**, and it is reasonable to
take stage 1 and stop.

## 6 — alternatives rejected

- **Delete the fields and the display code.** Honest, and cheaper than
  this RFC. Rejected because an operator-vouched application identity is
  a real improvement to a consent screen, and the schema, the validation
  and the gate are already paid for. Recorded as a live alternative: if
  this RFC is declined, deleting the dead display code is preferable to
  leaving it.
- **Render dynamic clients' values with a "supplied by the application"
  warning.** Rejected. A consent screen is the wrong place to ask an end
  user to weigh provenance, and RFC 095 `:269-270` already settled the
  logo half against it.
- **Let `validate_application_uri` diverge for administrators** — for
  example allowing `http` on a loopback host for development. Rejected
  as an unnecessary second rule on a security-relevant surface.

## 7 — open question for the approver

**Stage 1 only, or stage 1 and a stage 2 shape from §5?** Stage 1 stands
alone and delivers the policy and terms links from a vetted source. The
logo is the part that needs the decision.

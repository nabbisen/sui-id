# RFC 022 — Single-realm scope statement

**Status.** Implemented
**Priority.** Medium. The defect this RFC fixes is documentary,
not technical: sui-id's scope (single realm, single
organisation, first-party clients) is well understood internally
but is not stated in the user-facing documentation. The result
is intermittent recurring questions ("when will multi-tenancy
land?") and risk that an operator deploys sui-id assuming a
B2B fit and then hits the boundaries the design did not promise
to cover.
**Tracks.** v0.29.5 data-model review-2 §6 (the single-realm /
multi-realm path-A vs path-B framing) and §7 (recommendation
to "明確に宣言する"). Sister RFC 025 carries the case-B detailed
design.
**Touches.** `README.md` (a new "Scope" section), `docs/
operators.md` (the same statement, more operator-facing),
`docs/threat-model.md` (a one-line cross-reference), and
`rfcs/README.md` (a small index update). No code change. No
schema change.

## Summary

sui-id today is a single-realm, first-party self-hosted IdP.
It carries one flat namespace of users, one flat namespace of
clients, one global admin role, and one set of singleton
configuration rows (server_settings, smtp_config). This is by
design, consistent with the "small, single-binary, quiet"
philosophy of the project.

The choice is also visible in negative space: there is no
`tenant_id` column anywhere, no organisation table, no group
table, no per-tenant audit scoping. sui-id has *not*
half-implemented multi-tenancy — it has chosen not to.

This RFC writes that down. Operators evaluating sui-id need
to know whether it fits their use case before they install it,
and downstream RFCs that touch policy, claims, scope-grants,
or admin authorisation need a fixed referent for "what scope
are we operating in".

## Why now

Three pressures coincide:

- **The data-model review made the lack of declaration
  explicit.** The reviewer noted that the absence of
  multi-tenancy is a deliberate design choice but "現時点では
  明文化していない", and recommended writing it down. Without a
  written scope, the codebase reviewer must derive scope from
  the absence of features, which is fragile.
- **RFC 020 will return `email` in userinfo.** Returning
  identity claims is a B2B-coded affordance in many products,
  and a confused operator could read it as a signal that
  multi-tenancy is "around the corner". The scope statement
  forestalls that misreading.
- **RFC 025 (case B) is taking shape in parallel.** Pointing
  at a written expansion path makes the *current* scope
  declaration credible: the boundary is principled, not
  accidental.

## Requirements

After this RFC ships:

1. `README.md` carries a top-level "Scope" section
   immediately after Overview, containing the declaration
   below.
2. `docs/operators.md` carries the same declaration, framed
   for operators (what does and doesn't work; how to think
   about isolation).
3. `docs/threat-model.md`'s "out of scope" subsection
   explicitly references "single-realm operation; multi-tenant
   threat surface is documented in RFC 025".
4. `rfcs/README.md`'s archive-or-superseded list updates
   when RFC 007 (Multi-tenancy) is moved to `archive/` as
   superseded by RFC 025.
5. No code or schema change.

## Design

### § 1. The declaration (README.md)

The "Scope" section is short, so we can write it out in full
here:

```markdown
## Scope

sui-id is a single-realm, first-party, self-hosted OpenID Connect
provider. One running instance has:

- one flat namespace of users
- one flat namespace of OIDC clients
- one global admin role
- one set of server-wide settings (SMTP, session policy, HIBP
  mode, etc.)

It is built for the case where one organisation runs OIDC for
its own applications. For tenant or organisation isolation, run
one sui-id instance per tenant.

sui-id intentionally does not provide:

- multi-tenancy (a single instance serving multiple
  organisations with isolated data)
- organisation / group / role hierarchies
- arbitrary user attributes or custom claim mapping
- third-party application marketplace flows
- LDAP or SAML federation

If you need any of those, sui-id is not the right tool today.
The expansion path for multi-tenancy is documented in
[RFC 025](./rfcs/proposed/025-multi-tenant-expansion.md) — that
RFC is exploratory; it has no scheduled delivery.
```

The wording deliberately echoes the "explicitly not on the
roadmap" list in `ROADMAP.md`, but is more emphatic in tone
because the README is the front door.

### § 2. Operator-facing version (docs/operators.md)

Same declaration, with one additional paragraph framing the
isolation strategy:

```markdown
## Scope and isolation strategy

sui-id is single-realm. One running instance manages one set
of users, one set of clients, one admin role, and one
configuration. There is no `tenant_id`-style separation
inside the data model.

If you need to isolate tenants, the supported pattern is:
**one instance, one tenant**. Each instance has its own
SQLite file, its own master key, its own master-key-protected
data. Two tenants run as two `sui-id` processes (typically on
two ports or behind two reverse-proxy hostnames).

This pattern is friendlier to backup, restore, and key
rotation: each tenant's data is a single file plus a key.
The trade-off is that operators run N instances instead of
1; for the deployment scale sui-id targets (tens to low
hundreds of users per realm), this is generally acceptable.

If you need a single instance that handles multiple realms
internally, see [RFC 025](../rfcs/proposed/025-multi-tenant-
expansion.md). That RFC describes the schema and routing
changes a future major version would carry, with no schedule.
```

The "one instance, one tenant" pattern is already the
recommended deployment shape for operators who need
isolation; this writes it down.

### § 3. Threat-model cross-reference

`docs/threat-model.md` currently has an "Out of scope" or
"Non-goals" subsection (the codebase review verified its
existence). Append a short bullet:

```markdown
- **Multi-tenant attack surface.** sui-id is single-realm; a
  single instance models one tenant. Threats specific to
  multi-tenant deployments — cross-tenant data access,
  tenant-to-tenant escalation, shared admin compromise — are
  not modelled here. The expansion path that would introduce
  such surface is [RFC 025](../rfcs/proposed/025-multi-tenant-
  expansion.md), which carries its own threat-model section.
```

### § 4. RFC index updates

When this RFC and RFC 025 both land, RFC 007 (Multi-tenancy)
moves to `archive/` with status `Superseded by RFC 025`.
`rfcs/README.md` reflects this:

- The Proposed table loses the row for 007.
- The Archive table gains a row for 007 with the
  supersession note.
- The implementation-order paragraph drops the mention of 007.

Mechanical changes; no design.

### § 5. Exclusions from this RFC

This RFC does **not**:

- Make any code change.
- Forbid future work on multi-tenancy. RFC 025 captures the
  expansion path. The scope statement says "intentionally
  does not provide today", not "will never provide".
- Address LDAP, SAML, or federation. Those are separate
  non-goals (RFC 005 LDAP, RFC 004 federation) tracked
  independently. The scope statement mentions them in the
  "intentionally does not provide" list but does not
  supersede those RFCs.

## Tests

Not applicable; documentation-only. The verification is that
the README, operators.md, and threat-model.md carry the new
text after the change lands.

A small follow-up: the `docs/` mdbook build (when introduced
per the overall ROADMAP) should include the Scope section as a
top-level chapter, not buried under a sub-heading. Tracked as
future work, not this RFC.

## Security considerations

The scope statement clarifies what the threat model covers
and what it doesn't, which is itself a security improvement:
operators can plan around sui-id's actual properties rather
than inferred ones.

A common foot-gun is closed by the operator-facing section:
"running one instance with two tenants by clever schema use"
is not a supported pattern, and the docs say so. An operator
who tries it (renaming users to `tenant1-alice`,
`tenant2-alice`, partitioning by username convention) gets no
DB-level isolation, no admin-level isolation, and no cookie-
namespace isolation. The docs steer them toward the
"one instance, one tenant" pattern that does work.

## Open questions

1. **Should this RFC be marked Implemented immediately on
   landing?** It is doc-only, and shipping the doc is
   shipping the RFC. Treat it the same as any other
   implementation-on-merge RFC: lands in `proposed/`, moves
   to `done/` in the release notes for the version that
   carries the README change.
2. **Phrasing tone.** The current draft uses "intentionally
   does not provide", which is firm but not unfriendly. If
   the maintainer prefers a softer phrasing ("currently does
   not provide; see RFC 025 for the expansion path"), the
   wording substitution is one line. Either is fine.
3. **Should the LICENSE / NOTICE / authors be re-affirmed
   here?** No. Those are unrelated; the scope statement is
   about feature scope, not legal scope.

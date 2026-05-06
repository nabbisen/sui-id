# sui-id RFCs

Design notes for upcoming features. Each file scopes one ROADMAP
item: enough detail for the implementer to start without a second
design pass, but no more than that.

These are not blanket commitments — the [ROADMAP](../ROADMAP.md)
sets which of these will actually ship and in what order. An RFC
landing here means the design is settled enough to write code from;
not landing here means the design is still soft.

## Status

| ID  | Title                                                          | Status     | Priority   |
|-----|----------------------------------------------------------------|------------|------------|
| 010 | [Revoke sessions on forgot-password](./010-forgot-password-revoke.md) | Proposed   | **Highest** — security-critical bug fix |
| 011 | [Enforce WebAuthn transport at the server](./011-webauthn-transport-enforcement.md) | Proposed   | **High** — spec compliance gap |
| 012 | [Setup wizard scope reconciliation](./012-setup-wizard-reconciliation.md) | Proposed   | **High** — spec ↔ implementation decision |
| 016 | [Server logging completeness](./016-server-logging-completeness.md) | Proposed   | **High** — debugging blocker |
| 003 | [HIBP scope expansion (post-v0.24.0)](./003-hibp-expansion.md)  | Proposed   | **High** — priority elevated by v0.29.3 review |
| 013 | [Reduce SQLite blocking on async handlers](./013-db-blocking-mitigation.md) | Exploratory | Medium — performance ceiling |
| 014 | [Hot-path caches and benchmark harness](./014-hot-path-caches-and-benchmarks.md) | Exploratory | Medium — performance |
| 015 | [Documentation consistency pass](./015-doc-consistency-pass.md) | Proposed   | Medium — maintainability |
| 001 | [Persistent email outbox + retry worker](./001-email-outbox.md) | Proposed   | Medium — operational |
| 002 | [i18n scope expansion (post-v0.23.0)](./002-i18n-expansion.md)  | Proposed   | Medium — feature breadth |
| 004 | [Federation as upstream OIDC client](./004-federation.md)       | Exploratory | Low — longer-term |
| 005 | [Pluggable user backends (LDAP)](./005-pluggable-user-backends.md) | Exploratory | Low — longer-term |
| 006 | [Prometheus metrics endpoint](./006-metrics.md)                | Exploratory | Low — longer-term |
| 007 | [Multi-tenancy](./007-multi-tenancy.md)                        | Exploratory | Low — longer-term |
| 008 | [Third-party-posture bundle](./008-third-party-posture.md)     | Exploratory | Low — longer-term |
| 009 | [Pluggable SQL backends (PostgreSQL, MariaDB)](./009-sql-backends.md) | Exploratory | Low — longer-term |

**Implementation order.** RFCs are ordered above by intended
work sequence, not by RFC number. The numbering reflects the
order RFCs were written; the priority column reflects the
order an implementer should pick them up.

The first five (010, 011, 012, 016, 003) are the high-priority
backlog from the v0.29.3 codebase review and the maintainer's
follow-up on logging. They should land before any new feature
work. RFC 010 (session revocation on forgot-password) is the
highest priority — it's a small, unambiguous fix to a real
security gap. RFC 016 (server logging completeness) sequences
ahead of RFC 003 because logging makes implementing and
verifying the others materially easier.

The next three (013, 014, 015) are the same review's medium-
priority findings: performance and maintainability work that
strengthens the foundation.

The remaining eight (001, 002, 004–009) are the longer-term
ROADMAP items, sequenced however the maintainer prefers once
the higher-priority work is settled.

**Status legend.** *Proposed* means the design is firm enough to
implement against. *Exploratory* means the shape is sketched but
the details are still open; expect a follow-up RFC pass before
implementation begins.

## Template

The standard shape is light:

```markdown
# RFC NNN — Title

**Status.** Proposed | Exploratory | Accepted | Implemented | Withdrawn
**Tracks.** ROADMAP item this addresses.
**Touches.** crates / modules the work lands in.

## Summary

One paragraph. What changes for the user, why now, why this shape
over the alternatives.

## Background (optional)

Context the implementer needs that isn't on ROADMAP.md. Skip when
the title alone tells you what's going on.

## Design

What the implementer builds. Schemas, function signatures, state
machines, error paths. Treat this as the contract.

## Multiple implementation steps

If the work splits into stages that can ship separately, list them
here with rough scope.

## Tests (when non-trivial)

What the implementer should write to call it done. Concrete
fixture, assertion, and regression cases — not a full test plan.

## Security considerations (when applicable)

What an attacker might try, and what the design does about it.
Skip when the change is purely operational and adds no new
attack surface.

## Open questions

Anything the implementer should bring back before merging.
```

### When to add the heavier sections

The light template handles small, mechanical items. Anything
medium or larger — schema changes, new background workers,
cross-cutting policies, third-party integration shapes — earns
the heavier sections:

- **Requirements** — explicit list of what must be true after the
  change ships, separately from the design that delivers it.
- **Design** (replaces "Design" section title above) — same
  intent, but expected to be thorough rather than sketchy.
- **Test plan** — coverage map: what unit, integration, and
  regression tests get added; what existing tests might need to
  move.
- **Security considerations** — first-class section, not a footnote.

Each RFC declares which sections it carries by the headings it
uses. There's no separate metadata.

## Process

1. Open a draft RFC under `rfcs/NNN-slug.md` with status
   `Proposed` (or `Exploratory` if the shape isn't firm).
2. Iterate in review until the design is settled. Status moves
   to `Accepted` when the maintainer signs off.
3. Implementation proceeds. Status moves to `Implemented` when
   the work merges, ideally referencing the release tag.
4. RFCs that don't pan out move to `Withdrawn` with a sentence
   explaining why; they stay in the directory as a record.

The numbering is allocated when the file is created. Don't
renumber once a file exists, even if its status changes.

# sui-id RFCs

Design notes for sui-id features and policies. Each RFC scopes
one piece of work in enough detail that an implementer can start
without a second design pass — but no more than that.

These are not blanket commitments. The [ROADMAP](../ROADMAP.md)
sets which of these will actually ship and in what order. An RFC
landing here means the design is settled enough to write code
from; not landing here means the design is still soft.

## How this directory works

The lifecycle is governed by
[RFC 018 — RFC lifecycle policy](./done/018-rfc-lifecycle-policy.md).
Briefly:

- **`proposed/`** — open for review and discussion. Implementer
  should not yet start work; the design may change.
- **`done/`** — implemented and shipped. The RFC is now a
  historical record of the design decisions.
- **`archive/`** — withdrawn or superseded. Preserved as
  evidence the design was considered.

Files do not move out of `done/` or `archive/` after they land
there. Numbering is permanent: a file's RFC number is assigned
at creation and never changes, even if the file moves between
folders.

## Index

### Proposed (open for review)

| ID  | Title                                                          | Priority |
|-----|----------------------------------------------------------------|----------|
| 013 | [Reduce SQLite blocking on async handlers](./proposed/013-db-blocking-mitigation.md) | Medium |
| 014 | [Hot-path caches and benchmark harness](./proposed/014-hot-path-caches-and-benchmarks.md) | Medium |
| 017 | [UI/UX design contracts](./proposed/017-ui-ux-design-contracts.md) | Medium — recommended ahead of further admin-UI work |
| 001 | [Persistent email outbox + retry worker](./proposed/001-email-outbox.md) | Medium |
| 002 | [i18n scope expansion (post-v0.23.0)](./proposed/002-i18n-expansion.md) | Medium |
| 004 | [Federation as upstream OIDC client](./proposed/004-federation.md) | Low — longer-term |
| 005 | [Pluggable user backends (LDAP)](./proposed/005-pluggable-user-backends.md) | Low — longer-term |
| 006 | [Prometheus metrics endpoint](./proposed/006-metrics.md) | Low — longer-term |
| 007 | [Multi-tenancy](./proposed/007-multi-tenancy.md) | Low — longer-term |
| 008 | [Third-party-posture bundle](./proposed/008-third-party-posture.md) | Low — longer-term |
| 009 | [Pluggable SQL backends (PostgreSQL, MariaDB)](./proposed/009-sql-backends.md) | Low — longer-term |

### Implemented

| ID  | Title                                                          | Shipped in |
|-----|----------------------------------------------------------------|------------|
| 010 | [Revoke sessions on forgot-password](./done/010-forgot-password-revoke.md) | v0.29.4 |
| 011 | [Enforce WebAuthn transport at the server](./done/011-webauthn-transport-enforcement.md) | v0.29.4 |
| 012 | [Setup wizard scope reconciliation](./done/012-setup-wizard-reconciliation.md) | v0.29.4 (Position C) |
| 015 | [Documentation consistency pass](./done/015-doc-consistency-pass.md) | v0.29.4 |
| 016 | [Server logging completeness](./done/016-server-logging-completeness.md) | v0.29.4 |
| 003 | [HIBP scope expansion (post-v0.24.0)](./done/003-hibp-expansion.md) | v0.29.4 |
| 018 | [RFC lifecycle policy](./done/018-rfc-lifecycle-policy.md) | v0.29.5 |

### Archive

(empty — no withdrawn or superseded RFCs)

## Implementation order

Within `proposed/`, RFCs are listed by intended work sequence,
not by RFC number. The numbering reflects the order RFCs were
written; the order above reflects the priority an implementer
should pick them up.

The current top of the queue is the medium-priority backlog:
**013** (DB blocking), **014** (hot-path caches and benchmarks),
and **017** (UI/UX design contracts) — the last of these is
recommended ahead of any further admin-UI implementation work
(notably RFC 002's admin-domain i18n) so admin screens inherit
the contract rather than re-derive it. The remaining items
(001, 002, 004–009) are the longer-term ROADMAP work, sequenced
however the maintainer prefers once the medium-priority work
is settled.

The high-priority backlog (010, 011, 012, 015, 016, 003) cleared
in v0.29.4. The lifecycle reorganisation itself (this directory's
folder structure plus RFC 018) shipped in v0.29.5.

## Template

The standard shape is light:

```markdown
# RFC NNN — Title

**Status.** Proposed | Implemented (vX.Y.Z) | Withdrawn | Superseded by RFC NNN
**Tracks.** ROADMAP item or other context this addresses.
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

What the implementer should write to call it done.

## Security considerations (when applicable)

What an attacker might try, and what the design does about it.

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

The full lifecycle is described in
[RFC 018](./done/018-rfc-lifecycle-policy.md). The short version:

1. New RFC: open a draft as `rfcs/proposed/NNN-slug.md` with
   Status `Proposed`. The number is the next unused integer,
   zero-padded to three digits, and never reused.
2. Iterate in review until the design is settled.
3. When the work ships, move the file to `rfcs/done/`, update
   Status to `Implemented (vX.Y.Z)`, and update inbound
   references in this README and other RFCs.
4. RFCs that don't pan out move to `rfcs/archive/` with Status
   `Withdrawn` (and a one-line reason) or `Superseded by RFC NNN`.
   They stay there as a record.

Files are never deleted. The full reasoning is in RFC 018.

# RFC 098 handoff — documentation authority and reconciliation

**Governing RFC:** [RFC 098](../../proposed/098-documentation-authority-reconciliation.md) — Proposed
**Milestone:** M5
**Audience:** documentation owner
**Status:** Planning companion; inherits the governing RFC's current status —
Proposed.

## Entry gate

RFC 093 **M1b** Implemented — the mdBook, markdown-link and RFC-integrity gates
M1b owns are the mechanical foundation this RFC builds on. Reconciling claims
before those gates exist would leave the result unenforced and free to drift
again. This RFC Accepted.

## The M1/M5 boundary — this is the thing to get right

M1b already repaired **mechanical** debt: link targets, anchors, path spelling,
case, moved-file references, RFC folder/status/index consistency.

RFC 098 owns **semantic** reconciliation: what the project claims to be, which
document is authoritative when two disagree, and whether public claims match
implemented behaviour.

If a change alters a path, it was M1b's. If it alters a meaning, it is yours.
Where M1b left a link resolving to a document that is *wrong*, that is now your
finding.

## Documents

| Document | Contents |
|---|---|
| [`task-checklist.md`](./task-checklist.md) | Authority map, reconciliation targets, known contradictions |

## Known contradictions carried forward

Measured during the 2026-07-16 review and not yet resolved:

- `README.md:82-84` tells readers LDAP is not offered, while LDAP, federation
  and dynamic registration are shipped features;
- `README.md:181-189` omits the i18n crate from the workspace description;
- `docs/src/contributing/architecture.md:23-38` references moved paths and the
  pre-`Backend` `Arc<Mutex<Connection>>` design;
- the same file at `:75-79` states every mutation uses `events::emit`, which is
  false;
- root operator/integrator documents and the mdBook sources are divergent
  duplicate sets;
- RFC 024 promised consolidation that visibly did not complete — `PUBLISHING.md`
  remains at root and `ROADMAP`/`CHANGELOG` remain large.

## Stop and return to the architect if

- two documents disagree and neither is obviously authoritative;
- resolving a claim would require changing behaviour rather than wording;
- a public claim cannot be made true and cannot be withdrawn without an owner
  decision.

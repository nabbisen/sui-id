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

---

## Measured evidence from M1b C1 (2026-07-31)

Two concrete instances of the divergent-duplicate-sets problem above, found while
building G10b and recorded here with numbers rather than impressions.

### 1. Eight links inside the book point outside it

`docs/threat-model.md` and `docs/integrators.md` are not under `docs/src/` and are
absent from `SUMMARY.md`, so they are not in the mdBook. Eight links inside
`docs/src/` reference them via `../../`.

Verified against the built book, not inferred: `docs/src/guides/operators.md` emits
`href="../../threat-model.html"`, which from `<dest>/guides/` resolves outside the
book output to a file that exists nowhere.

**Neither gate can see this.** G10b validates the filesystem, where the links are
correct; `mdbook build` does not validate link targets, so G10a passes too. The
links are broken only for readers of the rendered book.

Remedy is one of two, and it is a documentation-authority decision, not a link
repair: move both documents into `docs/src/` with `SUMMARY.md` entries, or stop
linking to them from inside the book. Moving them changes published URLs, so it
needs the owner's agreement.

### 2. G10b provides zero coverage of `README.md`

README.md contains 26 links and **all 26 are external**
(`https://github.com/nabbisen/sui-id/blob/main/…`). G10b skips external links by
design, so its coverage of README.md is exactly zero, while RFC 093 names README.md
as a scanned target — implying an assurance that does not exist.

The consistency argument: G10b rejects `/absolute/local/paths` as not portable
across clones and mirrors. Twenty-six hardcoded `github.com/nabbisen/sui-id/blob/main/`
URLs are non-portable in the same way and pass unexamined. Converting them to
relative links would restore both portability and gate coverage in one change.

Full analysis: `.git-exclude/reviewed/m1b-c1-markdown-link-checker-review-2026-07-31.md` §7.

### 3. Two RFCs are both "RFC lifecycle policy", with no supersession recorded

Surfaced 2026-08-01 when M1b C4 added RFC 000 to the index, putting two
identically-titled rows side by side.

| RFC | Title | Status | Folder | Lines | Added |
|---|---|---|---|---:|---|
| 000 | RFC lifecycle policy | Implemented | `done/` | 654 | 2026-07-17 |
| 018 | RFC lifecycle policy | Implemented | `done/` | 563 | 2026-05-06 |

**Neither references the other.** RFC 000 is the newer, so RFC 018 is very likely
superseded — but nothing says so, and both present as live policy.

Under RFC 000's own rules a superseded RFC carries `Status: Superseded by RFC NNN`
and moves to `archive/`. If that reading holds, the corpus violates its own
lifecycle policy in the policy documents themselves.

**Not urgent:** the two agree where it currently matters. Their folder/status
mappings are identical (`done/ ← Implemented`, `archive/ ← Withdrawn or
Superseded`), so RFC 093's G11 rule, which cites RFC 000, is unaffected by which
governs.

**G11 cannot detect this and should not be extended to.** Two distinct identifiers
sharing a title violates no integrity invariant; this is a limitation of the
contract, not a gap in the checker.

Resolving it means reading both in full and deciding whether 018 is superseded,
partially superseded, or independently scoped — a lifecycle decision for the
architect with owner confirmation, not a debt repair.

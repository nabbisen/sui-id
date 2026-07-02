# RFC 024 — Documentation file consolidation

**Status.** Proposed
**Priority.** Low-medium. Internal-improvement only; no security
or functional impact. Triggered by file-size pressure at the
project root that is starting to obscure structure for new
readers.
**Tracks.** Maintainer request: CHANGELOG.md (4,532 lines) and
ROADMAP.md (587 lines) have grown past the size where a flat
top-level file is comfortable; PUBLISHING.md is a maintainer-
only file that does not earn a root slot; the root layout
should become tidier without growing.
**Touches.** `CHANGELOG.md` (becomes a thin index),
`docs/changelog/` (new subdirectory holding per-minor history),
`ROADMAP.md` (compressed to its three-section structure with
RFC pointers), `PUBLISHING.md` (deleted from root, content
moved to `docs/contributors/release-process.md`), `README.md`
(any links to the moved files updated), `rfcs/README.md` (no
change expected, sanity-check). No code change.

## Summary

The root of the repository has grown a set of long-form
documents that work better as references than as flat files:

- **CHANGELOG.md** is 4,532 lines, covering ~42 versions
  end-to-end. Reading it linearly is increasingly the
  exception; readers want either the latest entry or a
  specific historical version. A flat file makes both harder.
- **ROADMAP.md** is 587 lines, much of which restates content
  already settled in landed RFCs. The maintainer's intent for
  ROADMAP — "loose; nothing here is a promise" — is poorly
  served by a long file that reads as commitment.
- **PUBLISHING.md** is a maintainer-only operations document.
  It describes the publish-to-crates.io order, pre-publish
  checklist, yank procedure, and the dual-spec rationale. It
  does not need a root slot; new readers don't need it; only
  the maintainer does, episodically.

This RFC reorganises these three files into a structure that
keeps the root tidy without losing content. The repository
root keeps `CHANGELOG.md` (now compact) and `ROADMAP.md` (now
compact). It loses `PUBLISHING.md`. The full content of all
three lives under `docs/`, in persona-aware subdirectories that
fit the "docs/ for full documentation" convention from the
project instructions.

## Constraints

The maintainer's stated constraints:

1. **The root must not grow.** New top-level files or folders
   are not acceptable.
2. **Content should not be lost.** Historical changelog
   entries, longer roadmap discussions, and the publish
   procedure are still valuable; this RFC reorganises, it does
   not delete.
3. **Reorganisation should follow the persona-based docs
   structure** described in the project's instructions
   (first-time users, intermediate users, maintainers /
   contributors).

## Requirements

After this RFC ships:

1. `CHANGELOG.md` (root) is compressed to:
   - The two most recent minor versions (each shown in full).
   - A pointer to `docs/changelog/` for older history.
   - A short "format" note matching today's wording.
2. `docs/changelog/` exists and contains one file per minor
   version (`0.1.x.md`, `0.5.x.md`, `0.29.x.md`, etc.). Each
   file holds the entries for its minor series.
3. `ROADMAP.md` is compressed to:
   - Three sections only: "Near term", "Longer term, less
     certain", "Explicitly not on the roadmap".
   - Bulleted items in each, with a one-line summary and a
     pointer to the relevant RFC for detail.
   - "Done" history is removed (CHANGELOG carries it).
4. `PUBLISHING.md` is removed from the root.
   `docs/contributors/release-process.md` exists and
   contains the same content, in the same shape.
5. `docs/contributors/` exists as a new subdirectory under
   `docs/`. The maintainer's instructions describe the
   contributors-and-maintainers persona; this is its slot.
6. README.md links to moved files are updated. No dead links.
7. `rfcs/README.md` is reviewed; if it references removed
   files or stale paths, it is updated.

The root tree, before and after:

```
Before:                      After:
  CHANGELOG.md       4532      CHANGELOG.md          ~200
  Cargo.toml                   Cargo.toml
  LICENSE                      LICENSE
  NOTICE                       NOTICE
  PUBLISHING.md        89      (removed)
  README.md                    README.md
  ROADMAP.md          587      ROADMAP.md             ~80
  TERMS_OF_USE.md              TERMS_OF_USE.md
  crates/                      crates/
  docs/                        docs/
   assets/                       assets/
   deployment.md                 changelog/         (new)
   integrators.md                  0.1.x.md
   operators.md                    ...
   threat-model.md                 0.29.x.md
                                 contributors/      (new)
                                   release-process.md
                                 deployment.md
                                 integrators.md
                                 operators.md
                                 threat-model.md
  samples/                    samples/
  rfcs/                        rfcs/
  sui-id.example.toml          sui-id.example.toml
```

The root file count stays the same minus one (PUBLISHING.md).
The root subdirectory count is unchanged. New nesting is
inside `docs/`, which is the right place per the project
instructions.

## Design

### § 1. CHANGELOG split

The new root `CHANGELOG.md` is shaped:

```markdown
# Changelog

All notable changes to sui-id will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

For older releases, see the per-minor archive under
[`docs/changelog/`](docs/changelog/).

## [0.30.0] - YYYY-MM-DD

(latest entry, full text)

## [0.29.5] - 2026-05-06

(previous minor, full text)

## Older versions

| Range            | Archive file                              |
|------------------|-------------------------------------------|
| 0.29.0 – 0.29.4  | [`docs/changelog/0.29.x.md`](docs/changelog/0.29.x.md) |
| 0.28.0 – 0.28.x  | [`docs/changelog/0.28.x.md`](docs/changelog/0.28.x.md) |
| (etc., generated mechanically by the split script)        |
```

#### Per-minor archive format

Each `docs/changelog/0.N.x.md` file:

```markdown
# Changelog — 0.N.x

These are the historical changelog entries for the 0.N
minor series. See [`CHANGELOG.md`](../../CHANGELOG.md) for
the latest releases.

## [0.N.5] - YYYY-MM-DD

(full entry)

## [0.N.4] - YYYY-MM-DD

(full entry)

(...)
```

The split is mechanical: a script walks the existing
`CHANGELOG.md`, looks at each `## [X.Y.Z]` header, groups by
`X.Y` minor, writes per-minor files, leaves the latest two
minors in the root.

The split script is a one-shot. It does not need to be
preserved in `xtask/` — running it again would re-split the
already-split root, which is the wrong direction.

#### How releases are made after the split

The maintainer's release process:

1. Add the new entry at the top of root `CHANGELOG.md`.
2. If the new entry pushes the previously-second-to-latest
   minor's last entry out of the root (i.e. the root now
   carries three minors), move that minor's entries into
   `docs/changelog/0.N.x.md`.

In practice, step 2 happens once per minor. The release
process documentation in `docs/contributors/release-process.md`
(see § 4) describes this rotation in plain terms.

### § 2. ROADMAP compression

The current ROADMAP carries large amounts of "what landed"
text that duplicates CHANGELOG. The compressed shape:

```markdown
# Roadmap

This is a sketch of where sui-id is heading. Items are loose;
nothing here is a promise.

## Near term

(short bullets, each with one-line summary + RFC pointer)

- **Auth flow data integrity hardening** —
  see [RFC 019](./rfcs/proposed/019-auth-flow-data-integrity.md).
- **User identity invariants and OIDC claim consistency** —
  see [RFC 020](./rfcs/proposed/020-user-identity-invariants.md).
- **Schema invariant CHECKs and migration safety** —
  see [RFC 021](./rfcs/proposed/021-schema-invariant-checks.md).
- **Single-realm scope statement** —
  see [RFC 022](./rfcs/proposed/022-single-realm-scope-statement.md).
- **Visual design system** —
  see [RFC 023](./rfcs/proposed/023-visual-design-system.md).
- **DB blocking mitigation** —
  see [RFC 013](./rfcs/proposed/013-db-blocking-mitigation.md).
- **Hot-path caches** —
  see [RFC 014](./rfcs/proposed/014-hot-path-caches-and-benchmarks.md).
- **UI/UX design contracts** —
  see [RFC 017](./rfcs/proposed/017-ui-ux-design-contracts.md).

## Longer term, less certain

- **Multi-tenant expansion path** —
  see [RFC 025](./rfcs/proposed/025-multi-tenant-expansion.md).
  Detailed design is settled; delivery has no schedule.
- **Persistent email outbox** — see [RFC 001](...).
- **i18n scope expansion (admin domain)** — see [RFC 002](...).
- **Federation as upstream OIDC client** — see [RFC 004](...).
- **Pluggable user backends (LDAP)** — see [RFC 005](...).
- **Prometheus metrics endpoint** — see [RFC 006](...).
- **Third-party-posture bundle** — see [RFC 008](...).
- **Pluggable SQL backends (PostgreSQL, MariaDB)** — see [RFC 009](...).
- **Documentation file consolidation** —
  this RFC; included for traceability.

## Explicitly not on the roadmap

- SAML.
- Implicit or hybrid OIDC flows.
- Dynamic client registration over the public internet (while
  sui-id remains in the first-party deployment model; see
  RFC 008).
- A built-in clustering / multi-master mode.
```

The "Near term" / "Longer term" categorisation comes from
the existing ROADMAP. The compression rule: every item is one
line plus the RFC link; if the implementer needs more detail,
they read the RFC. ROADMAP becomes a navigation surface, not
a content surface.

The "Done in version X" lists from the current ROADMAP move
out — that information is in CHANGELOG. The ROADMAP describes
the future, not the past.

### § 3. PUBLISHING.md → docs/contributors/release-process.md

The file content is preserved verbatim, with two small edits:

- The opening paragraph clarifies the audience: "This page is
  for sui-id maintainers running a release. End users do not
  need it."
- A new short section is added at the end describing the
  CHANGELOG rotation rule from § 1 ("when the root carries
  three minors, move the oldest one to `docs/changelog/`").

Path: `docs/contributors/release-process.md`.

The README's link to `PUBLISHING.md` (currently a 404 per the
v0.29.3 codebase review; RFC 015 was supposed to either fix
or remove it) needs verification. If RFC 015's pass cleaned it
up, no further action. If a link still exists pointing at
`PUBLISHING.md`, it updates to `docs/contributors/release-
process.md`.

### § 4. The new docs/contributors/ subdirectory

Per the project's persona-based docs structure
(first-time-user / intermediate-user / contributor), this
subdirectory is the contributor slot:

```
docs/
  changelog/         (history reference)
  contributors/      (maintainer / contributor playbooks)
    release-process.md
  deployment.md      (operator)
  integrators.md     (intermediate user / RP integrator)
  operators.md       (operator)
  threat-model.md    (security reviewer)
```

This RFC adds two subdirectories: `changelog/` (history) and
`contributors/` (playbooks). Both are inside `docs/`, so the
root layout is unaffected.

A future RFC may add `docs/tutorials/` for first-time users
and `docs/api/` for intermediate users (the persona slots
that currently lack a directory). Out of scope for this RFC;
mentioned only so the directory plan is coherent.

### § 5. Root README link audit

The README references in scope:

- Any link to `PUBLISHING.md` → updates to
  `docs/contributors/release-process.md`.
- Any link to `TERMS_OF_USE.md` → unchanged (file remains in
  root; it is small and legally relevant).
- Any link to `CHANGELOG.md` → unchanged (file remains in
  root; the link still works for the latest entries).
- Any link to `ROADMAP.md` → unchanged (file remains in root;
  shape changes but the file is still there).

A `grep -E "(PUBLISHING|CHANGELOG|ROADMAP)\.md"` over the
repository finds every reference; each is reviewed.

### § 6. mdbook readiness (informational)

The project instructions call for `docs/` to be mdbook-ready.
Today `docs/` is a flat folder. The new structure introduced
here is more mdbook-friendly, not less:

- `docs/contributors/` becomes a chapter group.
- `docs/changelog/` becomes an appendix or a "Reference"
  chapter.
- The existing flat files (operators, integrators,
  deployment, threat-model) become top-level chapters.

Adding `docs/book.toml` and a `docs/SUMMARY.md` is **not** in
scope for this RFC. That work is its own RFC, partially
because the SUMMARY ordering is a design decision in itself.
This RFC only ensures the file layout is amenable to that
work later.

## Tests

Documentation-only; no automated tests.

Manual verification checklist on landing:

1. `wc -l CHANGELOG.md` ≤ ~250.
2. `wc -l ROADMAP.md` ≤ ~100.
3. `find docs/changelog/ -name '*.md'` contains one file per
   historical minor; concatenating them top-to-bottom plus
   the root entries reproduces the pre-split content.
4. `test ! -e PUBLISHING.md` (the file is gone).
5. `test -e docs/contributors/release-process.md` (and its
   content matches the old PUBLISHING.md plus the rotation
   note).
6. No 404s when clicking through README from the root.

## Security considerations

None. Documentation reorganisation has no security impact.

A small risk: a stale link in a downstream README, blog post,
or issue tracker pointing at `PUBLISHING.md` will 404 after
this RFC ships. The risk is low (PUBLISHING.md is a maintainer
file and is unlikely to be linked externally) and unavoidable
without leaving a redirect. GitHub does not support file-level
redirects in repositories. Acceptable.

## Multiple implementation steps

Can ship in two PRs:

- **PR 1.** Move PUBLISHING.md → docs/contributors/release-
  process.md; update README link. Trivial.
- **PR 2.** Run the split script for CHANGELOG; manually
  compress ROADMAP. Larger diff but mechanical.

Both can also ship in one PR. The split is mechanical enough
that bundling them is fine.

## Open questions

1. **Per-minor vs per-major archive granularity.** § 1
   chooses per-minor (`0.29.x.md`). For a project that
   reaches 1.0 and stays there, per-major might be more
   natural. The choice now is per-minor because sui-id is
   pre-1.0 and minors carry meaningful content; revisit at
   1.0.
2. **TERMS_OF_USE.md placement.** This RFC leaves it in the
   root because the file is short and legally relevant.
   Open question whether it should also move under `docs/`
   (as `docs/legal/terms-of-use.md`); recommend leaving it
   in root for now since it is not a long file pressuring
   the root.
3. **Should the split script live in the repo?** No. The
   split is a one-shot. Re-running it on an already-split
   tree produces wrong output. Leave it as a maintainer
   shell script run once and discarded; the result is what
   gets committed.

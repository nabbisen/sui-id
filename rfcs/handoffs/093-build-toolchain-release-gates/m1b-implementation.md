# RFC 093 M1b — documentation and lifecycle gates

**Governing RFC:** [RFC 093](../../accepted/093-build-toolchain-release-gates.md)
**Lane:** B (second implementer)
**Exit gate:** G10a, G10b, G11 and G12 pass hosted; RFC integrity reports no
known debt and carries no permanent allowlist.

M1b shares no files with M1a and may run concurrently from the start. Python
3.14.6 and mdBook 0.5.4 are present and match `ci/gate-inputs.toml`.

**Owns:** `scripts/check-markdown-links.py`; `scripts/check-rfc-integrity.py`;
`ci/rfc-policy.toml`; `scripts/tests/test_markdown_links.py`;
`scripts/tests/test_rfc_integrity.py`; RFC status and link corrections;
`rfcs/README.md`; retirement of the four legacy inline UI jobs.

---

## C0 — G10a mdBook lane

**This was missing from the handoff until 2026-07-29.** The M1b exit gate and
`README.md` both require G10a, but no theme implemented it: A0.2 fixed the icon
so `mdbook build docs` succeeds, and A3.2 built its negative fixture, yet nothing
wired the lane itself. Surfaced by the A3.2 implementer's scope question, which
was correctly declined as out of A3.2's scope.

Wire G10a as a blocking job running the Gate Matrix command verbatim:

```
mdbook build docs --dest-dir ../target/mdbook-gate
```

Use the dispatcher like every other lane. **`[gates]` does not currently carry a
G10a entry** — it holds G01–G09b only, because those are the `cargo` lanes the
dispatcher was built for. Adding G10a to `[gates]` also requires extending
A3.4's condition 7 lane filter (`^G0[1-9][a-z]?$`), which would otherwise reject
it. Decide one of:

- **(a)** add G10a to `[gates]`, widen A3.4's filter, and dispatch it like the
  others — uniform, but touches a checker that is already reviewed and landed; or
- **(b)** run G10a as a direct step in its job, outside the dispatcher, on the
  grounds that it is not a `cargo` lane and the manifest's `[gates]` is
  documented as the dispatcher's command table.

**Raise this as a design question before implementing** — it changes A3.4's
contract either way, and that is not the implementer's call.

**Installation is already specified** by RFC 093: exactly
`cargo +stable install mdbook --version 0.5.4 --locked`, with the version
pinned in `ci/gate-inputs.toml` `[tools]`. No new SHA-pinned action is needed.

The negative self-test for this lane already exists from A3.2 — see that
package's `mdbook-missing-chapter` fixture and note the
`create-missing = false` requirement recorded in RFC 093.

## C1 — G10b markdown link checker

Command: `python3.14 scripts/check-markdown-links.py --root . README.md ROADMAP.md docs`

Negative unittest fixtures (`python3.14 -m unittest scripts.tests.test_markdown_links`)
must reject: a missing file, a bad anchor, an absolute local path, and a
case-mismatched path.

**Scope rule.** M1b may repair link targets, anchors, path spelling, case, and
moved-file references. It may **not** change a feature claim, scope statement,
security-posture wording, or any other semantic content — that is RFC 098 in M5.
If a repair would change meaning rather than a path, stop and raise it.

Known mechanical debt in the scanned set: `README.md:169` points at a moved
router, and `README.md:205` contains a stray fragment.

---

## C2 — G11 RFC integrity checker

Command: `python3.14 scripts/check-rfc-integrity.py --root . --policy ci/rfc-policy.toml`

Implement the invariant list in RFC 093's RFC-integrity contract: one unique
identifier across lifecycle folders; folder and `Status` agreement; every RFC
indexed exactly once at a resolvable relative path; every relative Markdown link
in an RFC resolves; no RFC file directly under `rfcs/`; required metadata for
every standard numeric RFC with identifier 093 or greater; the closed historical
`RFC-MI-*` list enumerated in `ci/rfc-policy.toml`; and evidence rules for the
`Independent design review` and `Closure evidence` metadata fields — their
targets must resolve, be tracked per `git ls-files --error-unmatch`, and not be
ignored per `git check-ignore`.

Parser constraints: recognise the bold, period-terminated metadata labels only
in the RFC header before the first level-2 heading, and never treat an ordinary
content link as evidence.

Fixtures: one invalid and one boundary-valid case per invariant, including
duplicate numbers, status mismatch, missing index row, broken link, missing
prospective metadata, an invalid evidence path, and a valid historical RFC that
correctly carries no invented review metadata.

### Current state the gate must accept as correct

Two things look like debt and are not. Do not "fix" them:

- **RFCs 094, 095 and 096 are legitimately in `proposed/`** with
  `Status: Proposed`, returned there on 2026-07-28 for owner-approved material
  amendments. Their `Lifecycle history` fields record the prior acceptances.
- **`rfcs/reviews/094-federation-command-amendment-review-2026-07-21.md` and
  `rfcs/reviews/096-design-review-2026-07-21.md` intentionally still reference
  `accepted/09{4,6}-*`.** They are dated historical evidence tables citing files
  as they existed at that review, not live navigation. RFC 000 directs against
  rewriting historical decisions to match current state.

---

## C3 — broken-link debt, measured 2026-07-28

A full sweep of every tracked file under `rfcs/` found **29 unresolvable
relative links across five historical `done/` RFCs**. None was introduced by the
governance commit. G11 requires every relative link in an RFC to resolve, so
this is M1b scope. Three categories, handled differently:

| Category | Files | Count | Treatment |
|---|---|---:|---|
| Illustrative examples | `000-rfc-lifecycle-policy.md`, `018-rfc-lifecycle-policy.md` | 15 | **De-link — do not create files** |
| Wrong relative prefix | `022-single-realm-scope-statement.md`, `024-doc-file-consolidation.md` | 10 | Repair prefix **and** target |
| Moved doc targets | `076-configuration-reference.md` | 4 | Repoint into `docs/` |

### Design decision on the illustrative examples — settled

The two lifecycle-policy documents contain example index tables demonstrating
what an RFC index should look like, with entries such as
`042-feature-flags`, `023-multi-region`,
`035-old-caching`, `010-revoke-tokens`, `015-deprecate-old-api`, and
`050-session-hardening`. **None of those RFCs exists or should exist** — they
are format illustrations, not navigation.

Do **not** create the files. Do **not** add them to an exemption list in
`ci/rfc-policy.toml`: RFC 093 Requirements item 7 forbids the gate from carrying
an indefinite baseline allowlist. **De-link them** — render each as plain text
or inline code so the example table still reads correctly but contains no link
for the gate to resolve. Moving an example block into a fenced code block is
equally acceptable; choose per table and stay consistent within a file.

### The other two categories are genuine defects

In `022` and `024` the prefix is wrong: `./rfcs/proposed/X` written from inside
`rfcs/done/` resolves to `rfcs/done/rfcs/proposed/X`, and `../rfcs/proposed/X`
is wrong the same way. The correct form is `../proposed/X` — and several targets
have since moved to `done/`, so check each target's current folder rather than
only fixing the prefix.

In `076`, `../guides/deployment.md` and `./operators.md` should be
`../../docs/deployment.md` and `../../docs/operators.md`.

Re-run the sweep after repair. The count must reach zero with no allowlist entry.

---

## C4 — RFC status debt

**39 of 105 RFCs in `rfcs/done/` lack an Implemented status.** Also correct
`rfcs/README.md:354`, which links to a non-existent `proposed/089-*.md` while
RFC 089 lives in `done/`, and RFC 024, which sits in `done/` with
`Status: Proposed`.

Do not invent retrospective reviewers — RFC 093 explicitly allows historical
RFCs to be checked for number, folder, status, index and link integrity without
fabricating review metadata. Where a status is genuinely unknown, raise it
rather than guessing.

The gate must land with no ignored failures and no permanent allowlist.

---

## C5 — retire the four legacy inline UI jobs

Only after: G12's empty-value fix is committed (done, `50bd4a7`); the Wave 2 CI
integration is committed (done, `7055894`); and **one hosted run has been
observed green with both `ui-invariants-v1` and the four legacy jobs passing on
the same commit**.

Then remove `text-leaks`, `css-tokens`, `semantic-palette-parity` and
`inline-style-bound` from `ci.yml` in a separate reviewed change. Do not combine
removal with any other edit.

# RFC 093 M1b — documentation and lifecycle gates

**Governing RFC:** [RFC 093](../../done/093-build-toolchain-release-gates.md)
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

**The command was corrected on 2026-07-30.** RFC 093 originally specified
`--dest-dir ../target/mdbook-gate`; mdBook 0.5.4 resolves `--dest-dir` against
the current working directory, not the book root, so that wrote the book one
level *above* the repository. Raised by the C0 implementer, verified, and
amended in the RFC. Use the path below.

Wire G10a as a blocking job running the Gate Matrix command verbatim:

```
mdbook build docs --dest-dir target/mdbook-gate
```

**Design question resolved 2026-07-30 — option (a).** G10a goes through the
dispatcher and into `[gates]`, and A3.4's lane filter widens to admit it.

Why not the simpler option of running `mdbook` as a direct step: it would lose the
dispatcher's evidence block — the HEAD/`GITHUB_SHA` binding, the clean-tree
assertion, and the recorded tool versions. That block is the entire reason D1
chose a dispatcher, and running a blocking lane without it recreates review
finding B2 (evidence not bound to the commit it describes). The dispatcher is not
cargo-specific; it executes a recorded command string, and
`mdbook build docs --dest-dir target/mdbook-gate` is one.

Two changes are therefore required together:

1. **Add G10a to `[gates]`** with the command exactly as RFC 093's table records
   it, and dispatch the job as `bash scripts/ci-gate.sh G10a`.
2. **Extend A3.4's condition 7 lane filter by exact enumeration.** It is currently
   `^G0[1-9][a-z]?$`, which rejects `G10a`. Change it to
   `^(G0[1-9][a-z]?|G10a)$`.

   **Enumerate; do not widen to a range.** A range such as
   `^G(0[1-9]|1[0-2])[a-z]?$` would also admit G10b, G11 and G12 — none of which
   is in `[gates]` yet — and the set comparison would fail on C0's own commit.
   C1 adds `|G10b`, C2 adds `|G11`, and C2.1 deletes the enumeration entirely.

   Also correct the comment above that line. It currently reads "Only G01-G09b
   are expected in `[gates]`; G10a-G12 use separate mechanisms … intentionally
   not part of the ci-gate.sh dispatcher's command table." That is no longer
   true of G10a, G10b or G11 — it remains true only of G12.

**The completeness rule is not part of C0** — see C2.1. My start authorization
attached it here, which was wrong; the reason is recorded there.

**Installation is already specified** by RFC 093: exactly
`cargo +stable install mdbook --version 0.5.4 --locked`, with the version
pinned in `ci/gate-inputs.toml` `[tools]`. No new SHA-pinned action is needed.

The negative self-test for this lane already exists from A3.2 — see that
package's `mdbook-missing-chapter` fixture and note the
`create-missing = false` requirement recorded in RFC 093.

---

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

### Header-format decision, 2026-08-01

Three header conventions coexist in the 117-file corpus: bold-label (100 files),
YAML-ish bullets (1 file, `done/077-headless-setup.md`), and TOML front-matter in a
fenced block (16 files, all `RFC-MI-*`). Raised by the C2 implementer with the
corpus measured rather than assumed.

**Decision: read TOML front-matter, but only for identifiers on the closed
historical MI list in `ci/rfc-policy.toml`. Convert RFC 077's header to
bold-label.**

Rejected alternatives and why:

- **Read TOML generally** — a new RFC could then use that format and pass. RFC 093's
  parser constraint names the bold-label form; two accepted conventions means none
  is enforced. The narrowing preserves prospective single-convention enforcement.
- **Exempt the MI files from invariant 2 as well as 6/7** — the status is present
  and already correct in all sixteen (`done/` + `Implemented (v0.49.1…v0.57.0)`).
  Declining to read a field that exists and would pass makes the gate assert less
  than it can.
- **Convert all seventeen headers** — sixteen of them are frozen historical files
  that would be rewritten to state what they already state, with transcription risk
  and no gain.

The closed list is the right hook because `ci/rfc-policy.toml` already declares its
own semantics: *"This list is closed: a new MI RFC never gets added here."* The TOML
path therefore can never apply to a file added later.

**Implementation note.** The TOML block sits inside a ```` ```toml ```` fence, which
`iter_unfenced_lines` correctly strips. The narrowed reader must operate on the raw
header text before fence-stripping, and only when the identifier is on the closed
list.

**RFC 077** is a single standard-numbered file, not part of any closed set. Convert
its bullets to bold-label preserving every value verbatim, including the author
attribution. Its `Status: accepted` in `done/` is a separate genuine defect — let
the gate report it and fix it in C4; do not correct the value while changing the
format.

**Falsifiable prediction:** after the parser change alone the count should fall from
63 to **47** (38 folder/status + 1 for 077 + 2 unindexed + 6 links). If it is not
47, report the number rather than reconciling to it.

### Known latent defect in G10b — do not fix in M1b

`check-markdown-links.py` (G10b) uses a naive "any fence line toggles" parser.
`check-rfc-integrity.py` (G11) uses correct CommonMark fence-closing semantics after
`rfcs/done/076` exposed the difference.

**Verified unreachable in G10b's scope today**: both algorithms were run over all 51
files G10b scans and produced identical link sets, with `rfcs/` as a positive control
where they differ in exactly one file. Do not churn a green, hosted-verified lane for
symmetry.

**Trigger condition:** if `docs/` ever gains a fenced block containing a nested
fence, G10b silently stops scanning the rest of that file. The remedy is a shared
helper rather than two divergent implementations — cleanup for M5.

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

## C2.1 — the lane completeness rule

**Lands after C2, not with C0.** By this point `[gates]` holds G01–G09b, G10a,
G10b and G11 — every dispatched lane — and the rule can be stated in its final
form with no transitional exemptions.

Delete condition 7's enumerated lane filter and replace it with:

> Every lane in RFC 093's Gate Matrix v1 table must be either present in
> `[gates]` or listed on an explicit, commented exception list in the manifest.
> Nothing may be absent from both.

**G12 is the sole exception.** It predates the dispatcher and has a working entry
point (`ui-invariants-v1`). Record it with that reason. Do not migrate it —
churning a green gate for symmetry is not worth the risk.

**Corrected 2026-08-02:** an earlier version of this section called the exception
"permanent." That was wrong. There is no principled reason G12 must stay outside
the dispatcher forever — only that migrating it costs something and buys nothing
today. The exception is **provisional but unscheduled**. Write the reason so the
durable part (predates the dispatcher, has its own entry point) stands on its own,
and keep any time-bound clause such as "C5 depends on it" clearly subordinate — it
expires when C5 lands.

### Why this rule exists

The M1a C1-correction review noted that A3.4 only ever checked one direction (that
"C1" is the M1a gate-inputs correction, not M1b's C1 item): every `[gates]`
entry matches its RFC row, but nothing verified that every RFC lane is
*accounted for*. A lane could be absent from the manifest and no check would
object. This is the same class as R9 — a check that passes because it never
looked.

### Fixtures

- A lane present in the RFC table but in neither `[gates]` nor the exception
  list must **fail**.
- A lane on the exception list must **pass**.
- A lane present in **both** `[gates]` and the exception list must **fail** —
  the two lists are disjoint by construction, and an entry in both means one of
  them is stale.

### Also fold in: `[tools]` is currently unenforced

Found during the C1 review, recorded here because C2.1 is already rewriting this
checker and it is the same defect class.

`ci/gate-inputs.toml` declares `[tools]`:

```toml
rust_msrv = "1.95"
rust_stable = "stable"
mdbook = "0.5.4"
python = "3.14"
```

**A3.4's seven conditions never read this table.** Nothing compares it to the
workflow. `ci.yml` even carries a comment asserting "ci/gate-inputs.toml's
`[tools]` table pins this version" next to `cargo install mdbook --version 0.5.4`
— true as a declaration, false as an enforcement.

Coverage today is uneven and accidental:

- **`rust_msrv`, `python`** — transitively enforced, because the gate commands
  themselves embed `+1.95` and `python3.14`, and condition 7 compares those against
  RFC 093's table.
- **`mdbook`** — **not enforced at all.** `0.5.4` appears only in `ci.yml`'s install
  step and in `[tools]`, with nothing tying them together. `ci.yml` could install
  0.6.0 and every check would still pass while the manifest and the RFC both claim
  0.5.4.

That last one is a live risk rather than a theoretical one: mdBook 0.5.4's specific
behaviour has already caused two defects in this milestone (auto-creating missing
chapters, and CWD-relative `--dest-dir`). An unenforced version pin on a tool whose
version-specific behaviour keeps mattering is worth closing.

**Add a condition** asserting that every `[tools]` entry corresponds to what the
workflow actually installs or invokes, with fixtures for a drifted version and a
declared-but-unused tool. If a tool's version cannot be mechanically located in the
workflow, that is itself the finding — say so rather than skipping it.

This is an M1a-era gap in A3.4, not a C1 or C2 defect.

**Correction from the C2.1 review, 2026-08-02.** The first implementation asserted
that the declared pin appeared *somewhere* in `ci.yml`, which lets **partial drift**
pass: one job moved to Python 3.13 while others stayed at 3.14 was reported as
`all conditions satisfied`. The rule must be that **every** occurrence equals the
pin, with pure-comment lines excluded so a commented-out stale version cannot
false-positive. Verified: baseline and all fixtures still pass, partial drift is
caught naming both values, and a commented `python-version: "3.9"` is ignored.
Requires a third fixture for the some-right-some-wrong case; the original two cover
only all-wrong and none-found.

### One extraction hazard, verified

The lane set must come from the **`## Gate Matrix v1` section only**. RFC 093 has
a second table under `### Gate entry points and negative self-tests` whose rows
are keyed by G09a, G09b, G10b, G11 and G12 — the same IDs, with *fixture*
commands in a different column. The existing extraction is correctly bounded
heading-to-heading and yields exactly 15 lanes; a whole-file scan yields 20 and
would produce a confusing command-mismatch failure. **Do not re-implement the
extraction** — reuse the existing `rfc-matrix-section` step.

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

**RFC 076's six links — corrected instruction, 2026-08-01.** An earlier version of
this handoff said to repoint them at `../../docs/deployment.md` and
`../../docs/operators.md`. **That was wrong**, and following it would have looked
mechanical while making a meaning change.

`docs/deployment.md` and `docs/operators.md` both exist *and* so do
`docs/src/guides/deployment.md` and `docs/src/guides/operators.md` — divergent
duplicates, differing by 37 and 177 lines respectively. Picking between them is a
documentation-authority decision, not a path repair.

Three facts settle which the RFC meant:

1. **`docs/src/guides` is the only `guides` directory in the repository.** The root
   layout is flat (`docs/operators.md`), with no `guides/` component at all — so
   `../guides/X` can only denote the book layout.
2. **RFC 076 was added 2026-06-17**, twelve days *after* the mdBook structure
   (`983693e`, 2026-06-05). The author was writing against a tree where
   `docs/src/guides/` already existed.
3. All three targets exist there, and `#backups` resolves in
   `docs/src/guides/operators.md`.

So repoint all six at the **book** copies, from `rfcs/done/`:

| Current | Correct |
|---|---|
| `../guides/deployment.md` (×2) | `../../docs/src/guides/deployment.md` |
| `../guides/operators.md#backups` | `../../docs/src/guides/operators.md#backups` |
| `../guides/operators.md` | `../../docs/src/guides/operators.md` |
| `../guides/upgrade.md` | `../../docs/src/guides/upgrade.md` |
| `./operators.md` | `../../docs/src/guides/operators.md` |

All verified to resolve. `upgrade.md` has **no** root duplicate, which independently
confirms the book layout is the intended referent.

**Do not treat the root/book duplication itself as C3's problem.** Two divergent
copies of the same document is RFC 098/M5 work; C3 only picks the referent the RFC
already meant.

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

**Precondition satisfied 2026-08-02**: run `30735008212` on `bfbb2e4` had
`ui-invariants-v1` and all four legacy jobs green on the same commit, and every
run since has repeated it.

Then remove `text-leaks`, `css-tokens`, `semantic-palette-parity` and
`inline-style-bound` from `ci.yml` in a separate reviewed change. Do not combine
removal with any other edit — if a regression appears, the cause must be
unambiguous.

Expected result: **21 jobs → 17.**

### Two things to check rather than assume

1. **A3.4 condition 6** requires every gate-lane job in `ci.yml` to use the
   `[runner]` label, and it reads job keys. Removing four jobs should not affect
   it, but run `bash scripts/check-gate-inputs.sh --all --policy ci/gate-inputs.toml`
   and confirm rather than assuming. Condition 8 also reads `ci.yml`, so confirm
   the `[tools]` pins are still located after the removal.

2. **The two GitHub annotations will disappear, and that is correct.**
   *This item said the opposite until 2026-08-02; the correction is recorded in
   the C5 annotation-visibility decision.*

   `::warning::Standalone 't.field' lines` and `::warning::Declared tokens with
   no references` were echoed by the **legacy jobs themselves** — `text-leaks`
   and `css-tokens` — not by `ui-invariants-v1`. `check-ui-invariants.sh`
   contains **zero** annotation syntax. Removing those jobs removes the
   annotations by construction.

   **What must survive is the detection, and it does.** Verify by running the
   consolidated script and checking the counts are unchanged:

   ```
   bash scripts/check-ui-invariants.sh --all --policy ci/ui-invariants.toml
   ```

   Expect `G12 advisory standalone-translations` with **3** entries and
   `G12 advisory unused-css-tokens` with **18**, matching what the legacy jobs
   reported. Those lines land in `ui-invariants-v1`'s job log. If the *counts*
   change, stop and raise it — that would be a real coverage change. The
   annotations vanishing is not.

   Do **not** add `::warning::` formatting to `check-ui-invariants.sh`. RFC 093
   consolidated four inline CI implementations into one reviewed entry point;
   embedding GitHub Actions rendering syntax in it would re-couple that script to
   one CI platform, and it is run locally too. Two permanent warnings on every run
   also erode the signal value of annotations generally.

G12 stays in `[gate_matrix_exceptions]` throughout; C2.1's exception reason cites
this item, and that citation is a current-state note rather than a permanent
justification.

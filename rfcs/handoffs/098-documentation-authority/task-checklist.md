# RFC 098 reconciliation checklist

**Governing RFC:** [RFC 098](../../accepted/098-documentation-authority-reconciliation.md)

## 0 — Order of work, added at acceptance 2026-09-10

**The authoritative order is RFC 098 §Design 5**, a seven-step plan ordered by
severity and measured against `ee48257`. Work it in that order; this checklist
is the wider content sweep that follows, and its §4 contradictions merge into
step 6.

Steps 1–3 close every public-facing falsehood and come first:

1. Staleness banner on `docs/threat-model.md` — it declares itself current as
   of v0.26.0 at a v0.77.0 workspace, and `README.md` presents it publicly.
2. Delete `docs/deployment.md`, `docs/operators.md`, `docs/integrators.md` —
   stale forks of the book pages, holding no unique content and one false
   claim — and repoint `README.md` at the book pages with **repo-relative**
   links, not `https://github.com/...` URLs, which G10b cannot see.
3. Repoint `crates/sui-id/src/cli.rs` and
   `crates/sui-id/src/http/handlers/setup.rs`, which cite a deleted path.

Step 3 is the only one that touches `crates/`. Do not start step 4 or later
without a dispatch; steps 5 and 7 depend on open question 1, which acceptance
did **not** decide.

### Dispatch 2 — 2026-09-12: the last absolute self-URLs, then step 4

**Landed 2026-09-12** — 2a `e06b8b8` (two of three; the third was correct under
rule 6 and stays), 2b `c451c52`. Review:
`.git-exclude/reviewed/rfc-098-dispatch-2-and-g14-g15-2026-09-12.md`.

Steps 1–3 landed as `2671822` (reviewed:
`.git-exclude/reviewed/rfc-098-steps-1-3-2026-09-10.md`). Two small items,
both fully specified; do them as **two commits**, in this order.

**2a — Replace the remaining absolute self-URLs.** The count of twenty-six
in `README.md` §Findings above was measured before steps 1–3 and is stale.
Re-measured at `c97027c`, excluding `rfcs/done/`, `docs/changelog/` and
`CHANGELOG.md` (historical, rule 4), and excluding RFC 098's own text that
*describes* the pattern with `…` placeholders, **three** remain:

| File | Line | Target | Replace with |
|---|---|---|---|
| `README.md` | 205 | `…/blob/main/docs/threat-model.md` | `docs/threat-model.md` |
| `README.md` | 208 | `…/blob/main/PUBLISHING.md` | `PUBLISHING.md` |
| `docs/src/getting-started/overview.md` | 37 | `…/blob/main/ROADMAP.md` | `../../../ROADMAP.md` |

Why: an absolute URL to the project's own file is a repo-relative link
written wrongly. G10b skips external links, so it cannot see whether the
target exists — the README's link to F1's own document was invisible to the
gate for that reason. The third one is a book page: mdBook renders a
relative link to a file outside `docs/src/` as a broken page link on the
published site, so check the rendered output, not only the link checker.
If the rendered link is broken, say so and stop; do not substitute a
different target on your own.

Verify by re-running the measurement above: it must return zero. Report the
command and the count.

**2b — Step 4: evict the audit-coverage matrix to `ci/`.** Measured blast
radius at `c97027c`; smaller than RFC 098 §5 step 4 says — **RFC 093 does
not reference the path**, so there is no closed-RFC edit. That sentence in
step 4 was wrong.

```
git mv docs/src/reference/audit-coverage-matrix.md ci/audit-coverage-matrix.md
```

then repoint exactly these:

| File | What |
|---|---|
| `scripts/check-audit-matrix.sh` | line 4 (comment) and line 14 (`MATRIX=`) |
| `crates/sui-id-store/src/commands.rs` | line 543, a code comment naming the path |

Leave as written: the three dated RFC 093 review records under
`rfcs/handoffs/093-…/`, the superseded note at
`rfcs/handoffs/094-transactional-audit/migration-checklist.md:341`, and
RFC 101 §6.2 — those name the bare filename or are historical prose, and
stay true or stay dated after the move. RFC 098 §2 already names
`ci/audit-coverage-matrix.md` as the authoritative location; its F4 and
step 4 text are the record of why and stay.

The file has no outward links and nothing under `docs/` links to it, so
mdBook is unaffected; it was never in `SUMMARY.md`. `ci/` holds only
`.toml` today — a `.md` there is new, and correct: it is D3, a
machine-consumed contract, and the gate that reads it is the only thing
that makes it true.

Evidence: `bash scripts/check-audit-matrix.sh` must still report
**54 matrix entries, 54 source literals** — the same numbers as before the
move, which is the proof the script found the file at its new path rather
than passing on an empty read. Plus G10a, G10b, G11, fmt, both clippy
scopes (the `commands.rs` comment edit touches a crate).

**Not in this dispatch:** step 5 (the seven MI matrices) and step 7 wait on
open question 1; step 6 (`development-specification.md`) is a separate
dispatch. One further item is with the owner, not with you.

### Dispatch 3 — 2026-09-12: RFC 098's own lanes, G14 and G15

**Landed 2026-09-12** — G14 registered `73e49a2`; G15 built and held `336b9ab`.
G14 caught a real break on its first run (the matrix link in a dated 093 record).

Independent of dispatch 2. RFC 098 now declares its lanes in §Gate Matrix lanes
owned by RFC 098, registered through RFC 094's R10 registry; open question 2 is
answered by that. Handoff: [`g14-g15-doc-lanes.md`](g14-g15-doc-lanes.md).
G14 registers now (zero new code, green today). G15 is built now and held in
`[gate_matrix_exceptions]` until steps 4–6 clear the tree.

### Dispatch 4 — 2026-09-12: the outside-book link class, and check (B) under rule 6

**Landed 2026-09-12** — 4a `9bfaecf` (nine links, not the stated ten: five
threat-model links, not six), 4b `30f16a6`. G15 on the tree: 7 (A), 0 (B), 1 (C).
Review: `.git-exclude/reviewed/rfc-098-dispatch-4-and-g13-2026-09-12.md`.

Dispatches 2 and 3 landed (review:
`.git-exclude/reviewed/rfc-098-dispatch-2-and-g14-g15-2026-09-12.md`). The §1
stop condition produced a rule — RFC 098 §4 rule 6 — and this dispatch applies
it. Two commits.

**4a — four links, the other direction.** These render to files the book never
produces (`../../../X.html`). Convert each to the absolute repository URL, the
form `overview.md:37` already has and which was right all along:

| File | Line | Now | Becomes |
|---|---|---|---|
| `docs/src/contributing/local-dev.md` | 118 | `../../../rfcs/done/000-rfc-lifecycle-policy.md` | `https://github.com/nabbisen/sui-id/blob/main/rfcs/done/000-rfc-lifecycle-policy.md` |
| `docs/src/contributing/state-contract.md` | 121 | `../../../crates/sui-id-i18n/STATE_WORDS.md` | `…/blob/main/crates/sui-id-i18n/STATE_WORDS.md` |
| `docs/src/guides/operators.md` | 6 | `../../../README.md` | `…/blob/main/README.md` |
| `docs/src/reference/oidc-api.md` | 482 | `../../../ROADMAP.md` | `…/blob/main/ROADMAP.md` |

The other `../../threat-model.md` links inside `docs/src` — the six the
steps 1–3 review left for the owner — are the **same class** and are in scope
here too: `docs/threat-model.md` is outside the book. Convert them the same
way. Measure the full set with `grep -rnoE '\]\((\.\./)+[^)]*\)' docs/src`
filtered to targets that resolve outside `docs/src/`, and report the count
before and after; the after must be zero.

**4b — check (B) enforces rule 6.** In `scripts/check-doc-authority.py`: a link
whose source is under `docs/src/` and whose target starts with the self-URL
prefix is **allowed if and only if** the stripped path (after `blob/<ref>/`)
resolves to a tracked file *outside* `docs/src/`; it fails if the file does not
exist, or if it is inside `docs/src/` (a book page linking to a book page must
be relative). Outside `docs/src/`, every self-URL still fails. Also add the
inverse: a relative link from a `docs/src/` page whose resolved target is
outside `docs/src/` fails, naming rule 6 — that is the four links above, and
without this half the rule is enforced in one direction only. Tests: absolute
outside-book from a book page passes; absolute to a missing file fails;
absolute to a book page from a book page fails; relative outside-book from a
book page fails; the existing outside-`docs/src` cases unchanged.

**Evidence.** G15 on the tree after both commits: **7 (A), 0 (B), 1 (C)** —
the (B) count falls to zero because `overview.md:37` becomes sanctioned and
the four converted links stop being relative-outside. Report the run verbatim.
mdBook build clean; G10b, G11, G14 green; Python suite green with the new
tests counted.

**Not in scope.** Registering G15 (still waits on steps 5 and 6); any change to
`check-markdown-links.py`; the MI matrices (open question 1).

### Dispatch 6 — 2026-09-12: step 6c, cut the specification to what holds

**Landed 2026-09-12** — one commit, one file; 59 kept sections byte-identical, G15
7/0/0 with no banner. Review: `.git-exclude/reviewed/rfc-098-dispatch-6-2026-09-12.md`.
Step 6 is complete. Every remaining G15 violation is step 5.

Dispatch 5's audit (review: `.git-exclude/reviewed/r10-c-and-spec-drift-audit-2026-09-12.md`)
decided the disposition: **cut, do not reconcile.** Every principle section
holds; every enumeration drifted because it is a second copy of an
authoritative artifact. One commit, one file, plus `ci/doc-authority.toml` only
if the claim regex needs the new header form (it should not — keep the phrase
"reflecting the v0.77.0 codebase").

**Header.** `*v4 — reflecting the v0.77.0 codebase (2026-09-12). Supersedes v3
(v0.48.4). Under RFC 098 this document is a synthesis of policy and principle,
not a source: inventories were removed in v4 and each section that held one
now names where the authoritative copy lives.*` Remove the v3 paragraph and
the step-6a staleness banner — within tolerance again, and check (C) will
force the banner back if it lags.

**Keep verbatim** — the sections dispatch 5 marked *holds*: §0, §1, §2, §5.1,
§6.2, §6.3, §7 (intro), §7.2, §7.3, §8.3, §10, §11.3, §11.4, §11.5, §11.7–11.14,
§12, §14, §15.1, §16, §17.1–17.3, §18, §19.1–19.3, §19.5, §19.7, §20, §22.1,
§22.2, §24, §25. Do not "improve" them.

**Amend briefly** — one to four sentences each, from the audit's findings,
citing the RFC:
| § | Change |
|---|---|
| 3 | Remove "Social login" and "External IdP federation" (RFC 004, 005 shipped). State today's out-of-scope from `ROADMAP.md` §Constraints and non-goals: multi-tenancy (RFC 025), alternative SQL backends beyond RFC 009 step 1, user-facing theming API. |
| 4 | Add: federation provider, user source (LDAP), client registration token, metrics token — one line each, wording from the RFC that introduced it. |
| 5.2 | Add OAuth 2.0 Dynamic Client Registration (RFC 7591) — RFC 008. |
| 5.3 | "OP" → "OP and, since RFC 004, relying party to upstream OIDC providers". |
| 6.1 | Add the four secrets: federation client secrets, LDAP bind credential, metrics bearer token, registration tokens. |
| 7.1 | Do **not** add threats here — that is `docs/threat-model.md`'s and RFC 097's. Add one sentence: the list predates RFCs 004/005 and the threat model's banner records the gap. |
| 11.1, 11.2, 11.6 | One sentence each naming the shipped subsystem and its page: federation and dynamic registration → `docs/src/reference/oidc-api.md`; LDAP user sources and the metrics endpoint → `docs/src/guides/operators.md`. |
| 13.2 | Delete "Consent retention is reserved for future expansion"; consent shipped (migration 0025). |

**Replace with a pointer** — delete the inventory, keep the section heading and
one or two sentences of principle if the section has any, then one line naming
the authoritative source:
| § | Pointer |
|---|---|
| 8.1 | Rust 2024 edition stays; toolchain floor and matrix → RFC 093 §Gate Matrix v1 and `rust-version` in `Cargo.toml`. |
| 8.2 | Delete the table → `Cargo.toml` `[workspace.dependencies]`. |
| 9, 9.1 | Delete the tree and the CLI list; keep the crate-responsibility sentences with the two paths corrected (`src/http/handlers/`, `src/runtime/dev_mode.rs`) → RFC 000 for `rfcs/`, `README.md` §Project layout, `sui-id --help`. |
| 13.1 | Delete the entity list → `crates/sui-id-store/src/migrations/`. |
| 15.2 | Delete the event list (and with it `auth.refresh.family_revoked`, which does not exist) → `ci/audit-coverage-matrix.md` and `docs/src/reference/audit-events.md`. |
| 17.4 | Keep the design-system principles; delete the JS-file table and the two named gates → `crates/sui-id/static/` and `ci/ui-invariants.toml` (G12). |
| 19.4 | Delete the list → the tree, RFC 000, §19.7. |
| 19.6 | Replace the body → RFC 098 §4 rule 6, one sentence stating it. |
| 21 | Delete the table and the "228/228" floor → RFC 093 §Gate Matrix v1, the lane tables in RFCs 094 and 098, `ci/gate-inputs.toml`. |
| 22.3 | Delete the `/mnt/user-data/outputs/` mechanism → `PUBLISHING.md` and RFC 093. |
| 23 | Delete the checklist → `ROADMAP.md` §Programme outcomes. |
| App. A | Delete → `CHANGELOG.md`. |
| App. B | Delete outright. |

**Evidence.** G15 on the tree: 7 (A), 0 (B), 0 (C) — (C) must stay zero with
the banner *removed*, which proves the re-pin is within tolerance. Every
pointer's target exists (G10b covers `docs`; run it). Line count before and
after. A section-by-section diff summary in the request, in the table's order,
so the review can check each disposition against this list. Nothing outside
the one file.

**Stop if** a *holds* section turns out to need a change, or a pointer's
target does not exist — both are findings, not edits.

### Dispatch 7 — 2026-09-12: step 6d, the specification's §7 under rule 7

**Landed 2026-09-13** — `0fc6a81`. Three headings removed, one body changed, 87
sections byte-identical. Review: `.git-exclude/reviewed/rfc-098-dispatch-7-and-8-2026-09-13.md`.

Owner ruling: security and the threat model have a single source of truth in
documentation — RFC 098 §4 rule 7. `docs/development-specification.md` §7
opens by deferring to `docs/threat-model.md` and then restates it: §7.1 twelve
threats (already missing four shipped surfaces), §7.2 eight properties, §7.3
known limits. A summary is a copy. One commit, one file.

- **Keep** the §7 heading and its first paragraph, amended to: *"The threat
  model is `docs/threat-model.md`, and it is the only place threats, defensive
  properties and known limits are stated (RFC 098 §4 rule 7). This
  specification does not summarise it; a summary is a copy, and the one this
  section carried through v3 had drifted."*
- **Delete** §7.1, §7.2 and §7.3 in full, headings included.
- Nothing else changes. The 56 other kept sections stay byte-identical — prove
  it the way dispatch 6 did.

**Evidence.** G15: 7 (A), 0 (B), 0 (C) — (C) stays zero, the v4 claim is
untouched. G10b, G11, G14 green. Section-split byte-identity for everything
outside §7.

### Dispatch 9 — 2026-09-13: the public surface — README, architecture, PUBLISHING

**Landed 2026-09-13** — 9a `110b96b`, 9b `ba5b3db`, 9c `80a77e1`, plus the review's
four rulings `85b83c4` (Chinese not selectable; JavaScript is four files, not
WebAuthn-only; one locale-resolution chain; RFC 098's own table). README has a
recorded evidence row per claim. Review: `.git-exclude/reviewed/rfc-098-dispatch-9-2026-09-13.md`.

RFC 098's mechanical half is done: steps 1–7 landed, G14 and G15 enforce. What
remains before closure is the semantic half of its closure prerequisites —
README, contributor guidance and public claims agreeing with the code. The
checklist's §3 and §4 below scoped this in July; **re-measured 2026-09-13
against `5002a89`**, this is what is still live. Three commits, in this order.

**9a — `README.md`.** Three defects, all on the public front page:

| Lines | Now | Fix |
|---|---|---|
| 82–84 | "If you want SAML, LDAP federation, dynamic client registration over the internet, or twenty IdP integrations out of the box: sui-id is not for you" | LDAP user sources (RFC 005), upstream OIDC federation (RFC 004) and dynamic client registration (RFC 008) all shipped. Rewrite the paragraph to disclaim only what is true today: SAML, multi-tenancy (RFC 025), a plugin system, a catalogue of pre-built IdP integrations. Keep its voice. |
| §Features (129–164) | Names none of LDAP, federation, dynamic registration, or the metrics endpoint | Add one bullet each, in the section's existing style, pointing at the book page that documents it (`oidc-api.md` for federation and registration; `operators.md` for LDAP and metrics). The front page must not disclaim two shipped subsystems while listing none of the four. |
| §Project layout (181–193) | Five crates | Six: add `sui-id-i18n` in the tree, one line, matching the others. |

**Deliverable with 9a, in the review request: a claim/evidence table for
`README.md`** — every factual claim the file makes (scope, features, MSRV,
layout, links), one row each, with the code path, RFC, or gate that makes it
true. RFC 098 §Requirements calls for this; it is how the review checks the
public front page against the tree rather than against taste.

**9b — `docs/src/contributing/architecture.md`.** Three contradictions, each
verified live:

| Line | Now | Truth |
|---|---|---|
| 25 | `crates/sui-id/src/handlers/` | `crates/sui-id/src/http/handlers/` |
| 37 | `Database` is "an `Arc<Mutex<Connection>>`" | RFC 009 step 1: `Database` wraps `Arc<dyn Backend>`; `SqliteBackend` owns the connection (`crates/sui-id-store/src/backend.rs`). Say that, and that the backend is pluggable by design. |
| 75–79 | "Every mutation goes through `events::emit(...)`" | False since RFC 094. Class-A mutations go through the sealed seam — `declare_write_command!` and `WriteTx<AtomicAudit>` — which commits the mutation and its audit row in one transaction; `events::emit` remains for Class-B events (six call sites). Rewrite the paragraph to say that, in this page's register, and point at RFC 094 and `ci/audit-coverage-matrix.md`. Do not restate the threat model (rule 7). |

Re-read the whole page while there: it is 104 lines, and the three above are
what a grep finds, not what a read finds. Report anything else as a finding
with its line, and fix only what the dispatch names unless it is a path.

**9c — `PUBLISHING.md`.** RFC 024 (done) decided it "does not earn a root
slot" and moved it to `docs/contributors/release-process.md`. That never
happened — the file is still at the root — and `docs/contributors/` predates
the book. Under RFC 098's domains it is D1 contributor documentation:
`git mv PUBLISHING.md docs/src/contributing/release-process.md`, add it to
`SUMMARY.md` under Contributing, repoint `README.md`'s link (repo-relative —
README is not a book page), and add a dated note to RFC 024 recording where it
landed and why the path differs. Check whether anything else links to it
(`git grep PUBLISHING`); the crate manifests' `readme` fields do not.

**Also check, report, do not fix:** `Cargo.toml` sets
`documentation = "https://docs.rs/sui-id"`. Confirm that URL renders something
for a binary crate. If it is empty or a stub, say so — where it should point is
a decision, not an edit.

**Evidence.** G10a, G10b, G11, G14, G15 green; the README claim/evidence
table; `python3.14 scripts/check-doc-authority.py` unchanged at all-satisfied.
Each commit's `git diff --stat` in the request.

**Stop if** a README claim has no evidence in the tree — that is a public
claim that is not true, and the fix is not editorial.

### Dispatch 10 — 2026-09-13: the operator and integrator guides, claim by claim

**Landed 2026-09-13** — 10a `90135ad`, 10b `316b53a`. 58 + 41 evidence rows; five stop-condition
findings ruled in `.git-exclude/reviewed/rfc-098-dispatch-10-2026-09-13.md`; one gate
defect dispatched under RFC 094 as G13-b.

The last item before RFC 098's closure prerequisites can be assessed:
*operator/integrator guidance agrees with the code*. Dispatch 9a walked
`README.md` with a claim/evidence table and found two false public claims the
July list did not have. Do the same for the two guides a reader acts on. Two
commits, one page each; the table for each page is the deliverable in the
review request.

**10a — `docs/src/guides/operators.md`** (1,612 lines). For every factual
claim — a configuration key, a CLI subcommand or flag, a file path, a default
value, a route, an audit event name, a behaviour ("X happens when Y") — one
row: the claim, and the code path, migration, or gate that makes it true.
Fix only what is a path, a name, or a value the code contradicts (rule 1),
and say so per row. **Stop on any behavioural claim the code contradicts** —
a guide that tells an operator the system does something it does not is the
F1 class — report it with the row and do not rewrite it.

**10b — `docs/src/reference/oidc-api.md`** (492 lines). Same method. Every
endpoint, parameter, claim name, error code, and "what sui-id does not do
(yet)" entry is a claim; the router, the OIDC handlers, and the token code
decide them. The "not yet" list is the one to read hardest: it is where
shipped features go to be denied, and it already had one such entry
(`docs/integrators.md`, retired in step 2).

**Method notes.** Work from the page to the code, not the reverse: the
table's job is to catch what the page says that the code does not, so
every sentence with a checkable fact gets a row, including the ones that
turn out true. Where a claim is *understated* (the code does more), record
it as true and note the understatement — that is the right direction to be
wrong in. Where a claim is about an RFC that is Accepted but not
Implemented, it is a claim about the future and must say so or go. Audit
event names are checked against `ci/audit-coverage-matrix.md`, not memory:
the specification carried a non-existent one for two years.

**Evidence.** The two tables; G15, G10a, G10b, G11, G14 green; each commit's
`git diff --stat`. The counts of rows *true / corrected / stopped-on* per
page, so the review can see the shape of the drift before reading the rows.

**Not in scope.** `docs/src/guides/deployment.md`, `upgrade.md`,
`dangerous-operations.md`, `reference/configuration.md`, `audit-events.md`
— same method, later dispatch, once these two show what the drift looks
like. `docs/threat-model.md` — RFC 097's, rule 7.

### Dispatch 11 — 2026-09-13: the guides' behavioural drift, and the field reference

**Landed 2026-09-13** — 11a `a0e7196`, 11b `29536ce`, 11c `71d7597`; the HIBP bullet
(finding 1.2) removed at merge. Findings 1.1 and 1.3 ruled → dispatch 12; the
eight dead variants → G13-c. Review: `.git-exclude/reviewed/g13-b-steps-3-4-and-dispatch-11-2026-09-13.md`.

Dispatch 10's tables found five behavioural claims the code contradicts and
stopped on them correctly. Ruled — RFC 098 rule 1 throughout: the page says
what the code does. Three commits.

**11a — `docs/src/guides/operators.md`, five rewrites.**

| § | Ruling |
|---|---|
| Operational model (454–472) | Rewrite to the outbox that shipped: RFC 001, v0.33.0. Sends go to `email_outbox` and a retry worker delivers them; `OutboxMailSender` is the production sender (`startup.rs:243`); the queue depth is exported by §Prometheus metrics on this page. The paragraph arguing an outbox is not worth building goes. |
| "planned future operation" (396–398) | Delete the sentence; point at §Rotating the master key, line 138. |
| "does not do" bullets (1278–1281 and the two beside it) | The confirmation email *is* sent — §Email features says so; make the two agree. Step-up shipped (RFCs 058–060) and password change is *deliberately* exempt from it: say that, not "we'll add". HIBP shipped: drop "alongside HIBP". |
| §Security events table | Remove the six `oauth.*` rows — never emitted, and the section tells operators to alert on them. Add `webauthn.credential.register` and `.delete`, which are emitted (after G13-b registers them in the matrix; if G13-b has not landed, stop and say so — the table must match the matrix). |
| Configuring — "The fields:" | Rule 3: this list is a second copy of `docs/src/reference/configuration.md`. Replace it with a one-line pointer; keep the prose around it. |
| §Self-service password change (1181 and 1240) | Keep the first, delete the second, repoint any anchor. |
| CSP paragraph | "the bundled `/static/webauthn.js`" → the four files, by name. |

**11b — `docs/src/reference/configuration.md` and eight code comments.** The
authoritative field reference is wrong in nine places: lines 192, 194, 211,
224, 226, 245, 255 (`[[user_source]]` → `[[user_sources]]`;
`[[federation_provider]]` → `[[federation_providers]]`) and 267, 287
(`[metrics]` → the `metrics_*` keys under `[server]`; a whole section retitled
and its example rewritten). Prove each corrected example loads, the way 10a
did. The same singular keys sit in eight doc comments — `config.rs:19,23,296,379`,
`user_source.rs:75`, `ldap_source.rs:29`, `state.rs:48`, `startup.rs:303` —
fix them; comments only, no code.

**11c — `docs/src/reference/oidc-api.md`, two omissions.** Add `email` and
`email_verified` to the ID-token claim table, conditional on the `email`
scope (`oidc/authorize.rs:465`). Add `fed` to the `amr` token table
(`auth_method.rs:48`); the table's own example already uses it.

**Evidence.** G15, G10a, G10b, G11, G14, G13 green. For 11a, a before/after of
each rewritten passage in the request. For 11b, the loader run per example.

### Dispatch 12 — 2026-09-13: what dispatch 11 found and could not fix

**Landed 2026-09-13** — 12a `aaa89f1`, 12b `871da56`, 12c `9db88e0`. Review:
`.git-exclude/reviewed/g13-c-stopped-and-dispatch-12-2026-09-13.md`. Two rows of
§Security events (`auth.session.revoked`, `auth.logout`) are false — no code
writes them — and leave with G13-c. After G13-c, closure is assessed.

Three commits. Two are code — small, tested, and each makes a documented
contract true.

**12a — `TokensConfig` per-field defaults** (`crates/sui-id/src/runtime/config.rs:74–78`).
`Config.tokens` is `#[serde(default)]` but its three fields are not, so any
`[tokens]` table that omits a key fails to parse — including the configuration
reference's own *Production-ready annotated configuration*, which omits
`id_token_lifetime_secs` under a comment saying it defaults to 900. Add
`#[serde(default = "…")]` to each field with the values the reference
documents (900, 900, 1209600), keep `deny_unknown_fields`, and add a unit test
that a `[tokens]` table with one key loads with the other two at their
defaults. Then prove the reference's production example loads unchanged, the
way 11b proved the others. Do not edit the reference's table: it stated the
intended contract; the code did not honour it.

**12b — two doc comments.** `outbox.rs:19` names `Config::email_outbox_*`
fields that do not exist (`max_attempts` is hardcoded at `main.rs:111`; the
backoff is `BACKOFF_SECS`) — say what is true. `auth_method.rs`'s doc comment
on `Fed` says "RFC 005: LDAP bind"; it is constructed by both LDAP sign-in
(`admin/auth.rs:200`) and federated sign-in (`federation.rs:637`) — say both.
Comments only.

**12c — `operators.md` §Security events describes the audit table only**
(ruling 1 of the review). Only `events::emit` writes a tracing line with an
`event` field, and none of the sixteen events in the table goes through it.
Remove the sentence promising a log line "in addition to the audit-log row",
the "filter on `event = …`" instruction, the three `jq` recipes and
§Account lockout's fourth; keep the table and the SQL recipe, which are true.
Say in one sentence that the audit table is the record and the log is not.
Re-run the page's claim table for the section.

**Evidence.** For 12a: the new test; `cargo test` count; the production
example loading. For all: G10a, G10b, G11, G14, G15, G13 (59/59), fmt, clippy
both scopes.

### Step 6a — landed 2026-09-12

Staleness banner on `docs/development-specification.md`, the rule-5 sanctioned
state. G15's (C) is now zero; (A) = 7, all step 5.

### Dispatch 5 — 2026-09-12: step 6b, the specification's drift audit

`docs/development-specification.md` is 1,229 lines written against v0.48.4. Do
**not** rewrite it. Produce the drift table first, as a review request: for
each numbered section, one row — *holds / contradicted / superseded / absent*
— naming the accepted RFC or the code path that decides it, and quoting the
contradicted sentence. RFC 098 rule 1: code wins over prose; rule 2: an
accepted RFC wins over any document except code. The §4 contradictions table
below is the seed; the audit must reach every section. No file changes in this
dispatch. The architect decides from the table whether the document is
reconciled section by section, or superseded by the authority table plus the
book and retired — that is a design decision the table informs.

### Dispatch 8 — 2026-09-12: step 5, retire the MI records; register G15

**Landed 2026-09-13** — `5002a89`. 23 files retired against `600d184`; five
references converted; **G15 registered — first green run.** Step 7 closes with it.

**Authorized by `@nabbisen`, 2026-09-12** — deletion of the 23 files included.
Open question 1 is ruled: no new folder, no gate change.

The first draft (move both MI packages into `rfcs/handoffs/RFC-MI-…/` and
extend invariant 13) is withdrawn on self-review — see RFC 098 §Open questions
1 and `.git-exclude/reviewed/oq1-oq3-self-review-2026-09-12.md`. If ruled as now
proposed, one commit:

1. `git rm -r docs/src/mockup-integration docs/mockup-integration` — 23 files.
   Record the pre-deletion commit SHA; it goes in every note below.
2. Dated notes, the RFC 085 form, in `rfcs/done/RFC-MI-000-baseline-delta-inventory.md`
   and `rfcs/done/RFC-MI-080-ui-regression-a11y-hardening.md`: the companion
   artifacts this RFC describes were retired to history on 2026-09-12 under
   RFC 098 step 5; recover with `git show <sha>:<path>`. The RFCs' own text is
   not rewritten.
3. The five link-form references become named paths, each with the same
   one-line note: `ROADMAP.md` lines 507 and 554, `CHANGELOG.md` line 3504 (a
   link target in a historical file — rule 4 permits it), `rfcs/README.md`
   line 106, RFC-MI-000 line 19. G11 link-checks every RFC file including the
   MI ones, so RFC-MI-000's must not be left as a dead link.
4. **No change** to `scripts/check-rfc-integrity.py`, `ci/rfc-policy.toml`, or
   `SUMMARY.md`.
5. Then, in the same commit: G15 moves from `[gate_matrix_exceptions]` to
   `[gates]`, with a `G15` job shaped like G14's. A3.4 must pass — check 3
   requires the lane in exactly one of the two tables.

**Evidence.** `python3.14 scripts/check-doc-authority.py --root . --policy ci/doc-authority.toml`
→ `all conditions satisfied`, exit 0 — the first green run in the lane's
life. G10a (mdBook must not miss the deleted pages: they were never in
`SUMMARY.md`). G10b, G14, G11 green. `bash scripts/ci-gate.sh G15` on a clean
tree, exit 0.

Independent of dispatch 7 (§7 of the specification); either order.

## 1 — Publish the authority map first

Before changing any document, publish a map naming, per topic, **one**
authoritative document and the rule that resolves a conflict. Suggested topics:
product scope and claims; security posture; operations; integration; API
reference; lifecycle and governance; release history.

Everything downstream depends on this map, so it is reviewed on its own before
reconciliation begins.

## 2 — Separate normative from historical

Current normative documentation describes what the system is now. Historical RFC
rationale and changelog records describe what was decided when.

**Never rewrite a historical decision to pretend it always matched current
state.** Two dated review-evidence files intentionally reference
`accepted/09{4,6}-*` even though those RFCs are now in `proposed/` — that is
correct and must stay. The same principle governs every RFC in `done/`.

## 3 — Reconcile the public surface

Work through each against implemented behaviour:

- `README.md` — scope claims, feature list, workspace description, MSRV and the
  `rustup` expectation added in M1a, quick start, links;
- `ROADMAP.md` — programme state, milestone structure, what is frozen;
- `docs/` and the mdBook sources — resolve the divergent duplicate operator and
  integrator sets into one authoritative set;
- `docs/src/contributing/architecture.md` — current module layout and the actual
  database access pattern;
- package metadata and source-path references;
- `PUBLISHING.md` placement, per RFC 024's unfinished consolidation.

## 4 — Known contradictions to close

| Location | Contradiction |
|---|---|
| `README.md:82-84` | Says LDAP is not offered; LDAP, federation and dynamic registration are shipped |
| `README.md:181-189` | Omits the i18n crate |
| `docs/src/contributing/architecture.md:23-38` | Moved paths; pre-`Backend` `Arc<Mutex<Connection>>` design |
| `docs/src/contributing/architecture.md:75-79` | Claims every mutation uses `events::emit` — false |
| Root vs mdBook operator/integrator docs | Divergent duplicate sets |
| RFC 024 | Promised consolidation visibly incomplete |

Re-verify each before editing; several may have been overtaken by M1a–M4
implementation work, and a stale finding is as bad as a stale claim.

## 5 — Claims that need particular care

- **Anything describing the project as security-reviewed or production-ready.**
  Nothing before M7 carries that designation, and M7's outcome is a readiness
  *discussion*.
- **The audit chain.** It is tamper-evident within its trust boundary and is not
  evidence against a malicious database writer. Wording that implies otherwise
  is a security-claim defect, not a style issue.
- **Federation, LDAP and dynamic registration.** State what is actually
  supported after M2–M4, not what the original RFCs proposed.
- **MSRV.** 1.95, with the `rustup` expectation — most distribution-packaged
  toolchains are older.

## 6 — Exit

Authoritative documents, README, roadmap, development specification,
operator/integrator guidance, public claims, source paths and lifecycle metadata
all agree; mdBook and the integrity gates pass; and every remaining
known-inaccurate statement has been either corrected or explicitly recorded as a
limitation with an owner.

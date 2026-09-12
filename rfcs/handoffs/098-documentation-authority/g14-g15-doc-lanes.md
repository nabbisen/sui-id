# G14 and G15 — RFC 098's lanes: the uncovered links, and the three authority checks

**Governing RFC.** [RFC 098](../../accepted/098-documentation-authority-reconciliation.md)
§Gate Matrix lanes owned by RFC 098 (added 2026-09-12) and §Design 7. The table
is in the RFC; this dispatch is the manifest entries, one job, one new script
with its policy file and fixtures, and one deferred registration.
**Mechanism.** RFC 094's multi-source lane registry (R10, `153db49`). This is
the second RFC to register lanes; A3.4 reads RFC 098's table under the recorded
heading and requires the manifest to match it byte for byte.
**Authorized.** `@nabbisen`, 2026-09-12 — "proceed on the next item", the queue
having named this lane after G13. **Baseline.** `a33fd7e` or later.
**Implementer.** Mid-capability model.
**Ordering.** Independent of G13 and of RFC 098 dispatch 2 — nothing here
touches their files. Do it as **two commits**: G14 first, G15 second.

## Commit 1 — G14, zero new code

| File | Change |
|---|---|
| `ci/gate-inputs.toml` | `[gate_lane_sources]`: `"098" = "Gate Matrix lanes owned by RFC 098"` — the heading verbatim. `[gate_owners]`: `G14 = "098"`, `G15 = "098"`. `[gates]`: `G14 = "python3.14 scripts/check-markdown-links.py --root . rfcs/handoffs roadmap"`. `[gate_matrix_exceptions]`: `G15 = "<reason>"` — the reason must say it is declared by RFC 098, fails today on F1–F3, and moves to `[gates]` with step 6; check 6 requires G15 to exist in the RFC's table, which it does. `[rust_components]`: follow G10b/G11's convention for script lanes. |
| `.github/workflows/ci.yml` | A `G14` job shaped exactly like G11's — checkout, `setup-python` 3.14 (the SHA is already `setup_python_v7`), `bash scripts/ci-gate.sh G14`. Name: `"G14 — markdown links: handoffs and roadmap (Python 3.14)"`. |

**Evidence.** The A3.4 run *after* adding `"098"` to sources but *before* the
G14/G15 manifest rows: check 3 must fail naming both lanes — the registry
reading RFC 098's table. Then green. `bash scripts/ci-gate.sh G14` on the real
tree: `all links resolve`, exit 0. The A3.2 harness does not cover Python
lanes; G14 inherits `scripts/tests/test_markdown_links.py`, unchanged.

## Commit 2 — G15, built now, registered later

**`scripts/check-doc-authority.py`**, Python 3.14, stdlib only, same
`--root`/`--policy` shape as `check-rfc-integrity.py`, exit 0/1, every failure
on stderr prefixed `check-doc-authority:` and naming the check as `(A)`, `(B)`
or `(C)` and the file:line. Three checks:

- **(A) `SUMMARY.md` completeness, both directions.** Every `*.md` under
  `docs/src/` (excluding `SUMMARY.md`) is the target of some link in
  `SUMMARY.md`; every `SUMMARY.md` link target exists. Compare resolved paths,
  not strings — `./guides/x.md` and `guides/x.md` are the same page.
- **(B) No self-referential absolute URL.** Any Markdown link in a tracked
  `.md` under the scope `README.md ROADMAP.md docs rfcs roadmap` whose target
  begins `https://github.com/nabbisen/sui-id/blob/` fails. Mask fenced code and
  inline code first, the way `check-rfc-integrity.py` does — RFC 098 itself
  quotes the pattern in prose and must not trip its own check.
- **(C) Version-claim freshness.** For each document listed in the policy, find
  its declared version by the policy's regex, parse the workspace version from
  the root `Cargo.toml`, and fail if the lag exceeds the tolerance **unless the
  document contains a staleness banner** matching the policy's banner regex
  within its first N lines. RFC 098 rule 5: the banner is the sanctioned state.
  A document listed in the policy that declares no version at all also fails —
  a pin that vanished is a claim that stopped being checkable.

**`ci/doc-authority.toml`** — D3, nothing but data:

```toml
version = 1
[freshness]
tolerance_minor = 2              # "current as of" may lag the workspace by ≤ this many minor versions
banner_regex = '^> \*\*Stale as of'
banner_within_lines = 40
claim_regex = 'current as of \*\*?v(\d+\.\d+\.\d+)|reflecting the v(\d+\.\d+\.\d+) codebase'
documents = ["docs/threat-model.md", "docs/development-specification.md"]
```

Those are the two pinned documents on the tree today, found by grep; the
tolerance is a starting value for `@nabbisen` to adjust, and the handoff says so
rather than pretending it is derived.

**`scripts/tests/test_doc_authority.py`** — unittest, synthetic temp trees like
`test_rfc_integrity.py`: (A) orphan page fails; (A) entry with no file fails;
(B) absolute self-URL fails; (B) the same URL inside a code span passes; (C)
stale claim without banner fails; (C) stale claim *with* banner passes; (C)
listed document with no claim fails; and one fully valid tree passes. Wire the
test file into whichever job runs `test_rfc_integrity.py` today, the same way.

**Manifest.** G15 is already declared in RFC 098's table and already in
`[gate_matrix_exceptions]` from commit 1. Commit 2 adds **no** manifest change.
Registration — moving G15 from exceptions to `[gates]` and adding its job — is a
later one-line dispatch, after steps 4, 5 and 6 land and the script is green on
`main`. Do not register it now: a red lane on `main` is not a gate.

**Evidence.** `python3.14 scripts/check-doc-authority.py --root . --policy ci/doc-authority.toml`
on the real tree: **exit 1**, reporting exactly eight (A) orphans, three (B)
URLs, and one (C) failure for `development-specification.md` and none for
`threat-model.md`. Report the output verbatim. That red run is the evidence
F1–F3 are real, and the lane's first job is to stay red until they are fixed.
Plus the unittest run, G11, G10b both scopes.

## Stop conditions

- The A3.4 exception-reason rule, check 6, or condition 8 needs any change to
  admit a lane whose owner is not 093 or 094 — stop; that is a registry defect.
- `check-markdown-links.py` needs any change for G14's scope — stop; it is RFC
  093's tool and the scope was measured green.
- A fourth file in commit 1, or a fifth in commit 2.

## After this lands

Steps 4, 5, 6 of RFC 098 §5 clear the tree; then G15 registers. Open question
1 (the MI records) gates step 5 and is `@nabbisen`'s.

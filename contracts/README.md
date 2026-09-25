# `contracts/` — the contracts a gate compares against

These are the **contracts a gate compares against** (RFC 098 D3): policies fed to a
checker, registries that the docs and RFCs cite as a source of truth, and the inputs
the workflow is generated from. **Every file here is read by a gate or a check that
fails when it stops being true** (RFC 116 D2); a file that cannot be given one leaves
this directory and becomes documentation.

**CI is one caller of these files and a developer's `scripts/ci-gate.sh` is the
other.** A lane runs locally by the same command CI runs, so nothing here belongs to
CI alone. (That is the sentence whose absence let this directory be called `ci/`.)

**Formerly `ci/`** (renamed 2026-09-25, RFC 116 stage 7). A path in an older record
that names `ci/<file>` is `contracts/<file>` now; the records keep the path they had
when they were written.

This table is checked by `scripts/check-contracts.py` (lane G18): every file in the
directory has a row, every row names a file that exists, a gate that is in `[gates]`
and in the Gate Matrix, a script that exists, and an RFC that exists. A live file
outside this directory that names `contracts/<file>` must name one that exists, and
one that still names `ci/<file>` is a violation that says where the file moved.

| File | Kind | Read by | Owning RFC |
|---|---|---|---|
| `audit-coverage-matrix.md` | registry | G13, G15 | RFC 094, RFC 098 |
| `contract-paths.toml` | policy | G18 | RFC 116 |
| `doc-authority.toml` | policy | G15 | RFC 098 |
| `gate-inputs.toml` | registry, generator input | `scripts/ci-gate.sh`, `scripts/check-gate-inputs.sh`, `scripts/generate-ci-workflow.py` | RFC 093, RFC 094, RFC 116 |
| `owner-attribution-baseline.txt` | baseline | G16 | RFC 117 |
| `owner-attributions.toml` | policy | G16 | RFC 117 |
| `rfc-policy.toml` | policy | G11 | RFC 093, RFC 110 |
| `ui-invariants.toml` | policy | G12 | RFC 093 |
| `workflow-template.toml` | generator input | `scripts/generate-ci-workflow.py` | RFC 116 |
| `write-commands.toml` | registry | G17 | RFC 094, RFC 116 |

# RFC 093 — Build, Toolchain, and Release-Gate Contract

**Status.** Accepted
**Security review.** Required
**Accepted on.** 2026-07-17
**Approved by.** `@nabbisen`
**Amended on.** 2026-07-28 (second) — added Gate Matrix lane **G07b** (stable, default features, clippy). G07 runs `--all-features` only, so code behind `#[cfg(not(feature = "…"))]` is never linted by it; the workspace has one such site (`crates/sui-id/src/runtime/startup.rs`, the LDAP-absent branch). Without G07b, retiring the legacy `fmt-clippy` job in A3.1 would lose lint coverage on that code. This amendment is purely additive — it adds a lane and weakens nothing — and is applied in place under the same owner ruling that governed the MSRV amendment, because returning this RFC to `proposed/` mid-implementation would halt M1a. Flagged for owner confirmation.
**Amended on.** 2026-07-28 — MSRV raised from 1.91 to 1.95 under this RFC's own change-control clause (Requirements item 2), and M1 split into M1a (build baseline) and M1b (documentation and lifecycle gates). Approved by `@nabbisen`, who ruled that amendment in place is correct here because this RFC self-provides for an MSRV change and returning it to `proposed/` would invalidate the committed implementation-start authorization.
**Independent design review.** `codex-independent-architecture-security-reviewer` (OpenAI Codex), [Accept with notes](../reviews/093-design-review-2026-07-17.md)
**Implementation owner.** `codex-developer` (OpenAI Codex), confirmed by `@nabbisen`
**Design prerequisites.** M0 lifecycle governance and remediation roadmap approved.
**Implementation prerequisites.** `@nabbisen` authorized `codex-developer` against clean baseline `959f089983ce51e53ca403a422a1fe308c276036` with no competing owner for this RFC's `Touches` scope; the repository record is [`implementation-start-authorization.md`](../handoffs/093-build-toolchain-release-gates/implementation-start-authorization.md). Implementation remains prohibited until that record is committed; closure still requires the complete matrix below on one later clean commit.
**Closure prerequisites.** M1a closes when G01–G09 pass on one clean commit; M1b closes when G10–G12 pass on one clean commit. Every mandatory lane in Gate Matrix v1 passes; LDAP smoke and mdBook pass; RFC integrity reports no known debt; independent closure review confirms that the legacy audit diagnostic is not represented as structural assurance.
**Tracks.** ROADMAP M1a — Trustworthy build baseline; M1b — Documentation and lifecycle gates.
**Handoff.** [`../handoffs/093-build-toolchain-release-gates/README.md`](../handoffs/093-build-toolchain-release-gates/README.md)
**Touches.** `Cargo.toml`, crate manifests, `Cargo.lock`, `.github/workflows/`, `scripts/`, `docs/book.toml`, `rfcs/README.md`, RFC metadata and links.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

## Summary

Replace the current single moving-stable CI baseline with an explicit,
versioned release-gate contract. The contract tests the declared Rust 1.95
MSRV and the current stable channel, covers default and all features, exercises
the LDAP path, builds the mdBook, and adds a narrow RFC lifecycle-integrity
gate. It also keeps the existing literal audit-matrix script only as a
diagnostic until RFC 094 activates structural audit assurance.

This RFC changes assurance machinery, not product behavior. It lands first so
later remediation evidence has a trustworthy and repeatable foundation.

## Background

The workspace originally declared `rust-version = "1.91"`, but CI installs only
the moving stable channel, so current CI cannot demonstrate MSRV compatibility.
Implementing this gate contract established that the declared MSRV was not
merely untested but unachievable: `libsqlite3-sys 0.38.1` uses `cfg_select!`,
unstable until 1.95, so 1.91 through 1.94 cannot build the workspace at all.
The MSRV was corrected to 1.95 by the 2026-07-28 amendment; this is a
representative example of why an unexercised gate is not assurance. CI also
builds only default features; the optional `ldap` feature is therefore outside
the blocking build contract. The mdBook and RFC lifecycle invariants are not
blocking.

`scripts/check-audit-matrix.sh` compares event-name strings between Markdown
and Rust. It does not prove that a mutation emits an event, that the event is
typed, or that mutation and append share a transaction. This RFC may wire that
script as a visible diagnostic, but RFC 094 owns the authoritative replacement.

## Requirements

1. A checked-in Gate Matrix v1 defines every mandatory lane, exact command,
   toolchain policy, feature set, trigger, and failure semantics.
2. Rust 1.95 is the MSRV lane. It was raised from 1.91 on 2026-07-28 because
   `libsqlite3-sys 0.38.1`, reached through `rusqlite 0.40` on the `bundled`
   path this project uses, calls `cfg_select!` in its build script. That macro
   is an unstable library feature (E0658, rust-lang#115585) until 1.95, so no
   release in 1.91–1.94 could build this workspace and the previously declared
   MSRV was unachievable rather than merely optimistic. Measured on this
   workspace, 1.91, 1.92, 1.93 and 1.94 all fail and 1.95 builds. Raising it
   again requires an RFC or an amendment to this RFC, manifest updates, release
   notes, and owner approval. The MSRV is a public claim in `README.md` and
   must be changed there in the same commit. Because 1.95 is newer than most
   distribution-packaged toolchains, `README.md` must also state that building
   from source expects a `rustup`-managed toolchain.
3. The latest-stable lane resolves the stable channel at run time and records
   the exact `rustc -Vv` and `cargo -V` output in evidence. Stable-only lint
   drift may be fixed without raising MSRV; it may not be silenced globally or
   converted to an allowlist merely to preserve a date.
4. Default-feature and all-feature compilation and tests are blocking. The
   all-feature lane must compile the LDAP implementation, not merely its API.
5. A deterministic LDAP smoke test must exercise rustls provider installation,
   connector construction, and a connection attempt against a local test
   endpoint. Expected authentication/connection rejection is acceptable only
   after the TLS/provider path was reached and asserted.
6. `cargo fmt`, stable `cargo clippy`, mdBook, and narrow RFC integrity are
   blocking.
7. The RFC gate checks folder/status/index/link/metadata consistency. It has no
   indefinite baseline allowlist and distinguishes prospective metadata rules
   from historical RFCs without fabricating history.
8. The legacy audit literal check is labelled diagnostic-only everywhere. Its
   pass result must say that it proves neither completeness nor atomicity.
9. A release or milestone claim cites one clean commit and observed evidence
   from all applicable mandatory lanes. Results from different commits cannot
   be assembled into a passing matrix.

## Gate Matrix v1

The implementation stores this table in a machine-readable or mechanically
parsed repository file under `ci/`, and keeps this RFC as its normative design.
`--locked` is mandatory for every Cargo command.

| ID | Toolchain | Features | Blocking command / assertion |
|---|---|---|---|
| G01 | 1.95 | default | `cargo +1.95 build --workspace --all-targets --locked` |
| G02 | 1.95 | default | `cargo +1.95 test --workspace --locked` |
| G03 | 1.95 | all | `cargo +1.95 build --workspace --all-targets --all-features --locked` |
| G04 | 1.95 | all | `cargo +1.95 test --workspace --all-features --locked` |
| G05 | stable | default | `cargo +stable build --workspace --all-targets --locked` and `cargo +stable test --workspace --locked` |
| G06 | stable | all | `cargo +stable build --workspace --all-targets --all-features --locked` and `cargo +stable test --workspace --all-features --locked` |
| G07 | stable | all | `cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| G07b | stable | default | `cargo +stable clippy --workspace --all-targets --locked -- -D warnings` |
| G08 | stable | n/a | `cargo +stable fmt --all -- --check` |
| G09a | 1.95 | LDAP | `cargo +1.95 test -p sui-id-store --features ldap --test ldap_smoke --locked -- --exact rustls_provider_and_tls_connector_reach_fixture` |
| G09b | stable | LDAP | `cargo +stable test -p sui-id-store --features ldap --test ldap_smoke --locked -- --exact rustls_provider_and_tls_connector_reach_fixture` |
| G10a | stable / mdBook 0.5.4 | n/a | `mdbook build docs --dest-dir ../target/mdbook-gate` |
| G10b | Python 3.14 | n/a | `python3.14 scripts/check-markdown-links.py --root . README.md ROADMAP.md docs` |
| G11 | Python 3.14 | n/a | `python3.14 scripts/check-rfc-integrity.py --root . --policy ci/rfc-policy.toml` |
| G12 | Bash >=5.2,<6 / repository script | n/a | `bash scripts/check-ui-invariants.sh --all --policy ci/ui-invariants.toml` |

The security audit and fuzz schedules remain separate policies. RFC 099 owns
the complete pre-soak fuzz execution; they are not silently promoted into M1.
Every row above runs on push to `main`, every pull request, and manual dispatch.
No path filter may omit a mandatory row.

### Gate entry points and negative self-tests

The commands in the matrix are the public contract. CI uses
`scripts/ci-gate.sh GNN` as the dispatcher. The literal command for each lane
is stored as data in `ci/gate-inputs.toml` under `[gates]`, which is the
machine-readable expansion of the table above; the dispatcher executes that
recorded command verbatim and may not paraphrase, wrap, or reorder it.
`scripts/check-gate-inputs.sh` verifies that the manifest's command set matches
this RFC's table exactly, so the indirection a dispatcher introduces is a
checked invariant rather than a convention. The following negative
self-tests are also blocking:

| Gate | Fixture command | Required failure |
|---|---|---|
| G09a | `cargo +1.95 test -p sui-id-store --features ldap --test ldap_smoke --locked -- --exact rejects_missing_crypto_provider` | fixture catches provider absence before any network assertion; the positive test reaches the local TLS fixture |
| G09b | `cargo +stable test -p sui-id-store --features ldap --test ldap_smoke --locked -- --exact rejects_missing_crypto_provider` | same assertion under the resolved stable toolchain |
| G10b | `python3.14 -m unittest scripts.tests.test_markdown_links` | missing file, bad anchor, absolute local path, and case-mismatched path fixtures are rejected |
| G11 | `python3.14 -m unittest scripts.tests.test_rfc_integrity` | each RFC invariant listed below has one invalid fixture and one boundary-valid fixture |
| G12 | `bash scripts/tests/check-ui-invariants-fixtures.sh` | one deliberately invalid fixture for each blocking UI invariant causes its sub-check to fail |

G10a is self-tested by building a minimal invalid mdBook fixture whose SUMMARY
references a missing chapter:
`mdbook build scripts/tests/fixtures/mdbook-missing-chapter`. The command must
exit non-zero. **The fixture's own `book.toml` must set
`[build] create-missing = false`.** Verified against mdBook 0.5.4: with the
default `create-missing = true`, a missing chapter is not an error — mdBook
silently writes an empty stub for the referenced path and exits 0, so the
fixture would assert nothing. This applies to the fixture only; `docs/book.toml`
keeps the default so contributors retain the auto-create convenience. G01–G08 use compile-error, failing-test, lint-warning, and
format-drift fixtures maintained under `scripts/tests/fixtures/gate-matrix/`;
`bash scripts/tests/check-gate-matrix-fixtures.sh G01 G02 G03 G04 G05 G06 G07 G08`
must observe the intended lane fail and must itself exit zero only after every
failure was detected.

G12 replaces four inline CI implementations with one reviewed entry point. Its
version-1 policy enumerates exactly these blocking sub-checks:

1. `text-leaks` — reject bare `t.field` Leptos children;
2. `css-tokens-resolve` — every used CSS variable is declared;
3. `semantic-palette-parity` — all four semantic triples exist in all three
   theme roots; and
4. `inline-style-bound` — `pages/` contains at most 20 inline style attributes.

The existing standalone-translation and unused-token reports remain advisory
and are emitted by the script after the four blocking checks. Adding, removing,
renaming, or weakening a G12 sub-check changes `ci/ui-invariants.toml`, advances
its version, and requires owner review; CI does not inherit unnamed jobs.

### Toolchain resolution and evidence

CI installs an exact `1.95` toolchain for the MSRV lanes and `stable` for the
compatibility lanes. The runner is `ubuntu-24.04`, not `ubuntu-latest`.
`ci/gate-inputs.toml` records Gate Matrix version 1, mdBook `0.5.4`, Python
`3.14`, `/usr/bin/bash >=5.2,<6`, the Rust components per lane, and every
GitHub Action by a full
40-hex-character commit SHA. Floating action tags are prohibited. Action SHA,
runner label, Python minor, mdBook version, or required system-package changes
advance the input-manifest revision and receive owner review. Stable Rust is
the sole deliberately moving compiler input; its resolved version is evidence,
not an unreviewed matrix edit.

mdBook installation is exactly
`cargo +stable install mdbook --version 0.5.4 --locked`. Python is installed by
the full-SHA-pinned `actions/setup-python` action with `python-version: '3.14'`.
Rust is installed by a full-SHA-pinned toolchain action. Cache use cannot skip
a command and is keyed by the action/input-manifest revision plus `Cargo.lock`.
Required Ubuntu packages and their resolved versions are printed before the
gate; changing their package names changes the manifest. The job asserts the
Bash constraint before G12. Each job records:

- commit SHA and dirty-tree check;
- operating-system/runner image identifier;
- `rustc -Vv`, `cargo -V`, and relevant tool versions;
- command, exit status, and start/end timestamps.

The stable lane is intentionally moving. A newly released stable compiler that
introduces a warning or regression makes the lane red until corrected or until
an independently reviewed upstream exception is recorded with an expiry. Such
an exception never changes the MSRV.

### Feature and target policy

“Default” means no explicit feature flags. “All” means
`--all-features` over the workspace. **Both feature sets are linted:** an
`--all-features` clippy run never sees code behind `#[cfg(not(feature = "…"))]`,
so G07 (all) and G07b (default) are both mandatory and neither subsumes the
other. `--all-targets` covers libraries,
binaries, examples, benches, and test targets at compile time. Tests use UTC.
No package may opt out without an RFC amendment explaining why its omission
does not weaken the release claim.

### LDAP smoke contract

The smoke fixture is local and contains no real directory credentials. It must:

1. install/select the process rustls crypto provider exactly as production
   startup does;
2. construct the configured LDAP connector with `tls-rustls`;
3. connect to a loopback fixture or controlled listener;
4. demonstrate that the TLS/provider path executes without the provider panic
   or feature-compilation failure that motivated M1;
5. fail on unexpected plaintext downgrade or an unclassified error.

A compile-only test is insufficient. A public LDAP service is forbidden for
the blocking lane.

### RFC integrity contract

`python3.14 scripts/check-rfc-integrity.py --root . --policy
ci/rfc-policy.toml` checks all Markdown RFCs and `rfcs/README.md`:

- one unique RFC identifier across lifecycle folders;
- folder and `Status` agreement;
- every RFC indexed exactly once at a resolvable relative path;
- every relative Markdown link in RFCs resolves;
- no RFC file exists directly under `rfcs/`;
- every standard numeric RFC with identifier 093 or greater has Security
  review, three prerequisite fields, tracks, touches, and accountable role
  fields, regardless of its Git history or current folder;
- the existing `RFC-MI-*` set is enumerated as historical in
  `ci/rfc-policy.toml`; any MI identifier not in that closed historical list is
  prospective and requires the current metadata fields;
- every pre-existing Proposed RFC gains current metadata before acceptance;
- Accepted RFCs have acceptance metadata and, when security review is
  required, an identifiable independent reviewer plus durable reference;
- Done security-sensitive RFCs created from 093 onward have dated independent
  closure metadata and a resolvable repository-relative evidence link;
- historical Done/Archive RFCs are checked for number/folder/status/index/link
  integrity but are not required to invent retrospective reviewers.

The parser recognizes the bold, period-terminated metadata labels from the
README template only in the RFC header before the first level-2 heading. An
ordinary content link is checked for local target/anchor resolution but is not
treated as evidence. `Independent design review` and `Closure evidence` are
the only metadata fields whose targets receive evidence rules: their Markdown
links resolve relative to the RFC, `git ls-files --error-unmatch` confirms the
target is tracked, and `git check-ignore` confirms it is not ignored. Absolute
local paths, ignored `.git-exclude/` paths, missing targets, and external-only
ephemeral references fail.

Known mechanical debt discovered while implementing the gate is repaired in
the same M1 change. The gate does not land with ignored failures.

### Legacy audit diagnostic

If `scripts/check-audit-matrix.sh` is retained in CI, its job and terminal
summary must include:

> Diagnostic only: string parity proves neither emission completeness nor
> mutation/audit atomicity. RFC 094 owns the authoritative structural gate.

Its success is not an M2 or audit-assurance result.

## Multiple implementation steps

1. Add Gate Matrix v1 and split CI into MSRV/stable and default/all-feature
   lanes while retaining existing non-Rust jobs.
2. repair LDAP all-feature compilation and add the local smoke fixture;
3. add mdBook and RFC-integrity jobs, fixing all discovered mechanical debt;
4. relabel and wire the audit diagnostic without expanding its authority;
5. run the complete matrix on one clean commit and assemble closure evidence.

Steps may be separate reviewable commits, but M1 closes only on one commit for
which the complete matrix is green.

## Test plan

- Matrix self-tests: run the exact negative commands in the table above and
  observe each deliberately invalid fixture make its gate fail.
- RFC-gate fixture tests cover duplicate numbers, status mismatch, missing
  index row, broken link, missing prospective metadata, invalid evidence path,
  and a valid historical RFC without invented review metadata.
- LDAP smoke includes provider-not-installed and downgrade-negative cases.
- Run every G01–G12 command and capture exact versions and exit statuses.
- `git diff --check` and a clean-tree assertion close the evidence package.

## Security considerations

Gate weakening is the primary threat. An attacker or hurried maintainer could
hide a vulnerable feature outside default builds, combine passing evidence
from different commits, or present string presence as audit assurance. The
matrix prevents those claims by covering all features, binding evidence to one
commit, and explicitly limiting the legacy diagnostic.

CI actions and tool installers remain part of the supply-chain surface. Their
versions are pinned according to repository policy, and permission scopes stay
minimal. Test fixtures contain no production secrets or network dependencies.

## Rollback

If a new lane is initially unreliable, the change remains Proposed/Accepted
work and M1 stays open. The lane is not made advisory. Reverting the gate after
closure reopens M1 and blocks dependent milestone evidence until the approved
contract is restored or amended.

## Acceptance criteria

- Gate Matrix v1 is represented exactly in tracked CI/configuration.
- G01–G12 pass on one clean commit with recorded tool versions.
- Rust 1.95 and resolved stable both cover default and all features.
- LDAP smoke and mdBook pass as blocking jobs.
- RFC integrity has no known debt or permanent allowlist.
- Audit string parity is visibly diagnostic-only.
- Independent review finds no route to claim a later milestone from partial,
  stale, mixed-commit, or advisory evidence.

## Open questions

None. Toolchain upgrades after acceptance use the change-control rule above.

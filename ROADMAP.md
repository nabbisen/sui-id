# Roadmap

This file is a loose sketch of direction — nothing here is a promise.
Completed work is tracked in [CHANGELOG.md](CHANGELOG.md) and the
[`rfcs/done/`](rfcs/done/) directory.

---

## Active plan — security and release-assurance remediation

**Planning baseline:** v0.76.12, reviewed 2026-07-16.

The current tree is **not approved for a production release, a
"security-reviewed" claim, or a v1/rc/beta designation**. Continued
development is approved only for the remediation programme below. New feature
expansion, multi-tenancy (RFC 025), and alternative SQL backends (RFC 009)
remain frozen until every milestone in this programme has passed its exit
gate.

Milestone windows are set by the accountable owner and are planning aids, not
release promises. They must include design, review, correction, implementation,
and evidence time. A failed gate moves the affected milestone; scope or evidence
is never cut to preserve a date.

As of 2026-07-28 the programme runs **two lanes**: a second implementer is
assigned and the owner has confirmed increased security-review capacity, so
RFC 096 no longer waits behind RFCs 094–095. The overlap conditions are in
*Dependencies and change control* below and remain binding — in particular,
Lane B starts only once its governing RFC is Accepted in its amended form and
the preparatory `handlers/federation.rs` split has landed.

Before any remediation implementation begins, M0 adopts RFC 018's five-folder
variant: `rfcs/accepted/` is the repository-native design-approved and
implementation-eligible state, its files carry `Status: Accepted`, and the RFC
index and integrity checks
recognize it. `proposed/` continues to mean under review and not ready for
implementation. Chat, external boards, and roadmap wording are not approval
records. The identifiers 093–100 are permanently assigned; their proposal files
exist. RFCs 094, 095 and 096 were returned to `proposed/` on 2026-07-28 for
owner-approved material amendments and require fresh independent design review
and re-acceptance before implementation.

### Programme outcomes

The programme is complete only when:

1. the declared MSRV, current-stable, default-feature, and all-feature build
   contracts are explicit and enforced by CI;
2. Class-A mutations and their audit events share one transaction, and the
   coverage gate verifies structure rather than string presence;
3. dynamic client registration validates before use consumption and commits
   token consumption, client creation, and audit atomically;
4. upstream OIDC federation verifies discovery, transport, signatures, and
   required ID-token claims, including a mandatory nonce;
5. the threat model covers every shipped external trust boundary and current
   failure/rollback behaviour;
6. public documentation, RFC state, source layout, and release claims agree;
7. fuzz, packaging, integration, and soak evidence is current and reviewable.

### Milestone schedule

| Milestone | Target window | Theme | Planned RFCs | Exit gate |
|---|---|---|---|---|
| **M0 — Plan, governance, and design freeze** | complete | Approve the roadmap; adopt `accepted/`; assign RFC numbers and ownership | RFCs 093–099 (design only) | Met 2026-07-22 |
| **M1a — Trustworthy build baseline** | **closed 2026-07-30** | Raise MSRV to the verified 1.95 floor; repair `ldap3` rustls provider and mdBook icon; clippy cleanup; wire G01–G09 with negative self-tests; `ci-gate.sh`; complete and mechanically enforce `ci/gate-inputs.toml` | **RFC 093** | G01–G09 pass on one clean commit, hosted, with recorded tool versions; the gate-input manifest is enforced by a tracked check rather than an ad-hoc scan |
| **M1b — Documentation and lifecycle gates** | **exit gate met 2026-08-03** | G10a/G10b/G11 with unittest fixtures; `ci/rfc-policy.toml`; repair RFC folder/status/index/link debt; retire the four legacy inline UI jobs | **RFC 093** | G10–G12 pass hosted; RFC integrity reports no known debt and no permanent allowlist |
| **prep — federation module split** | *owner to set* | **Accountable owner for the split (RFC 096 finding B-096-3).** Split `handlers/federation.rs` into validation and mutation modules; zero behaviour change, evidenced by an observational-equivalence record covering routing, every existing provider operation and every reachable callback outcome including denial paths; independently reviewed by someone other than its implementer, before 096-A or any RFC 094 federation work begins | none (preparatory) | Reviewed and committed; test count unchanged; diff is moves, not rewrites |
| **M2a — Transactional security records (foundation)** | *owner to set* | Typed registry; Class-A transaction seam; typed `ReadConn` with static denial; Class-B `emit_must_attempt`; failure injector; convert user administration, credential/consent/session, token/registration **including C15**, and signing keys | **RFC 094** | Every converted Class-A row has injected-failure rollback and exactly-once evidence; C15 atomic; structural gate passes over converted commands; the coverage matrix states conversion status per command and claims no unconverted command is atomic |
| **M2b — Remaining conversion and authority switch** | *owner to set* | Convert settings, pending settings, federation configuration, client metadata; land the `syn` AST boundary gate; make the structural gate blocking; correct audit-chain claims | **RFC 094** | No production Class-A best-effort append remains anywhere; AST negative fixture rejects an unregistered write; independent adversarial closure review accepts the evidence |
| **M2c — Master-key rotation recovery** | *owner to set* | Rotation journal, atomic key-file publication, startup crash recovery across every transition prefix, old-key custody and disposition | **RFC 100** | Every recovery-table prefix idempotently resumes or cleanly returns to `OldReady`; no prefix yields mixed ciphertext, a missing active key, or an overstated audit record; independent adversarial closure review accepts the crash-injection evidence |
| **M3 — Atomic dynamic registration** | *owner to set* | Validate all metadata before consuming authorization; atomic use/client/audit transaction; race and retry semantics; owner disposition of legacy unstamped registrations | **RFC 095** | Invalid or failed requests consume no use; exactly one concurrent limited use wins; rollback and adversarial evidence pass independent review; **every `legacy_unstamped_dynamic_candidate` has a durably recorded owner disposition** — a "zero candidates" claim is not accepted, because the historical best-effort audit cannot prove completeness |
| **M4-A — Federation validation and transport** | *owner to set* | Discovery and egress/SSRF policy; HTTPS and exact issuer binding; JWKS signature verification; required claim validation; mandatory one-time nonce; cache and key rotation. **No durable mutation.** | **RFC 096** | Substitution, missing/mismatched nonce, issuer/audience/time errors, algorithm confusion, hostile endpoints, oversized responses, and rotating keys all handled as designed |
| **M4-B — Federation mutation and session integration** | *owner to set* | Federation provider/link commands on the Class-A seam; session establishment from a verified assertion | **RFC 096** | Federation mutations carry rollback evidence; representative live integration and independent security review pass |
| **M5 — Threat-model and documentation reconciliation** | *owner to set* | Settle authoritative-document structure; synthesize the current threat model; reconcile README/spec/operator/integrator claims and source paths | **RFCs 097–098** | Threat model covers all shipped boundaries; authoritative docs identified; mdBook and integrity gates pass; public claims match code |
| **M6 — Release-assurance closure and soak entry** | *owner to set* | Runtime file modes; all fuzz targets; package automation and inspection; live LDAP/upstream integration; immutable build/configuration/evidence manifest | **RFC 099** | Full approved clean-tree matrix passes; artifact digest and sanitized configuration recorded; no blocker or high defect remains; independent review approves soak entry only |
| **M7 — Real-environment soak** | *owner to set* | Exercise the exact M6 artifact under representative auth traffic, failure modes, operational cycles, and incident drills | Operational evidence, not a feature RFC | At least four meaningfully exercised weeks and every workload criterion pass; owner and independent reviewer accept the evidence; the earliest outcome is a readiness discussion, never automatic approval or tagging |

Logical review checkpoints may produce internal, versioned source archives,
but no checkpoint before M7 carries a production-ready or security-reviewed
designation. Version numbers for implementation checkpoints are assigned only
after every RFC governing that checkpoint is Accepted; the roadmap does not
reserve semantic versions in advance.

Urgent security fixes, remediation-enabling refactors, dependency/security
maintenance, and repairs required to keep approved gates executable are
controlled exceptions to the feature freeze. A narrowly scoped corrective
v0.x release may be considered when operationally necessary, but it requires
independent gates for the affected surface, accurate limited claims, and does
not inherit M7 confidence. Internal archives and evidence identify the exact
Git tree and artifact digest, not only a human-friendly version label.

### M7 soak workload and reset rules

RFC 099 must turn the following minimum contract into exact event counts,
traffic volumes, thresholds, commands, and evidence formats before M6 closes:

- The soak baseline is one immutable M6 artifact digest plus a sanitized
  configuration/environment manifest. Changing that baseline requires review.
- The environment must sustain representative successful and rejected login,
  MFA, authorization-code, refresh-rotation/reuse, session, registration, and
  federation traffic. Quiet or unavailable periods pause elapsed soak time.
- At least one successful backup/restore, signing-key rotation, master-key
  rotation, restart/upgrade, LDAP outage/recovery, upstream JWKS
  rotation/failure, registration-concurrency exercise, dependency-alert
  handling drill, incident-response drill, and rollback drill is required
  where applicable.
- Exit requires zero unresolved blocker or high-severity defect and
  authentication/security error rates within the thresholds fixed by RFC 099.
- A security-sensitive behaviour change, data migration, authn/authz change,
  or fix for a blocker/high-severity defect restarts the relevant soak window.
  The owner and independent reviewer decide whether a narrower change resets a
  targeted exercise or the complete four-week window.
- Calendar completion never passes M7. The earliest post-soak event is a
  release-readiness discussion, not production approval or a version tag.

### Execution order

Numbered work items in dependency order. Each is one reviewable unit with its
own hash-pinned package. Items at the same number may run concurrently.

| # | Work item | Lane | Gate | Handoff |
|---|---|---|---|---|
| 1 | Corrective fixes: `ldap3` rustls provider, mdBook icon | A | none — owner-authorized | [093 M1a](rfcs/handoffs/093-build-toolchain-release-gates/m1a-implementation.md) |
| 1 | clippy cleanup to zero on stable | A | none | 093 M1a |
| 1 | M1b tooling: G10a lane, G10b/G11 scripts, `ci/rfc-policy.toml`, fixtures, then the A3.4 lane-completeness rule (C2.1) | B | none | [093 M1b](rfcs/handoffs/093-build-toolchain-release-gates/m1b-implementation.md) |
| 2 | MSRV raise to 1.95 | A | item 1 corrective fixes | 093 M1a |
| 3 | Gate lanes G01–G09 + negative fixtures + `ci-gate.sh` + manifest enforcement | A | items 1–2 | 093 M1a |
| 3 | M1b debt repair: RFC status and `rfcs/` broken links — **re-measured by G11, not by the 2026-07-28 counts** | B | item 1 tooling (G11 must exist first) | 093 M1b |
| 4 | **M1a closes** — G01–G09 hosted green on one clean commit | A | item 3 | — |
| 4 | **M1b closes** — G10–G12 hosted green, integrity debt zero | B | item 3 | — |
| 5 | `handlers/federation.rs` split, zero behaviour change | prep | M1a; clippy landed (both met at `0fcb423`); **module boundary confirmed by RFC 096's correction review — not yet** | [prep split](rfcs/handoffs/prep-federation-module-split/README.md) |
| 6 | RFC 094 M2a: registry, seam, `ReadConn`, Class-B emitter, priority conversion incl. C15 | A | M1a; RFC 094 re-accepted | [094](rfcs/handoffs/094-transactional-audit/README.md) |
| 6 | RFC 096-A: discovery, JWKS, claims, mandatory nonce — no durable mutation | B | M1a; item 5; RFC 096 re-accepted | [096](rfcs/handoffs/096-upstream-oidc-federation/README.md) |
| 7 | RFC 095 (M3): validate-first dynamic registration | A | M2a incl. C15; RFC 095 re-accepted | [095](rfcs/handoffs/095-dynamic-client-registration/README.md) |
| 7 | RFC 094 M2b: remaining conversion, AST gate, authority switch | A | M2a | 094 |
| 7 | RFC 100 (M2c): master-key rotation recovery | A/C | M2a; RFC 100 Accepted | [100](rfcs/handoffs/100-master-key-rotation/README.md) |
| 7 | RFC 096-B: federation mutation and session integration | B | M2a seam; 096-A | 096 |
| 8 | RFC 098: documentation authority and reconciliation | — | M1b | [098](rfcs/handoffs/098-documentation-authority/README.md) |
| 9 | RFC 097: current threat model baseline | — | items 6–8 complete; 098 authority decision | [097](rfcs/handoffs/097-threat-model/README.md) |
| 10 | RFC 099 (M6): runtime modes, fuzz, packaging, live integration, evidence manifest | — | 097, 098, and **RFC 100 Implemented** | [099](rfcs/handoffs/099-operational-hardening/m6-implementation.md) |
| 11 | M7 soak against the immutable M6 artifact | — | M6 closure | [099 soak](rfcs/handoffs/099-operational-hardening/m7-soak-operations.md) |

Items 1 and 3 are the only places where Lane A and Lane B both have work with no
dependency between them; everything else in a lane is strictly ordered. Item 5
is deliberately sequenced after the clippy cleanup so a large file move does not
collide with workspace-wide lint churn.

### Planned RFC set and boundaries

| RFC | Working title | Owns | Explicitly does not own | Handoff expectation |
|---|---|---|---|---|
| **093** | Build, Toolchain, and Release-Gate Contract | LDAP crypto provider; MSRV/latest-stable matrix and lint-drift policy; all-feature CI; mdBook; narrow folder/status/index/link repair and gate; legacy audit diagnostic wiring with explicit limitation | Audit completeness/atomicity or broader documentation reconciliation | Versioned gate matrix is mandatory in the RFC; no separate large handoff |
| **094** | Transactional Audit Completeness and Typed Event Registry | Class-A transaction design; event vocabulary; structural coverage; injected-failure tests; audit-chain claim correction | External anchoring service implementation | **Required** multi-file developer handoff and migration checklist |
| **095** | Dynamic Client Registration Transaction and Validation | Validate-first flow; registration-use/client/audit transaction; metadata parity; concurrency/retry tests | Broad RFC 7591 management API expansion | **Required** focused implementation and QA handoff |
| **096** | Upstream OIDC Federation Validation | Discovery/egress/SSRF policy; JOSE/JWKS/signature/claim/nonce validation; cache/rotation; hostile-provider tests | New providers, account-link UX expansion, trusting upstream MFA as local MFA | **Required** security invariants, attack cases, and staged developer handoff |
| **097** | Current Threat Model and Security-Assurance Baseline | All shipped trust boundaries, STRIDE, cross-boundary attacks, SSRF, secret boundaries, rollback/failure analysis, residual risks | Implementation changes owned by RFCs 093–096 | Recommended security-review checklist |
| **098** | Documentation Authority and Reconciliation | Authoritative doc set; README/roadmap/spec/operator/integrator/public-claim and current-path reconciliation; lifecycle metadata/link corrections not completed mechanically in M1 | Rewriting historical RFC decisions | Optional mechanical task checklist |
| **099** | Operational Hardening and Soak Readiness | Runtime file modes; all fuzz targets; package automation/inspection; live integration evidence; soak entry criteria | Declaring production readiness or a v1 date | **Required** operator/tester handoff and evidence manifest |
| **100** | Master-Key Rotation Crash Recovery | Rotation journal; atomic key-file publication; startup crash recovery across every transition prefix; old-key custody and disposition | The Class-A transaction seam it consumes; signing-key rotation (`K01`, RFC 094); any KMS/HSM integration or online rotation | **Required** operator recovery procedure and crash-injection matrix |

### Dependencies and change control

Restructured 2026-07-28 to split M1/M2/M4 and to run federation as an
independent lane. Target windows are re-baselined by the owner; the structure
below fixes only contents and order.

```text
M0 approval mechanism + RFC files/ownership
  └──> RFC 093
         ├── M1a build baseline ──> prep: federation.rs split
         │     ├──> Lane A: RFC 094 M2a ──> RFC 095 (M3)
         │     │                        ├──> RFC 094 M2b
         │     │                        └──> RFC 100 (M2c)
         │     └──> Lane B: RFC 096-A ──> RFC 096-B (needs M2a)
         └── M1b documentation and lifecycle gates   (parallel; no code dependency)

RFC 094 M2b + RFC 095 + RFC 096-B + M1b
  + RFC 098 authority decision ──> RFC 097 final baseline

RFC 097 + RFC 098 reconciliation ──> RFC 099 ──> M6 ──> M7 soak

RFC 100 (M2c) is required before M6 entry: the M7 workload contract includes a
master-key rotation exercise, and RFC 099 requires zero blocker defects.
```

Two lanes run concurrently after M1a. Lane B is authorized by the overlap rule
below: all designs are Accepted or in re-review after owner-approved amendment,
a second implementer is assigned, file ownership is named, and the owner
confirmed increased review capacity on 2026-07-28. `handlers/federation.rs` is
split first because it currently contains both Lane A and Lane B territory.

- RFC 093 lands first because later evidence is not trustworthy until the
  build and gate contract is reliable.
- The existing literal-presence audit script is diagnostic-only in M1 and
  carries an explicit statement that it proves neither completeness nor
  atomicity. RFC 094 owns and activates the authoritative structural gate.
- RFC 093 owns narrow mechanical RFC folder/status/index/link repair and gate
  activation. RFC 098 owns document authority, content reconciliation, public
  claims, and broader current-path cleanup; no indefinite baseline allowlist
  hides known lifecycle debt.
- RFC 095 depends on RFC 094 so dynamic registration uses the new atomic audit
  mechanism instead of introducing a second transaction pattern.
- RFC 097 requires RFCs 093–096 plus RFC 098's authority decision, then
  synthesizes verified current behaviour. Threat-model deltas remain acceptance
  criteria inside RFCs 093–096 and land with their behaviour changes.
- RFCs 094–096 require independent review of their security invariants before
  implementation and observed adversarial/rollback evidence before closure.
- Any newly discovered authentication bypass, privilege escalation, token
  forgery, secret exposure, or irreversible migration risk pauses the schedule
  and receives an explicit roadmap/RFC decision before work resumes.
- Milestone completion requires observed command output and review evidence;
  historical handoff logs do not satisfy a current gate.
- The existing source-size finding is deferred rather than expanded into a
  global refactor. RFCs 094–096 split touched oversized security modules only
  when that materially improves reviewability without unrelated churn.
- RFC 096 runs as an independent lane from RFCs 094–095, with a second
  implementer. Overlap is authorized once each lane's governing RFC is Accepted
  **in its amended form**, shared-file ownership is named — the preparatory
  `handlers/federation.rs` split — and reviewer capacity is confirmed. Capacity
  was confirmed by `@nabbisen` on 2026-07-28. RFCs 094, 095 and 096 are
  presently in re-review following their 2026-07-28 amendments, so **Lane B
  begins on re-acceptance, not before**. RFC 095 never precedes RFC 094 M2a;
  RFCs 097–099 wait for both lanes.

---

### Traceability

Required by the multi-agent framework (roadmap item → RFC → Handoff →
implementation → evidence). Status column is **verified fact** as of 2026-07-29.

| Milestone | RFC | Handoff | Implementation | Evidence |
|---|---|---|---|---|
| **M1a — CLOSED 2026-07-30** | 093 (Accepted) | `handoffs/093-…/m1a-implementation.md` | A0, A1, A2, A3.1–A3.5 and the C1/R9 corrections all landed | **Hosted run `30546346612` on commit `474d0f2`: 18/18 jobs success, zero skipped** |
| **M1b — EXIT GATE MET 2026-08-03** | 093 (Accepted) | `handoffs/093-…/m1b-implementation.md` | C0, C1, C2, C2.1, C3, C4, C5 all closed | **Run 30754447964 on `1d58da2`: 17/17 jobs success, four legacy UI jobs retired, G12 advisory counts unchanged (3 / 18), RFC integrity debt zero with no allowlist.** **RFC 093 remains Accepted** — closure needs independent closure review + closure metadata |
| prep | none (preparatory) | `m1a-implementation.md` §Theme B | Not started | — |
| M2a / M2b | 094 (Proposed) | `handoffs/094-transactional-audit/` | Blocked on re-acceptance | — |
| M2c | 100 (Proposed) | `handoffs/100-master-key-rotation/` | Blocked on 094 M2a | — |
| M3 | 095 (Proposed) | `handoffs/095-dynamic-client-registration/` | Blocked on 094 M2a | — |
| M4-A / M4-B | 096 (Proposed) | `handoffs/096-upstream-oidc-federation/` | Blocked on re-acceptance | — |
| M5 | 097, 098 (Proposed) | `handoffs/097-threat-model/`, `handoffs/098-documentation-authority/` | Blocked | — |
| M6 / M7 | 099 (Proposed) | `handoffs/099-operational-hardening/` | Blocked | — |

Every remediation RFC has a Handoff. No implementation task exists without a
governing RFC and Handoff.

### Risk register

Required format: description, likelihood, impact, detection, mitigation,
residual risk, decision owner. **Decision owner `@nabbisen` means the risk
cannot be closed by the architect alone.**

**Deployment status, confirmed by `@nabbisen` on 2026-08-18: sui-id is not in
production use, and no deployment by any third party is known.** This is a
material input to every impact rating below and to how the open security defects
should be described.

It means the unfixed defects — the federation ID-token signature bypass above all —
have **no known exposed users**. They are real defects in published code, not an
active compromise. The correct phrasing is "no known deployment", not "no
consequence": the repository is public and carries 127 signed release tags, so it
is distributed even though the owner runs nothing.

The practical effect is that **waiting for a properly independent reviewer costs
little**. Schedule pressure must not be used as an argument for accepting a
weaker review. If the deployment status changes, this paragraph must change with
it, and the impact ratings below should be re-read.

| ID | Risk | Likelihood | Impact | Detection | Mitigation | Residual | Owner |
|---|---|---|---|---|---|---|---|
| R1 | Review independence: authoring, implementation and review share one vendor | Certain (structural) | High — the readiness claim rests on it | Role metadata inspection | Two-tier ruling of 2026-07-28: vendor independence required for RFC 094, 096, 097 and M6 closure; role independence elsewhere | Role-independence-only reviews outside the named set; RFC 100 not in the set although its failure mode is severe | `@nabbisen` |
| R2 | Audit hash chain is unkeyed and unanchored — tamper-evident only within its trust boundary | Certain (by design) | High if misrepresented; low if stated | Documentation review | RFC 094 corrects the claims; no external anchor is introduced | Accepted permanently for this programme; revisit on a non-repudiation requirement or an untrusted-DB-writer deployment | `@nabbisen` |
| R3 | Source-size debt: 26 files over 500 lines, incl. load-bearing security modules | Certain (measured) | Medium — raises review cost and change-collision risk | `find`/`wc` sweep | No new file over 500 ELOC; split-when-touched-if-it-helps; **M5 revisit** | Residue unresolved until the M5 decision | `@nabbisen` at M5 |
| R4 | MSRV 1.95 leaves ~2 releases of headroom below current stable | Certain (measured) | Medium — operators need `rustup`, not distro Rust | Toolchain bisect (done) | README states the `rustup` expectation | Narrow support window accepted when the floor was approved | `@nabbisen` |
| R5 | **No hosted CI run has ever occurred.** All gate evidence to date is local | **Retired 2026-07-29** | — | — | First hosted run executed on `1e59e3d` | Superseded by R9 | — |
| R9 | A3.2's self-test harness passed vacuously in CI: `ci-gate.sh`'s HEAD/`GITHUB_SHA` assertion fired inside staged fixture repos | **Closed 2026-07-30** | — | First hosted run `1e59e3d`; caught at the first `expect_gate_passes` | `GITHUB_SHA` bound per staged fixture, and `expect_gate_fails` now requires `exit_status=` present and no `::error::ci-gate` | None — verified green hosted on `474d0f2`, 38 assertions, 0 precondition errors | closed |
| R6 | `[gates]` ↔ RFC 093 matrix correspondence is unenforced until A3.4 lands | Likely while A3.4 is outstanding | Medium — dispatcher could drift from the RFC undetected | Manual comparison only (done once, 2026-07-28) | A3.4 `check-gate-inputs.sh` condition 7 | Unguarded until A3.4 | architect (tracked) |
| R7 | Pre-RFC-094 dynamic registrations are indistinguishable from admin-created clients | Certain (historical) | Medium — provenance cannot be proven | RFC 095 migration report | Owner disposition of every candidate before M3 closure; "zero candidates" is not an accepted claim | Permanent data scar; requires manual adjudication | `@nabbisen` at M3 |
| R8 | Two-lane execution raises review load on a single reviewer | Likely once Lane B starts | Medium — review becomes the bottleneck, not implementation | Review turnaround time | Owner confirmed increased review capacity 2026-07-28 | Unproven until both lanes run concurrently | `@nabbisen` |

### Standing programme risks — detail for R1–R3

The register above is the index. This section holds the reasoning, the owner
rulings, and the revisit triggers for the three risks that are **not owned by
any RFC** and would otherwise be lost. `S1`/`S2`/`S3` below correspond to
`R1`/`R2`/`R3`. None blocks M1a.

#### S1 — Review independence is undefined, and the roles share one vendor

`codex-project-architect`, `codex-developer`, and
`codex-independent-architecture-security-reviewer` are all agent identities from
one vendor, with `@nabbisen` as sole human owner and approver. RFC metadata
labels reviews "independent", and RFC 093's integrity contract requires "an
identifiable independent reviewer" for Accepted security-sensitive RFCs — but
**no document defines what independence means**, and no rule prevents the same
lineage authoring, implementing, and approving the same change.

For a product whose entire value is trust, this is load-bearing: the eventual
readiness discussion after M7 rests on it.

**Owner ruling, 2026-07-28 — decided.** Two tiers of independence apply:

- **Role independence** (minimum, in force for every artifact): the reviewer of
  an artifact did not author it, implement it, or previously approve it. Where
  this is violated the review must say so plainly and substitute adversarial
  testing for the independence it cannot claim.
- **Vendor independence** (required for the set below): at least one reviewer of
  record outside the vendor that authored and implemented the change.

Vendor independence is **required** for:

| Artifact | Why |
|---|---|
| RFC 094 — transactional audit seam | 62 Class-A commands; the whole audit guarantee rests on it |
| RFC 096 — federation validation | Fixes ID tokens currently accepted without signature verification |
| RFC 097 — threat model baseline | The document every security claim is read against |
| M6 closure (RFC 099) | The gate that authorizes soak entry |

Role independence alone is sufficient elsewhere, including RFC 095, whose
2026-07-28 amendment is a single prerequisite re-point rather than a design
change.

Consequence for the current critical path: RFCs 094 and 096 cannot be
re-accepted on a same-vendor review alone, and their re-review is what currently
blocks M2a and 096-A. Arranging that reviewer is the immediate next action.

**RFC 100 is a candidate the owner may wish to add.** It was not in the ruled
set, and role independence therefore applies — its author must not review it.
Flagged because its failure mode, an unrecoverable key/database divergence, is
as severe as anything in the ruled set; the architect under-weighted it when
proposing the set. Adding it is an owner call, not an assumption.

#### S2 — The audit hash chain has no external anchor, permanently for this programme

RFC 094 corrects the *claims* about the chain but explicitly introduces no
external anchoring or notarization service. The chain is therefore
tamper-evident **within its trust boundary only**: a writer with database access
who knows the public algorithm can edit rows and recompute the affected suffix,
and a tail-only verifier cannot detect an altered old row outside its window.

This is a deliberate scope decision, not an oversight. It becomes a defect only
if documentation implies otherwise — which RFC 098 must check and RFC 097 must
carry as residual risk with a named accepter.

**Revisit if** a compliance requirement demands non-repudiation, or a deployment
model appears in which the database writer is not already fully trusted. Either
would need its own RFC; neither is in this programme.

#### S3 — Source-size debt is deferred without a trigger

**26 files exceed 500 physical lines**, unchanged since 2026-07-16. The project
rule strongly recommends splitting above 500 ELOC. Several are legitimately large
translation tables (`sui-id-i18n` locale and string files, 890–910 lines), but the
rest are load-bearing security and handler modules: `handlers/oidc.rs` (886),
`oidc/authorize.rs` (817), `cli.rs` (787), `handlers/federation.rs` (790),
`authn/step_up.rs` (769), `handlers/settings.rs` (760), `repos/users.rs` (724),
`handlers.rs` (718), `authn/session.rs` (717), `store/models.rs` (708).

Large security modules raise review cost and change-collision risk — exactly the
risk that made the `federation.rs` split a prerequisite for two-lane work.

Standing rules, effective now:

1. **No new file over 500 ELOC** is introduced by any programme RFC.
2. RFCs 094–096 split an oversized module they touch **when that materially
   improves reviewability**, and not otherwise — no unrelated churn.
3. **Revisit point: M5.** At M5, review the remaining list and decide whether the
   residue needs its own RFC or is accepted with translation tables excluded.
   Deferral without a decision point is how this became debt in the first place.

## Current status

**Security-assurance arc — RFCs 078–086 (v0.63.2).** Created by
the architect audit
(`docs/security-assurance-audit-v0.63.1.md`). Recommended
sequencing — each step independently shippable:

| Step | RFC | Theme | Suggested release |
|---|---|---|---|
| ✅ 1 | [078](rfcs/done/078-security-type-modeling-baseline.md) | Type modeling baseline (newtypes, secret redaction) | v0.64.0 |
| ✅ 2 | [080](rfcs/done/080-refresh-rotation-atomicity.md) | Refresh rotation atomicity + reuse detection | v0.66.0 |
| ✅ 3 | [079](rfcs/done/079-authorization-code-lifecycle-assurance.md) | Auth-code single-use by statement | v0.66.0 |
| ✅ 4 | [081](rfcs/done/081-actor-scope-boundary.md) | Actor scope boundary | v0.67.0 |
| ✅ 5 | [082](rfcs/done/082-authorization-decision-core.md) | Pure authorization core | v0.67.0 |
| ✅ 6 | [083](rfcs/done/083-security-state-machine-testing.md) | State-machine proptest harness | v0.68.0 |
| ✅ 7 | [085](rfcs/done/085-audit-event-completeness.md) | Audit completeness + atomicity | v0.68.0 |
| ✅ 8 | [084](rfcs/done/084-fuzzing-untrusted-input-boundaries.md) | Fuzzing harness | v0.69.0 |
| ✅ 9 | [086](rfcs/done/086-formal-model-checking-pilot.md) | Kani / TLA+ / Flux pilots (time-boxed) | evaluation only |

**Auth-core assurance arc (RFCs 078–086): COMPLETE as of v0.69.0.**

**Note — v0.65.0 took the token-foundation slot.** v0.65.0 shipped the
UI-security handoff's Unit 1 (WCAG AA contrast + explicit disabled tokens
+ a contrast CI test; see CHANGELOG). The auth-core suggested releases
above therefore shift forward by one (079 / 080 → v0.66.0, 081 / 082 →
v0.67.0, 083 / 085 → v0.68.0, 084 → v0.69.0); 086 stays evaluation-only.
Targets remain indicative, not commitments.

**UI-security contract — handoff units 1–6. ✅ COMPLETE as of v0.74.0.** The approved v2.3 UI/UX
contract defines six units. Unit 1 shipped in v0.65.0; Units 2–6 shipped
as RFCs 088–092 through v0.74.0. They build on the completed auth-core
primitives: the actor-scope boundary (081), authorization-decision core
(082), and audit completeness work (085).

The `ThemeToggle` contract landed in RFC 092 (v0.74.0): blocking
`theme-init.js`, `no-js` / `js` root-class swap, and a `<noscript>`
fallback.

**Mockup Integration epic — sixteen RFCs, Phase 0 → Phase 8.**
Introduced in v0.49.0. The full epic table and reading order live
in [`rfcs/README.md`](rfcs/README.md) ("Proposed — Mockup
Integration epic"); see also
[`docs/mockup-integration/`](docs/mockup-integration/).

| RFC | Title | Priority | Notes |
|---|---|---|---|
| [RFC 004](rfcs/done/004-federation.md) | OIDC federation (upstream IdP) | Shipped | Upstream OIDC relying-party federation |
| [RFC 005](rfcs/done/005-pluggable-user-backends.md) | Pluggable user backends | Shipped | LDAP directory integration |
| [RFC 006](rfcs/done/006-metrics.md) | Metrics and observability | Shipped | Prometheus endpoint |
| [RFC 008](rfcs/done/008-third-party-posture.md) | Third-party posture / consent screen | Shipped | Dynamic registration and explicit consent for external RPs |
| [RFC 009](rfcs/proposed/009-sql-backends.md) | Alternative SQL backends | Low | PostgreSQL / MySQL support |
| [RFC 025](rfcs/proposed/025-multi-tenant-expansion.md) | Multi-tenant expansion | Low | Per-tenant namespaces (post-1.0) |

---

## Historical UI/UX hardening plan

The v0.42 → v0.48 UI/UX hardening plan is complete. Six phases (A–F)
addressed correctness gaps surfaced during a v0.41.0 implementation
review: the rendered UI was not matching the design contract the v0.40
handoff claimed had been met.

| Phase | Version  | Theme                                              | RFCs (planned)       |
|-------|----------|----------------------------------------------------|----------------------|
| **A** | v0.42.0  | Stop the bleeding (this release)                   | 048, 049, 050        |
| **B** | v0.43.0  | i18n completeness sweep                            | 051, 052, 053, 054   |
| **C** | v0.44.0  | Self-service unification (`/me/security/*`)        | 055, 056, 057        |
| **D** | v0.45.0  | Dangerous operations contract                      | 058, 059, 060        |
| **E** | v0.46.0  | Visual hierarchy + palette extension               | 061, 062, 063, 064   |
| **F** | v0.47.0  | Code structure (split `pages.rs` and admin.rs)     | 065, 066, 067        |
| —     | v0.48.0  | Buffer + RFC index / docs reconciliation           | 068, 069             |

The plan was intentionally correctness-first: visible polish (Phase E)
landed fifth, only after the underlying i18n, navigation, and
dangerous-operation contracts were honest. See
[`docs/src/contributing/`](docs/src/contributing/) and the individual
proposed RFCs once they enter the repository at each phase start.

---

## Mockup Integration arc (v0.49.0 → )

Following Phases A–F, the project enters the **Mockup Integration
("MI") arc**: a controlled migration that adopts the
`sui-id-web-mockup-v0.4.8` UI/UX language into the product. Eight
phases (0 → 8), each backed by one or more `RFC-MI-NNN` documents
in `rfcs/proposed/`. v0.49.0 opens the arc with the Phase 0 planning
artifacts only — **no runtime code changes** — so subsequent
implementation work has an auditable baseline. See
[`docs/mockup-integration/`](docs/mockup-integration/) for the
migration plan, codebase handoff, and mockup handoff package.

| Phase | Target version | Theme                                                   | RFCs                          |
|-------|----------------|---------------------------------------------------------|-------------------------------|
| **0** | v0.49.0–0.49.1 | Planning + baseline inventory                           | RFC-MI-000 → `done/`          |
| **1** | v0.50.0–0.50.1 | CSS sharding; token mapping; theme decision             | RFC-MI-010–012 → `done/`      |
| **2** | v0.51.0–0.51.1 | Shell decision; CSRF; route-based tabs                  | RFC-MI-020–022 → `done/`      |
| **3** | v0.52.0        | Dashboard + audit read-only screens                     | RFC-MI-030, 031 → `done/`     |
| **4** | v0.53.0–0.53.1 | Auth surfaces + setup wizard                            | RFC-MI-041, 040 → `done/`     |
| **5** | v0.54.0        | Form system + danger zone                               | RFC-MI-050, 051 → `done/`     |
| **6** | v0.55.0        | Self-service `/me/security/*`                           | RFC-MI-060 → `done/`          |
| **7** | **v0.56.0**    | **OIDC consent UX (this release; Phase 7 complete)**    | RFC-MI-070 → `done/`          |
| **8** | v0.57.0        | Responsive + a11y hardening (MI arc done)               | RFC-MI-080 → `done/`          |
| —     | v0.57.1        | Dependency refresh: rand 0.10 + reqwest                 | RFC 069, 070 → `done/`        |
| —     | v0.58.0        | Dashboard action items                                  | RFC 073 → `done/`             |
| —     | v0.59.0        | Auditor role                                            | RFC 071 → `done/`             |
| —     | v0.60.0        | End-user app-access surface| **End-user app-access surface (this release)**          | RFC 072 → `done/`             |

Phase-1 blockers (`D-01` / `D-02` / `D-03` in the migration plan)
must be resolved before any code-level visual replacement starts:
component CSS sharding, path-based-tab preservation, and CSRF
threaded through `Shell` server-side. Target versions above are
indicative — not commitments. v1.0 designation continues to be
deferred (verification phase, spec §22).

---

## Completed (recent)

| Version | What shipped |
|---|---|
| v0.76.8 | **Workspace dep consolidation.** Sibling crate deps moved to `[workspace.dependencies]` in root Cargo.toml; each crate now uses `{ workspace = true }`. Version bumps now require changing one file only. **141/141 tests; all crates at 0.76.8.** |
| v0.76.7 | **Doc verification pass.** Spec/impl divergences corrected: RFC 004 amr claim corrected to 'fed' (not 'fed:{slug}'), held-state note added; RFC 008 consent_policy values corrected ('none'/'first_time'/'always'), scope_definition future-work note added. oidc-api.md: federation amr rows, 'not yet' list updated. upgrade.md: v0.76.x migration table + new routes + new CLI. operators.md: LDAP/federation/metrics/dynamic-reg operator sections. overview.md: LDAP/dynamic-reg/federation moved from 'Not in scope' to 'Supported'. **141/141 tests; 0 clippy; all 5 CI gates PASS.** |
| v0.76.6 | **Full security audit of all 104 done RFCs.** Flaw 1 (Med): RFC 004 federation state_param not verified in callback — CSRF gap; fixed by storing upstream_state in FedState + ct_eq check in callback. Flaw 2 (High): admin password reset could set local credential on LDAP shadow user, bypassing LDAP auth; fixed by source check in reset_user_password. Confirmed clean: RFCs 010/019/026/038/058/071/077/088/089/001/003/006/069/070/090/020/008/005. **141/141 tests; 0 clippy; all 5 CI gates PASS.** |
| v0.76.5 | **Security audit + doc pass.** P3 email_verified guard logic clarified; state cookie cleared on error paths; audit-doc §7/§9 updated to ✅ Shipped for RFCs 079–086; audit-events.md: federation/dynamic-reg/user-source events; configuration.md: user_source/federation_provider/metrics/new CLI cmds; oidc-api.md: POST /oauth2/register + federation sections; 'not yet' list updated. **141/141 tests; 0 clippy; all 5 CI gates PASS.** |
| v0.76.4 | **RFC 004 (federation — upstream OIDC RP).** FederationProviderId; migrations 0037 (federation_provider, encrypted secret) + 0038 (federation_link, UNIQUE (provider_id, upstream_sub) P1); ProvisionMode/FederationProviderRow/FederationLinkRow; federation_provider + federation_link repos (6 tests); MasterKey::as_bytes(); AppState.http_client; FederationProviderConfig (env-indirected secret); startup sync; handlers: /start (PKCE+state-cookie P5), /callback (code exchange, nonce P5, email-collision P2, provision P3, MFA P4, AuthMethod::Fed session), /link skeleton; audit-matrix 50×50. **141/141 tests; 0 clippy; all 5 CI gates PASS.** |
| v0.76.3 | **RFC 008 (third-party-posture bundle).** Migration 0035 (clients app-identity cols), migration 0036 (scope_definition seeded + client_registration_token); RegistrationSource/RegistrationTokenId; scope_definition + client_registration_token repos (8 tests); POST /oauth2/register (RFC 7591, P4/P5/P6); CLI issue-registration-token; consent screen with logo/homepage (P6); 7 i18n strings ×3 locales; audit-matrix 46×46. **135/135 tests; 0 clippy; all 5 CI gates PASS.** |
| v0.76.2 | **RFC 009 Step 1 — `Backend` trait + `SqliteBackend`.** `sui-id-store::backend` module; `Backend` trait (dyn-compatible via type erasure); `SqliteBackend` (`Arc<Mutex<Connection>>`); `Database` rewrapped as `Arc<dyn Backend>`; public API identical, zero call-site changes; 5 backend tests. **127/127 tests; 0 clippy; all 5 CI gates PASS.** |
| v0.76.1 | **RFC 005 (pluggable user backends — LDAP).** `UserSource` trait + `InMemoryUserSource` + `LdapUserSource` (ldap3, `ldap` feature flag); RFC 4515 `escape_filter_value` (P1); migration 0034 (`users.source` + `users.external_stable_id` + partial unique index); `upsert_ldap_shadow`; `AuthMethod::Fed`; `UserSourceConfig` (env-indirected bind secret, P2); `try_login_with_cascade` in `auth.rs`; 10 new store tests; 45×45 audit matrix. **122/122 tests; all 5 CI gates PASS.** |
| v0.76.0 | **RFC 006 (Prometheus metrics endpoint).** `sui_id_store::metrics` module with 10 counters, 3 gauges, 2 histograms; global handle; `AppState.metrics`; call-site hooks (audit::append, email_outbox::enqueue, login outcomes, token issuance); `GET /metrics` (auth-gated, disabled-by-default — P5); migration 0033 (`metrics_token_hash`); `admin rotate-metrics-token` CLI; `metrics_enabled` config. **112/112 tests; all 5 CI gates PASS.** |
| v0.75.1 | **Docs: detailed write-ups for the six proposed post-1.0 RFCs** (004 federation, 005 LDAP user-source, 006 metrics, 008 third-party posture, 009 SQL backends, 025 multi-tenant). Each expanded into the canonical 15-section structure. All remain `Proposed`, no schedule, require owner direction + soak. **No code change; 107/107 tests; all 5 CI gates PASS.** |
| v0.75.0 | **Codebase audit pass.** RFC 088 gap fixed: 403 page now uses `error_403_auditor_title/body` (was showing generic copy). Audit-matrix gate: `auth.login.password_ok_mfa_required` added to matrix doc (43×43 bidirectional). ROADMAP stale RFC 085 link fixed. 10 dead i18n keys removed (old `*_empty` names, old `error_4xx/5xx` names, `signing_keys_rotate_warning`). `table_empty_row` removed from public exports (zero call sites post-RFC-092). 3 `get_summary` tests added. **107/107 tests; all 5 CI gates PASS.** |
| v0.74.0 | **RFC 092 (UI component suite).** ThemeToggle: `no-js`→`js` class swap, blocking `theme-init.js`, noscript fallback. `EmptyState` wired into users/clients/signing-keys/audit. `CopyField` (`readonly` + `role="status"`). `error_summary` (`role="alert"`). 10 i18n keys. CSS token fixes. **UI-security arc (RFCs 087–092) COMPLETE.** **104/104 tests; all 5 CI gates PASS.** |
| v0.73.0 | **RFC 091 (LoginContext rendering).** `LoginContext` enum in `sui-id-web`; trusted-name `OidcAuthorize` derivation via DB lookup; context-aware copy on login page (`AdminPanel` / `OidcAuthorize` / `SelfService`); 5 i18n keys. **104/104 tests; all 5 CI gates PASS.** |
| v0.72.0 | **RFC 090 (signing-key rotation confirm + settings pending-change).** Migration 0032; `pending_settings_change` repo (8 tests); `PendingChangeId` newtype; `pending_change` core domain (create/apply/cancel/purge); `GET /admin/signing-keys/rotate-confirm` + step-up revalidation on POST; SMTP `email_post` → pending-change redirect when password provided; `GET|POST /admin/settings/email/confirm`; 7 i18n keys; 4 audit events. **104/104 tests; all 5 CI gates PASS.** |
| v0.71.0 | **RFC 089 (step-up contract).** `sanitise_return_to` gains `STEP_UP_RETURN_ALLOWLIST` — non-allowlisted `?return_to=` falls back to `/me/security`. 12 unit tests. Recovery-code exclusion documented in `policy_for_session`. RFCs 089–092 proposed (UI-security units 3–6). **96/96 tests; all 5 CI gates PASS.** |
| v0.70.0 | **RFC 088 (auditor authorization matrix + read-only rendering).** 6 mutation-only GET routes corrected: `users_new_get`, three user confirm-GETs, `clients_new_get`, `clients_delete_confirm_get`, `signing_keys_delete_confirm_get` — all now return HTTP 403 for auditors (not 401-redirect). `HttpError::html_403_auditor()` helper added. 3 i18n keys. `can_write: bool` plumbed through all settings data structs. **96/96 tests; all 5 CI gates PASS.** |
| v0.69.0 | **RFC 084 + RFC 086 (fuzzing + formal verification pilot).** Six cargo-fuzz targets; smoke runs clean (1 000 iter each on openssl-free targets); nightly toolchain pinned; scheduled weekly CI job. TLA+ spec for RFC 080 rotation protocol (guarded = Inv1 holds; guard-less = Inv1 violated). Five Kani proof harnesses for RFC 082 P1–P5 under #[cfg(kani)]. Pilot reports: Kani(authorize)=Adopt, TLA+(rotation)=Adopt(doc), Flux=Defer. **96/96 tests; all 5 CI invariants PASS.** |
| v0.68.0 | **RFC 083 + RFC 085 (state-machine testing + audit completeness).** Three proptest state-machine harnesses (auth-codes, refresh-tokens, sessions) — 256 cases each, named INV_* invariants, 27s total. `audit::append_within_tx` for Class-A atomicity. `AuditReceipt`/`Audited<T>` types in `sui-id-core`. Normative audit coverage matrix (39 events at time of ship; now 43 with pending_change events added in v0.72.0) + CI gate. Fixed stale dashboard prefix. **96/96 tests; all 5 CI invariants PASS.** |
| v0.67.0 | **RFC 081 + RFC 082 (actor scope boundary + authorization decision core).** `Actor`/`AdminActor`/`ReadOnlyAdminActor`/`SelfActor` capability types; pure `authorize(role, action) -> Decision` table with exhaustive tests (P1–P5). All admin mutations now require `&AdminActor`; self-service requires `&SelfActor`; a privileged call without proof of privilege is a compile error. Last-admin safeguard delegates to the authz table. `require_admin` deprecated. **90/90 tests; all CI invariants unchanged.** |
| v0.66.0 | **RFC 079 + RFC 080 (auth-code lifecycle assurance + refresh-token rotation atomicity).** `consume` enforced by SQL predicate + rows-affected guard. Typestate pipeline (`ConsumedCode`→`BoundCode`→`PkceVerifiedCode`→`IssuableGrant`) in `exchange_code`. `begin_rotation` closes the 3-closure TOCTOU race with a single-tx rows-affected arbitration; `RotationLookup` makes reuse-detection explicit. Migration 0031. **90/90 tests; all CI invariants unchanged.** |
| v0.65.1 | **RFC 087 (clippy/rustfmt baseline cleanup).** All four buildable crates clippy-clean (`--all-targets -D warnings`) and fmt-clean under Rust 1.96. Fixes across sui-id-web (16), sui-id-shared (2), sui-id-store (16 lib + test-target), sui-id-i18n (3). 31 files reformatted. No logic change. **78/78 tests; all CI invariants unchanged.** |
| v0.65.0 | **WCAG AA contrast correction — token foundation (UI/UX handoff unit 1).** Dark-mode AA defect fixed (all 5 colour pairs were failing, worst 1.5:1). Light-mode fills darkened to pass AA. Explicit `--fg-disabled`/`--bg-disabled` tokens; `button:disabled` wired to explicit tokens. Contrast CI test (`tokens/tests.rs`) validates all pairs in 3 modes. Dangling `--surface-overlay` reference fixed. **78/78 tests; all CI invariants unchanged.** |
| v0.62.0 | **RFC 075 + RFC 076 (soak cleanup).** Mechanical file splits: `admin.rs`→`admin/`, `backup.rs`→`backup/`, `main.rs`→`cli.rs`. Full `configuration.md` reference (10 fields, env vars, flags, examples). **175/175 tests; all CI invariants unchanged.** |
| v0.61.0 | **RFC 074 (Navigation + UX polish).** User-menu dropdown replaces flat Security link. "Apps" nav label. Settings: Basic→General, Other→Advanced. Migration 0030 (`last_login_at`); last-login anti-phishing line on `/me/security/overview`. 6 i18n keys. **175/175 tests PASS; all CI invariants unchanged.** |
| v0.60.1 | **v0.60.1 (Documentation).** CHANGELOG dated; README and docs updated for three-role model and UX-rethink arc; RFC 074 filed. No code changes. |
| v0.60.0 | **RFC 072 (End-user app-access surface).** Migration 0029 (`user_consent.last_used_at`). `list_for_user`, `revoke_with_tokens`, `touch_last_used` repo helpers. `TokenSet.user_id` for best-effort `last_used_at` update at token exchange. `MeTab::Apps` + `render_me_apps`. `GET /me/apps` + `POST /me/apps/{id}/revoke`. 9 i18n keys. **175/175 tests PASS; all CI invariants unchanged.** |
| v0.59.0 | **RFC 071 (Auditor role).** `users.role` column (migration 0027) + `audit_log.actor_role` (0028). `Role` enum with `is_admin()` / `can_read_admin()`. `CurrentAdminOrAuditor` extractor on all GET admin routes. `can_write: bool` in 5 render functions hides mutation controls from auditors. Role-change UI on user detail with last-admin safeguard. 7 new i18n keys. **175/175 tests PASS; all CI invariants unchanged.** |
| v0.58.0 | **RFC 073 (Dashboard action items).** Getting Started checklist (3 items, ☐/✓ ABDD-safe text indicators) + 4 new action items (admins without MFA, old signing key, stuck outbox, pending resets). 4 new read-only repo helpers. 8 i18n keys (×3 locales). `.action-items-list` and `.checklist` CSS. **228/228 tests PASS; all CI invariants unchanged.** |
| v0.57.1 | **Dependency refresh: RFC 069 (rand 0.10) + RFC 070 (ureq → reqwest).** rand 0.8→0.10 via getrandom; `OsRng.fill_bytes` (×10), `SaltString::generate`, `SigningKey::generate` (Option B: Zeroizing + from_bytes) all migrated. ureq removed; `HibpClient` trait made async via async-trait; `HttpHibpClient` rebuilt on reqwest 0.12. Bug fixed: enforce_hibp now properly awaits the check instead of blocking the tokio thread. **228/228 tests PASS; all CI invariants unchanged.** |
| v0.57.0 | **Phase 8 complete — MI arc fully closed: RFC-MI-080 (UI Regression + A11y Hardening).** Skip link added to Shell and AuthShell (WCAG 2.4.1). `<header role="banner">`, `<main id="main-content">`. `@media (max-width: 480px)` and `(max-width: 360px)` breakpoints added. New i18n key `a11y_skip_to_main`. Six verification matrices committed (`docs/src/mockup-integration/`). **16/16 MI RFCs in `done/`. `inline-style-bound` = 0. 228/228 tests PASS.** |
| v0.56.0 | **Phase 7 complete: RFC-MI-070 (OIDC Consent UX). `inline-style-bound` reaches 0.** Four inline styles in `pages/oidc.rs` eliminated via `.consent-card`, `.consent-intro`, `.consent-scope-list`, `.consent-scope-item` classes. Scope item structure improved. PKCE/redirect validation unchanged. 15/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.55.0 | **Phase 6 complete: RFC-MI-060 (Self-Service Security Tab Integration).** Password-change page (`render_password_change`) updated: `show_nav=true`, `current="me"`, tab strip added. All six `/me/security/*` routes now consistently render `.route-tabs` with `aria-current="page"`. MFA enable/disable decision documented (Option 2: self-service + admin reset). Cancel link updated to `/me/security/overview`. Form actions migrated to `.form-actions`. No i18n changes. `inline-style-bound` = 4 (unchanged). 14/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.54.0 | **Phase 5 complete: RFC-MI-050 (Form System) + RFC-MI-051 (Danger Zone).** Two new form CSS primitives (`.field--required`, `.review-summary`) added to `forms.rs`. User detail page restructured: action buttons moved from header into a `.danger-zone` section at the bottom. New i18n key `user_detail_danger_zone_body` (×3 locales). All confirmation routes unchanged. `inline-style-bound` **5 → 4**. 14/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.53.1 | **Phase 4 complete: RFC-MI-040 (Setup Wizard UX).** `StepState` enum + `SetupStep` struct added to `components/setup.rs` (re-exported from `components.rs`). `.setup-steps` nav container class and `.setup-step__label--{current,done,upcoming}` classes replace the two inline style= attributes in `setup_step_indicator()`. `inline-style-bound` **7 → 5**. 12/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.53.0 | **Phase 4 opens: RFC-MI-041 (Authentication Surfaces).** Ships ahead of MI-040 at user request. Three inline styles eliminated in `pages/auth.rs` (login forgot-password link, MFA QR code, password-change card). Three new CSS classes: `.auth-meta-link`, `.qr-display`, `.card--narrow`. ABDD: `FlashKind::aria_role()` added to `common.rs` — `Error` → `role="alert"`, `Info`/`Warn` → `role="status"`. **Zero copy / zero i18n changes** (security review confirms anti-enumeration wording byte-identical). `inline-style-bound` **10 → 7**. 10/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.52.0 | **Phase 3 complete: RFC-MI-030 (Dashboard) + RFC-MI-031 (Audit + Tables).** Dashboard: warning callout migrates to `.callout--warning`; 4 sparkline inline styles eliminated via `.sparkline-{container,header,title,legend}` classes. Audit: `.cell-id`, `.cell-nowrap`, `.cell-actions` added to `tables.rs`; applied to `audit_row_view`; filter row inline style eliminated via `.filter-bar` class. Total: **6 inline styles eliminated; `inline-style-bound` 16 → 10**. 9/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.51.1 | **Phase 2 complete: RFC-MI-022 (Route-Based Tab Component).** `.route-tabs` + `.route-tabs__link` CSS added. `RouteTab` + `route_tabs()` fn. `MeTab::Password` added. Both tab helpers migrated. `inline-style-bound` 17 → 16. **228/228 tests PASS.** |
| v0.51.0 | **Phase 2 opens: RFC-MI-020 (Shell Layout decision) + RFC-MI-021 (Server-Rendered CSRF).** Shell: keep top-nav decision recorded; no structural code change. CSRF: Shell now requires `csrf_token: String`; Nav renders token directly into sign-out form hidden field; `logout-csrf.js` removed. 27 Shell call sites updated; 5 render function signatures updated. Sign-out works with JS disabled. **228/228 tests PASS; 0 warnings; all 4 CI invariants unchanged.** |
| v0.50.1 | **Phase 1 complete: RFC-MI-011 (Token Mapping + Visual Primitives) + RFC-MI-012 (Theme Persistence).** Zero new CSS tokens (mockup vocabulary is a strict subset of the product's). Three CSS primitives adopted: `.callout` + tone variants (→ `cards.rs`), `.field__error` + `.field--invalid` (→ `forms.rs`), `.dl-grid` (→ `utilities.rs`). Theme persistence: **Option A chosen** (preserve `localStorage` model, no code change). Phase-1 blockers D-01/D-02/D-03 status: D-01 resolved (v0.50.0); D-02 and D-03 owned by Phase 2 (RFC-MI-022 and RFC-MI-021). **228/228 tests PASS; 0 warnings; all 4 CI invariants unchanged.** |
| v0.50.0 | **Phase 1 opens: RFC-MI-010 (Component CSS Sharding).** `components.rs` (1094 lines) split into 11 bounded shards under `components/` (badges, banners, buttons, cards, chrome, confirm, forms, setup, tables, tabs, utilities). `StatusKind` + `status_badge` moved to `badges.rs`; re-exported from `components.rs` for backward compatibility. `components_css()` fn (OnceLock-cached) replaces the former `COMPONENTS_CSS` const — produces a byte-identical CSS body to v0.49.x. Phase-1 blocker `D-01` (CSS sharding) resolved. **228/228 tests PASS; 0 warnings; all 4 CI invariants unchanged.** |
| v0.49.1 | **Phase 0 of the Mockup Integration arc completes.** The six baseline-inventory documents specified by `RFC-MI-000` (`screen-map.md`, `dangerous-action-map.md`, `tab-routing-delta.md`, `token-delta-draft.md`, `i18n-copy-delta-draft.md`, `route-render-handler-map.md` + a `README.md` index) ship under `docs/mockup-integration/inventory/`. Headline findings: zero new CSS tokens (mockup vocabulary is a strict subset of the product's), 18 dangerous-action values reduce to 9 link-rewrites + 5 do-not-implement + 3 step-up-policy-deltas + 1 inline-only, the 382 mockup-only i18n keys are mostly renames (~58 net-new keys × 3 locales = ~174 translation entries). `RFC-MI-000` moves to `rfcs/done/` with `Status = Implemented (v0.49.1)`. **No runtime code change**; CI invariants unchanged; 228/228 library tests PASS. |
| v0.49.0 | **Opens the Mockup Integration ("MI") arc.** Sixteen `RFC-MI-NNN` documents added to `rfcs/proposed/` (Phase 0 → Phase 8 plan); supporting planning artifacts placed under `docs/mockup-integration/` (migration plan, codebase handoff, mockup handoff package) and `docs/development-specification.md` (v3 spec). `rfcs/README.md` rewritten to surface the MI namespace and the eight-phase implementation order. Phase-1 blockers `D-01`/`D-02`/`D-03` restated. Workspace version → 0.49.0. **No runtime code changes**: CI invariants unchanged at their v0.48.4 values (228/228 floor unaffected; text-leaks 0; inline-style-bound 16; css-tokens green; semantic-palette-parity 12×3). |
| v0.48.4 | **Setup UX.** (1) Setup token moved from text-input to URL parameter: startup now prints a full URL (`/setup?token=xxx`), the welcome screen forwards it to `/setup/admin?token=xxx`, and the admin form holds it as `<input type="hidden">` — operators no longer copy-paste a raw token string. Token travels through language PRG redirects and error re-renders unchanged. (2) Chinese (`中文`) removed from setup wizard language picker — core i18n covers ja and en only; showing zh would be misleading. 228/228 PASS; 0 warnings. |
| v0.48.3 | **Verification-phase bug: `email` claim absent from ID token.** External RP reported `JSON error: missing field 'email'` at OIDC callback. `IdTokenClaims` had no `email`/`email_verified` fields; only the UserInfo endpoint returned them. OIDC Core §5.1: `email` scope SHOULD populate those claims in the ID token too. Fix: added `email: Option<String>` + `email_verified: Option<bool>` (both `skip_serializing_if = "Option::is_none"`) to `IdTokenClaims`; `issue_token_set` takes a new `user_email: Option<(&str, bool)>` param; `exchange_code` passes it from the already-fetched user row; `exchange_refresh` adds a conditional `users::get` only when scope includes `"email"`. Accounts without email → field omitted (not null). `email_verified` faithfully reflects `email_verified_at IS NULL`. 228/228 tests PASS; 0 warnings; CI PASS. |
| v0.48.2 | **Second verification-phase release (verification-pass buffer).** Six issues from the same real-environment round that produced v0.48.1. **Bug 1** (`::selection` invisible): `--accent-default` + `--fg-on-accent` replaces `--accent-subtle`. **Bug 5** (`/me/security/overview` i18n): 3 hardcoded/miskeyed strings replaced with 3 new keys (`me_overview_label_mfa_totp`, `me_overview_label_passkeys`, `me_overview_no_recent_events`) × en/ja/zh. **Issue 4** (setup wizard language): explicit 3-button picker on welcome screen, `?lang=xx` → LANG_COOKIE set (PRG) → all subsequent wizard steps auto-locale via existing RequestLocale. **Issue 6** (footer a11y labels): `<ul role="note">` / `<li class="app-footer__a11y-item">` with `cursor: default` and caption sizing — passive informational badges, not interactive. **Issue 7** (tagline prominence): caption-size + muted + opacity 0.75. **Bug 8** (mobile responsive): first `@media (max-width: 768px)` in codebase; `.app-nav__link { white-space: nowrap }` + `td/th { white-space: nowrap }` + `.cell-wrap` opt-out class; nav horizontal-scroll, main padding shrink, footer column stack. Tests stable at 228/228; 0 warnings; CI invariants PASS. |
| v0.48.1 | **First verification-phase hotfix.** Three lock-out / main-feature bugs surfaced during actual-environment testing of v0.48.0 at localhost:8801 — all CSP-related. **Bug 2** (CSP `script-src 'self'` blocking 3 inline `<script>` blocks + 3 inline `onclick=` handlers → theme toggle, clipboard copy, sign-out all silently failed): externalised the inline JS into `/static/theme-init.js`, `/static/copy.js`, `/static/logout-csrf.js`; theme buttons keep only `data-theme-value` attributes and `theme-init.js` attaches listeners on DOM-ready. **Bug 3** (sign-out → /admin redirect loop): subsumed by Bug 2 fix — CSRF token injection script now runs. **Bug 9** (401 lock-out after restart, "Back home" loops to /admin): `html_error_response` now redirects `CoreError::Unauthenticated`+HTML to `/admin/login` instead of rendering a 401 page; `pages/error.rs` "Back home" is context-aware (401 → /admin/login, else → /). Tests stable at 228/228; 0 workspace warnings; CI invariants all PASS. No new RFC consumed (hotfix scope). Six other v0.48.0 issues (`::selection` color, /me/security/overview i18n, mobile responsive on nav + tables, setup wizard language picker, footer a11y label intent, title tagline restraint) deferred to v0.48.2 — none of them lock operators out. |
| v0.48.0 | **Phase F (final buffer)** — RFC 068 (`handlers/me_security.rs` 1099 LOC → 7 sub-modules, Rust 2018+ style; all under 500 LOC) + RFC 067 (inline-style discipline: 119 → 16 with 40+ utility classes in `components.rs`; new CI bound `inline-style-bound` at 20). Pre-existing warning cleanup: 5 issues cleared (dead `mailer`/`title`, `_caches`/`_clock` rename for API symmetry, `decrypt_field` allow(dead_code)). 0 workspace warnings. Phase F closes; project enters verification phase. **No v1.0-rc/pre tag is scheduled from this release** — sufficient soak, external review, and integration verification precede any v1 designation. |
| v0.47.1 | **Phase F (continued)** of the UI/UX hardening plan — RFC 066 (`handlers/admin.rs` 1531 LOC → 8 sub-modules under `admin/`, Rust 2018+ module style; every file under spec's 500-LOC ceiling, umbrella 55 LOC; public route paths unchanged through `pub use {submodule}::*;` re-exports). Hygiene: 14 `#[derive(...)]` attributes lost during extraction were re-attached from the original; 85 unused-import warnings auto-pruned by `cargo fix`; `_silence_state*` dead-code suppressors removed (the split made them unnecessary). RFC 067 (inline-style discipline) + `handlers/me_security.rs` split deferred to v0.48.0, the final Phase F buffer release. |
| v0.47.0 | **Phase F (partial)** of the UI/UX hardening plan — RFC 065 (`pages.rs` 4170 LOC → 22 sub-modules under `pages/`, Rust 2018+ module style throughout; every file under spec's 500-LOC ceiling; sub-directory splits for `settings/` and `me_security/`; public API surface unchanged through `pub use {submodule}::*;` re-exports). Build hygiene: 22 unused-variable warnings cleared; 7 genuine dead code removals (`let csrf_*`/`let *_url` from pre-Phase-D row buttons). RFC 066 (admin.rs split) deferred to v0.47.1; RFC 067 (inline-style discipline) + `handlers/me_security.rs` split deferred to v0.48.0. |
| v0.46.0 | **Phase E** of the UI/UX hardening plan — RFC 061 (semantic palette extension: 12 new tokens completing `--{semantic}-subtle` + `--fg-on-{semantic}` triples for danger/warning/success/info × light/dark/auto-dark; closes v0.44.0 `.banner--success` regression where `--success-subtle` was used but undeclared; new CI job `semantic-palette-parity` enforces structural completeness), RFC 062 (card variants `.card--warn`, `.card--info`, `.card--success`, `.card--callout` over `.card` base; 2 inline `border-left` migrations), RFC 063 (dashboard signal/noise reorder: recent events promoted above stats with `.card--info`, sparkline demoted to h3+opacity), RFC 064 (`empty_state()` + `table_empty_row()` primitives replacing 5 ad-hoc `<p class="muted">No X yet.</p>` sites). |
| v0.45.0 | **Phase D** of the UI/UX hardening plan — RFC 058 (step-up enforcement on 4 dangerous routes: `users_set_disabled`, `clients_set_disabled`, `mfa_disable`, `passkey_delete`), RFC 059 (`<ConfirmScreen>` shared component; 5 `render_confirm_*` functions delegate to one template), RFC 060 (audit-note rollout: 7 use cases gain `reason` parameter, 8 handlers migrate to new `ConfirmedReasonForm`, self-service routes write canonical `"self"` note, reason textarea added to all 5 confirm screens). Latent bypass closed: 5 routes accepted POSTs without `_confirmed=1`; now enforced server-side. New docs page `guides/dangerous-operations.md`. |
| v0.44.0 | **Phase C** of the UI/UX hardening plan — RFC 055 (consolidate self-service onto `/me/security/*`: 9 handlers moved, `render_profile` removed, Nav "Profile" → "Security", 301 redirect for the GET endpoint, all old POST routes deleted), RFC 056 (recovery codes remaining: new `count_recovery_codes_remaining()` + i18n template replacing the hardcoded `0`), RFC 057 (language save confirmation banner via `?saved=1`), RFC 054 (aria-label sweep: 3 sites remaining after RFC 051's incidental fixes, now done). Bug fix: `.banner` CSS family was used in code but never declared — added in this release. |
| v0.43.0 | **Phase B** of the UI/UX hardening plan — RFC 051 (per-screen i18n completeness audit: 95 hardcoded JA strings → 0 across every render function in `pages.rs`; ~100 new typed Strings fields with ja/en/zh values), RFC 052 (status word + empty placeholder vocabulary unification, completing pre-existing partial work), RFC 053 (copy-button i18n contract, last call site `audit_row_view`). Bug fix: missing Chinese option on `/me/security/language`. Language self-name discipline (`locale_native_*`). RFC 048 grep widened to catch 28 additional brace-missing sites missed in v0.42.0. RFC 054 deferred to v0.44.0. |
| v0.42.0 | **Phase A** of the UI/UX hardening plan — RFC 048 (48 `t.xxx` literal-leak fixes), RFC 049 (CSS token freeze + 7 typo fixes), RFC 050 (admin chrome i18n: Nav, Footer, ThemeToggle). Plus the `/me/security/*` locale-resolution fix. Two new CI invariants (`text-leaks`, `css-tokens`). |
| v0.41.0 | RFC 040 completion (`/me/security/mfa`+`/sessions`), RFC 045 (user disable reason), RFC 046 (audit copy-ID), RFC 047 (dev summary + secret rotation) |
| v0.40.0 | RFC 040 (`/me/security` tabs initial), RFC 041 (HIBP consistency), RFC 042 (error i18n), RFC 043 (dashboard events), RFC 044 (state-word contract) |
| v0.39.0 | RFC 038 (consent screen), RFC 039 (settings i18n complete) |
| v0.38.0 | e2e coverage (RFC 030/033/035), audit-events doc, settings i18n section headers |
| v0.37.0 | RFC 029 pass 2 (dynamic locale), RFC 035 (user detail), RFC 036 (docs/Phase 5) |
| v0.36.0 | RFC 030 (dangerous ops confirm), RFC 031 (dashboard prompts), RFC 033 (audit), RFC 034 (passkey+empty) |
| v0.35.0 | RFC 032 (dev mode banner), RFC 029 first pass (admin i18n) |
| v0.34.0 | RFC 002 (i18n: zh locale, Formatters, audit labels, dir=, per-recipient locale) |
| v0.33.0 | RFC 001 (email outbox + retry worker) |
| v0.32.0 | RFC 017 (UI/UX contracts), RFC 023 (visual design system), RFC 024 (doc consolidation) |
| v0.31.0 | RFC 014 (hot-path caches), RFC 028 (copy buttons) |
| v0.30.0 | RFC 013 (async DB layer — full implementation + test fixes) |
| v0.29.13 | RFC 026 (admin logout), RFC 027 (client scope UX), dup-username bug fix |
| v0.29.12 | RFC 013 async DB layer initial |
| v0.29.10–11 | RFC 021/022 (schema invariants, boolean CHECKs, migration safety) |

Full history: [CHANGELOG.md](CHANGELOG.md)

---

## Status

**v0.60.0** completes the UX-rethink arc (RFCs 071, 072, 073) identified
in the post-MI-arc audit. All three targeted gaps are closed:

- RFC 073 (v0.58.0) — Dashboard action items
- RFC 071 (v0.59.0) — Auditor role (read-only admin access)
- RFC 072 (v0.60.0) — End-user app-access surface (`/me/apps`)

**v0.62.0** completes all verification-soak items identified during
the v0.61.0 audit (RFC 075: file-size refactor, RFC 076: configuration
reference). The project is now in **verification soak**.

The remaining open requirements for a v1.0 designation are external
to this repository:

1. **External OIDC integration verification** — run sui-id against a
   real relying party (e.g. a web app using `openid-client` or Passport).
2. **Optional second-party security review** — code review by a party
   other than the primary author.

All planned engineering work is complete. The remaining `rfcs/proposed/`
items (RFC 004, 005, 006, 008, 009, 025) are post-1.0 exploratory work;
none are scheduled.

All 16 MI RFCs across Phases 0–8 are implemented and in
`rfcs/done/`. The arc spanned v0.49.0 through v0.57.0.

Final metrics against v0.48.4 baseline:

| Metric | v0.48.4 baseline | v0.57.0 |
|---|---|---|
| `inline-style-bound` | 17 | **0** |
| MI RFCs completed | 0 | **16 / 16** |
| CSS shards | 1 monolith | **11 bounded shards** |
| Skip link | absent | **present (WCAG 2.4.1)** |
| No-JS sign-out | JS required | **server-rendered** |
| Responsive breakpoints | 768px only | **768 / 480 / 360px** |
| `inline-style-bound` ceiling | 119 (pre-v0.48.0) → 16 (v0.48.4) → 20 (MI arc limit) | **0** |

The project remains in **verification phase**. The MI arc
completion is a quality milestone, not a v1.0 gate.
**No release will start with v1 until sufficient soak, external
review, and integration verification have occurred.**

---

## Constraints and non-goals (pre-1.0)

- **Single realm.** All users share one namespace. Per-tenant isolation is
  RFC 025, post-1.0. See [docs/operators.md](docs/operators.md) §
  "User–client relationship".
- **SQLite only.** Alternative backends are RFC 009, low priority. The
  current SQLite implementation is production-grade for small deployments.
- **No user-facing theming API.** CSS tokens are for the maintainer, not
  operators.
- **No plugin system.** RFC 005 sketches one; it is not scheduled.

# RFC 097 security-review checklist

**Governing RFC:** [RFC 097](../../proposed/097-current-threat-model.md)

## 1 — Inventory before analysis

Enumerate, with an owner for each: actors; assets; entry points; data stores;
processes; outbound connections; and trust boundaries. The model is not started
until this inventory is complete, because STRIDE over an incomplete inventory
produces confident coverage of the wrong system.

## 2 — Boundary coverage

Every shipped external trust boundary gets its own analysis. At minimum:

| Boundary | Must cover |
|---|---|
| Browser ↔ application | Session, CSRF, cookies, headers, step-up |
| OAuth/OIDC client ↔ token endpoint | Client authentication, code exchange, refresh rotation and reuse |
| Dynamic registration | Token issuance and consumption, metadata validation, contention |
| Upstream OIDC federation | Discovery, JWKS, ID-token claims, nonce, egress/SSRF, hostile provider |
| LDAP | Bind, shadow-user upsert cascade, outage behaviour |
| SMTP | Outbound mail, enqueue/worker state |
| HIBP | Outbound query, failure mode |
| Metrics | Authentication, exposure surface |
| Filesystem and environment | Database and key file modes, config and secret sourcing, master-key rotation artifacts |
| Operator/CLI | Privileged commands, bootstrap, dev-mode reachability |

## 3 — Per boundary

For each: STRIDE, then the controls that address each threat, then the evidence
each control actually works, then what remains.

**A control claim without an evidence link is a finding, not a control.** Cite
the milestone, the test, or the observed run. "Implemented in M2a" is
insufficient; "M2a rollback evidence, command U06" is a citation.

## 4 — Cross-boundary compositions

Single-boundary analysis misses the interesting attacks. Explicitly consider at
least: federation identity feeding session establishment; dynamic registration
feeding authorization and logout redirect matching; LDAP shadow users feeding
role and permission resolution; audit-chain trust versus a database writer;
step-up context crossing self-service and admin surfaces; and outbound egress
reachable from attacker-influenced input.

## 5 — Failure and rollback

For each boundary, state what happens when it fails: does the system fail
closed, and is partial state possible? The remediation programme's central claim
is that mutation and audit commit or roll back together — the model must state
where that holds, where it does not yet (any unconverted M2b command at time of
writing), and what the consequence is.

## 6 — Residual risk

Every accepted residual risk records: the risk, why it is accepted, who accepted
it, and what would change the decision. Anonymous acceptance is not acceptance.

Known items that must appear if still true when drafted. The first two are
tracked as standing programme subjects in
[`ROADMAP.md`](../../../ROADMAP.md) §Standing programme risks and constraints —
carry them here with a named accepter rather than restating them as open:

- **S2** — the audit hash chain is unkeyed and unanchored: tamper-evident within
  its trust boundary only, and **not** evidence against a malicious database
  writer. RFC 094 corrects the claims; it introduces no external anchor.
- **S1** — review independence. **Decided 2026-07-28:** vendor independence is
  required for the RFC 094 seam, RFC 096 federation, this threat model, and M6
  closure; role independence applies everywhere else. Record here whether that
  requirement was actually met for each — a ruling that was not honoured is a
  finding, not a control. Note also that this RFC is itself in the ruled set, so
  its own reviewer must be outside the authoring vendor.
- master-key rotation, if RFC 100 is not yet Implemented — see RFC 100 and the
  M2c entry gate.

## 7 — Honesty checks before sign-off

- No control is described as present when only designed.
- No boundary is omitted because it is inconvenient to model.
- No completed work is described as future, and no future work as completed —
  the failure mode of the document being replaced.
- Every claim about a gate cites an observed run, not an expectation.

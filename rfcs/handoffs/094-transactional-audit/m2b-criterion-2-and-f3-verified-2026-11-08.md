# M2b criterion 2's reading (§F1) and `rusqlite` under dev-dependencies (§F3), verified

> ## Both rulings settled, 2026-11-08 — `@nabbisen`: *"Both your directions are accepted."*
> §F1 is reading **(b′)**. §F3 is **production reach**, with RFC 094's sentence
> amended and the check moved into G23, both inside stage 10.

**Asked.** *"Verify carefully that it meets my fundamental philosophy"*
(§F1), and whether §F3 *"is up to the result of verification on 3"*.
**Prepared by.** The architect, 2026-11-08, against the code at `044b12c`.
**The philosophy, both halves.** Clean, safe, secure, robust and
sophisticated design; **and** APIs and UI/UX that users cannot
misunderstand. Each reading below is tested against each half separately,
because the earlier recommendation was tested against the first half only.

## 1 — §F1: what "no production Class-A best-effort append anywhere" should mean

### Three readings

| | Reading | Consistent with RFC 094 itself? |
|---|---|---|
| **(a)** | Literal: no best-effort append in production at all | **No.** RFC 094's invariant 5 requires the opposite for denials, detections and revocations: *"use Class B. Append failure is reported … without … changing the denial."* (a) would make a refused sign-in or a revocation depend on the audit write succeeding, so a full disk would un-deny a denial. It contradicts the RFC it reads |
| **(b)** | The earlier recommendation: no best-effort append **accompanying a Class-A mutation** | Yes |
| **(b′)** | (b), plus the conditions that make it true in practice (below) | Yes |

**(a) is out on the first half by itself.** An audit trail that can switch
off a security response is less safe, not more.

### (b) tested against the first half: safe, robust

Measured, all nine sites (§2 of the M2b assessment): **no best-effort append
accompanies a Class-A mutation.** But (b)'s safety rests on two things it
does not itself check.

1. **Class B must mean what the matrix says it means.** The matrix (`:32`)
   defines B as *"a failure is **logged loudly**"*. Seven of the nine sites
   are `let _ = audit::append(...)`, which **discards** the failure. Under
   (b) as written they pass, while what they do is a third class nobody
   defined: *silently dropped*. **So (b) alone admits a record that can
   vanish without trace**, and the denial-or-revocation it describes then
   happened invisibly.
2. **The classification that exempts an append must itself be right.** (b)
   lets an append be best-effort when the mutation beside it is Class P or I.
   That moves the question from the append to the row's class, which is the
   right place for it, provided each class is justified. Measured for the
   four that carry best-effort appends:
   - **U32** (pending MFA) is P: *"short-lived one-time protocol state"*.
     Justified.
   - **U34** (reset token) is P, and its row states that its events are Class
     B. Justified.
   - **T06**/**T07** (token revocation) are I, *"subordinate to"* named
     commands, and **that list omits the RFC 7009 path** that actually calls
     them (residual R2). Not justified as written.

**On the first half, (b) holds only with R1 fixed** (every surviving
best-effort append reports its own failure, which is what Class B already
claims) **and R2 fixed** (the exempting rows name their real callers).

### (b) tested against the second half: users cannot misunderstand

**The users of an audit log are operators, and they read it to answer "did
this happen?"** Measured: `docs/src/reference/audit-events.md` never says, in
general, that a best-effort event can be absent from the log, or where the
evidence of its absence goes. An operator who finds no `token.revoke` row for
a token can reasonably conclude it was never revoked. **That is the
misunderstanding the second half forbids, and (b) does nothing about it.**

**On the second half, (b) holds only if the operator reference says which
events can be missing and how you would know**: each such failure is a line
in the service log, after R1.

### Result

**(b) does not meet your philosophy as recommended. (b′) does:**

> **Criterion 2 means:** no best-effort append accompanies a Class-A
> mutation; **every best-effort append that remains reports its own
> failure** (Class B as the matrix defines it); **every row that exempts an
> append by its class names its real callers**; and **the operator reference
> says which events can be absent from the log, and that each absence leaves
> an error in the service log.**

**This changes the earlier recommendation**, and the change is the point of
the verification: the M2b assessment put R1 *after* `0.88.0`. **Under (b′),
R1 and R2 are part of criterion 2, so they belong in stage 10.** The
stage-10 dispatch includes them, plus the operator-reference paragraph.

## 2 — §F3: does it depend on §F1? No, but it fails the same way

**The subject is different.** §F1 is about audit records; §F3 is about which
crates may reach the database driver. **Neither answer constrains the other.**
They share a shape, though, and verifying them the same way finds the same
kind of fault: the text and the code disagree.

### The two readings, against the first half

- **Strict** (no `rusqlite` in `[dependencies]` **or** `[dev-dependencies]`
  outside `sui-id-store`). This forbids `crates/sui-id/Cargo.toml:120`, used
  only by that crate's own tests to inspect and seed databases with raw
  SQL. **Cargo never links a dev-dependency into a shipped binary**, so the
  strict reading protects no production path. It would mean rewriting at least
  nine e2e files (cycle J §F3's count), and black-box tests that check the database directly, rather
  than through the API under test, are the stronger test, not the weaker.
- **Production reach** (`[dependencies]` only; M2a closed on this,
  `tests/e2e/r094_m2a_db_access_scope.rs`). It protects exactly what
  invariant 6 is about: shipped code cannot obtain a write handle except
  through the store.

**On the first half, production reach is right.**

### Against the second half: the text misleads twice today

1. RFC 094 says `[dev-dependencies]` too. **The code does the opposite, and
   M2a closed on the opposite.** A reader of the RFC is told something
   false.
2. RFC 094 says **the structural gate** fails on it (*"one boundary, one
   gate"*, the A6 sweep note). **Measured: G23 reads no Cargo manifest at
   all.** The condition lives only in an e2e test, which the RFC does not
   mention, and which runs under a different lane.

**On the second half, both must be fixed**, or the boundary's own
documentation is the thing that misleads.

### Result

**Production reach, with the text amended and the check moved where the RFC
says it lives:**
- RFC 094's sentence becomes *"`rusqlite` appears in the `[dependencies]`
  table of no crate other than `sui-id-store`"*, with one clause saying
  why dev-dependencies are excluded.
- **G23 reads the manifests** and enforces it, with a negative fixture.
  The e2e test then either retires, or stays as a second witness and says
  that it is one.

**Material or not is yours to judge.** It narrows a sentence of a security
boundary's text. Against that, it brings the text into line with the
evidence M2a was accepted on, and adds enforcement where the RFC promised
it. The architect's view is that it is a **correction**, not a weakening,
since nothing that was ever enforced is relaxed. But RFC 000 leaves this
call to you for an Accepted RFC.

## 3 — your two rulings

1. **§F1:** adopt **(b′)** as criterion 2's meaning, with R1, R2 and the
   operator-reference paragraph inside stage 10. **Recommended.**
2. **§F3:** adopt **production reach**; amend RFC 094's sentence; move the
   check into G23 with a fixture, as **part of stage 10** if you agree, or
   as its own round. **Recommended: inside stage 10**, so M2b's gate is one
   boundary in one place when M2b closes. And say whether you judge the
   amendment material.

# RFC 096-B1 stage 6d — F01, as Class A folded into L04

> ## Whose turn it is: **the implementation role's.**
> The RFC 094 record question that gated this stage is settled —
> `@nabbisen` accepted the amendment on 2026-10-15 and RFC 094 is Accepted
> again with F01 reclassified. Nothing is waiting on him or on the architect.

**Dispatched.** 2026-10-15 JST, by the architect.
**RFC.** 096 — **Status: Accepted**. **RFC 094 — Status: Accepted**, re-accepted
2026-10-15 with the F01 class amendment.
**Prior.** Stages 0–5, 6a–6c, 6e, 7, 8, 9, 10a, 10b, 11, 11b all landed. Stage
11b landed `63e0e09`.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## 0 — why this stage waited, and what changed

6d was moved after stage 11 on 2026-10-12 because it could not be built
honestly: RFC 094 `:548` called F01 Protocol-class while the approved design
made it Class A, and the plan parked a gap rather than let a stage be built
against a prerequisite the repository contradicted. That gap is now closed.
RFC 094 `:548` reads, as of `93b4b3e`:

> - F01 is **Class A, folded into RFC 102's L04** rather than a separate
>   Protocol-class transaction: the existing-link/no-MFA promotion's guarded
>   `exchanging → completed` transition and its enabled-provider and
>   same-active-link/user authority checks run *inside* L04's own `db.class_a`
>   transaction, so the attempt completion commits atomically with the session
>   it authorises and `auth.federation.signin.success` stays the Class-A event
>   it already is.

Build against that sentence.

## 1 — what moves, and where it moves to

`sign_in_federated` (L04) is already a `db.class_a` transaction
(`commands.rs:2214-2222`): it inserts the session, evicts over the cap, and
emits `auth.federation.signin.success` through `L04Event::Success`. Its one
caller is `handlers/federation.rs:1804`.

Three things currently sit **outside** that transaction on the existing-link,
no-local-MFA path and must move inside it:

- the guarded `federation_login_attempt` `exchanging → completed` transition;
- the enabled-provider check;
- the same-active-link/user authority check.

The point is atomicity: today a crash between the attempt completion and the
session insert can leave an attempt marked completed with no session, or a
session with the attempt still exchanging. After this stage, one transaction
either does all of it or none.

**Measure the current split yourself before moving anything** — the handler
does some of this work and the `files` list in `contracts/write-commands.toml`'s
F01 row points at `handlers/federation.rs` as the *legacy* implementation, not
an implementation of F01. Say in the package what you found where.

## 2 — what must not change

**`auth.federation.signin.success` stays exactly as it is**: same name, same
Class A, same descriptor, same attributes. The whole reason F01 went Class A
rather than Protocol is that downgrading an already-atomic audit was the wrong
trade. If your change moves that event in any way, stop — that is a finding,
not an implementation detail.

**No new event.** Stage 10b's `auth.federation.mfa_success` is the MFA path's;
this is the no-MFA path and it already has its event.

**No handler rewrite beyond the move.** Stage 12 owns the handler's legacy
paths.

## 3 — the contract row, and the one question I am not deciding for you

`contracts/write-commands.toml`'s F01 row is today `class = "P"`,
`sealed = false`, `status = "target-legacy"`, and its rationale says
*"post-commit `auth.federation.signin.success` is Class B under U24/U30
policy"* — which the amendment makes false. That sentence must go regardless.

**The question**: does F01 keep a row at all, now that it is not a separate
transaction?

- **Keep it**, reclassified `A` and `implemented`, pointing at the expanded
  L04; or
- **Retire it into L04**, the way U06 was retired — the file header records
  *"U06 (admin password reset) has no row: RFC 103 D1 retired it, and U37
  replaced it"*, with the retirement written up in `command-inventory.md`.

Measure both against what G17 actually enforces and argue one. **State the
`sealed` rule back to me from the script rather than from this dispatch** —
`check-write-commands.py:244-248` is the authority, I have mis-stated it once
already this month, and whichever option you choose has to satisfy it rather
than my memory of it.

## 4 — evidence

The atomicity claim is the stage, so it needs the proof this project uses for
atomicity: **an injected commit failure that leaves neither the attempt
completed nor the session inserted**, in the shape
`f03_injected_commit_failure_rolls_back_everything` already uses. A test that
only shows the happy path does not establish what this stage is for.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **What you found where** before moving it — the current split between
   handler and command.
2. **The move**, and the injected-failure rollback test of §4.
3. **Proof the event did not move**: same name, class, descriptor, attributes.
4. **§3's answer**, argued, with the `sealed` rule quoted from the script.
5. **Per-hunk SHA-256** against the tip you named, regenerated after your
   *last* commit.
6. **The full matrix, all 24 green.** G13, G17 and G20 are the ones this moves.
7. **Count reconciliation** from `63e0e09`: `sui-id-core --lib` **201**,
   `sui-id-store --lib` **527**, `sui-id --lib` **560**, `sui-id --test e2e`
   **533**, `sui-id --test compile_fail` **12**, `sui-id-i18n` **19**,
   `scripts/tests` **349**.
8. **Anything wrong with this dispatch.** Your last four rounds corrected me on
   the `sealed` flag, the G15 framing, an RFC citation, a factual error in an
   RFC I wrote, a consequence I asserted without reading a foreign key, and
   the difference between "the exploit is unreachable" and "the function is".
   Keep going.

# RFC 096-B1 stage 10b fix round — a name of its own, and the link nobody checks

**Dispatched.** 2026-10-15 JST, by the architect.
**RFC.** 096 — **Status: Accepted**, re-accepted 2026-10-14.
**First round.** [`stage10b-f03-the-four-guarded-outcomes-2026-10-15.md`](stage10b-f03-the-four-guarded-outcomes-2026-10-15.md);
result in `.git-exclude/reviewed/rfc-096-b1-stage10b-one-name-cannot-carry-two-guarantees-2026-10-15.md`.
**Baseline.** `b33f5d5` — the CHANGELOG catch-up landed alone while this round
was in review, so it has moved. Hash against the new tip and name it.
**Keep the working tree**; it is uncommitted and that is correct.

Four items. Two blocking, and the first of those is my error, not yours.

## 1 — (blocking) F03's success gets a name of its own

My §3 told you to reuse `auth.federation.signin.success`. That instruction is
what produced the failing gate, and it was wrong.

You were right that the registry cannot hold one name with two mechanisms, and
right to refuse the resolution that passes by hiding `L04_SUCCESS`'s descriptor.
What neither of us should have needed a tooling change for is that **the
project already does this, on the local path, with a distinct name**:

| path | no second factor | second factor completed |
|---|---|---|
| local | `auth.login.success` (L03) | **`auth.mfa.success`** (L02, `commands.rs:2854`) |
| federated | `auth.federation.signin.success` (L04/F01) | yours |

The class is a property of the guarantee — Class A cannot be missing if the
mutation happened, Class B can — so one name carrying both leaves an operator
unable to tell which assurance a row carries. The matrix's one-class-per-name
column encodes that; it is not a limitation to route around.

And the RFC does not ask for the reuse. `:816-817` separates *"post-commit
Class-B sign-in success"* from the *observations*, and the closed-vocabulary
sentence at `:826-831` enumerates only F03's three observations. The success
event is unnamed there, so naming it is a design call, and I am making it:

**`auth.federation.mfa_success`**, Class B, one `MustAttemptDescriptor` in the
shape of stage 9's `AUDIT_MFA_REQUIRED_DESCRIPTOR`. Verified free — zero
occurrences repo-wide.

Then: `auth.federation.signin.success` reverts to Class A owned by L04 alone,
its matrix row goes back to what it said before this round, the fourth new row
is `auth.federation.mfa_success` in both the matrix and the reference, and
**G13 goes green** with no tooling change and no amendment. Drop
`F03_PROMOTED_MUST_ATTEMPT`'s name collision with it.

An operator gains something here rather than losing it: a federated sign-in
that required a second factor becomes distinguishable from one that did not,
exactly as on the local path.

## 2 — (blocking) nothing checks the link

`InvalidatedReason` declares seven variants; F03 produces four. Two of the
missing three are correctly deferred — `CeremonyInvalid` needs stage 11's F06,
`ProviderChanged` needs M2b's durable version/generation columns, which stage
10a measured as placeholders at `(0, 0)`.

**`LinkChanged` is not deferred, it is absent.** `federation_link` exists, F04
writes it, C18 can remove it, and this command already calls
`federation_link::touch_last_seen_within_tx` on the `Promoted` path — the only
reference to that repo in the whole command. So an administrator who unlinks a
federated identity inside the 300-second continuation window gets a
`[Fed, Totp]` session created anyway. That is authority drift, and `Invalidated`
is what the RFC provides for it: `contracts/write-commands.toml`'s own F03 row
says *"exact provider/**link**/user/factor/method"*, and `:831` lists
`link_changed`.

Add the check beside the provider and user checks, in the same guarded
transaction, with its own test. In the package, **say which reasons remain
unproducible and why** — the first round listed the four it produces without
distinguishing a gap from a deferral, and that distinction is the whole of this
finding.

## 3 — two wrong RFC citations in stage-7 code

`handlers/federation.rs:1262` cites RFC 096 `:710-711`; `:1279` cites
`:706-712`. Both are wrong — `:706-712` is about `at_hash` and the construction
capability. The rule is **`:727-728`**: *"`provision_on_first_login` requires a
present email and `email_verified=true`."*

Mine, from stage 7, which I reviewed and accepted. The sibling CHANGELOG entry
inherited `:706-712` from the comment and reached a public file with it before I
caught it. Correct both comments here, where `crates/` is already open.

## 4 — the gate goes green

With item 1 done, the full matrix passes. A package does not come back with a
red gate: the bar is every declared gate green on one named commit, and when
that is not reachable the question comes back instead of the tree. You were
right to escalate rather than launder; the resolution simply existed.

## Not in this round

**The `user_id` question stays open.** Two live continuations per user with
independent counters, no `UNIQUE` on `federation_mfa_pending.user_id` — your §2
answered it correctly as a finding with the schema read directly. It belongs
with the F02/F03 pair, not here. Do not add the constraint.

**No WebAuthn, no handler.** Unchanged from the first round.

## What to return

1. **Item 1**, with the matrix row for `auth.federation.signin.success`
   restored to its pre-round text, and G13 green.
2. **Item 2**, with its test, and the reason-by-reason statement of what remains
   unproducible.
3. **Item 3**, both comments.
4. **Per-hunk SHA-256** against the new tip.
5. **The full matrix, all 24 green**, from a throwaway workspace.
6. **Count reconciliation** from your own `08f1c36` figures — `sui-id-core
   --lib` 190, `sui-id-store --lib` 475, `sui-id --test e2e` 533 — and say what
   moved.
7. **Anything wrong with this round.** You overrode my `sealed = true`
   instruction correctly last time and were right; three of this round's
   findings were mine. Keep doing that.

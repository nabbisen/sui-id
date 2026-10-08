# RFC 096-A stage 6c (fix) — pin every bound at its limit, and drop the mandatory dot

**Dispatched.** 2026-10-09 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** Read the tip yourself with `git log -1`, hash against it, name the
full SHA — as you have the last two stages. **Start from your returned stage 6c
tree**, not a clean one: everything in it is accepted except the two items below.
**Review result.** `.git-exclude/reviewed/rfc-096-a-stage6c-optional-claims-and-the-capability-2026-10-09.md`
**Scope.** Seven tests added, one condition removed. Nothing else.

## Item 1 — seven bounds, none pinned at its limit (yours)

I tightened every numeric bound in `optional_claims.rs` by one, all seven at
once, and **all 487 tests passed**:

| Bound | Mutated to | Tests that failed |
|---|---|---|
| `email` 254 bytes | 253, and `>` → `>=` | none |
| display byte bound (512 / 1024) | one lower | none |
| display scalar bound (128 / 256) | one lower | none |
| `amr` count 16 | 15, and `>` → `>=` | none |
| `amr` element `1..=64` | `1..=63` | none |
| `acr` `1..=256` | `1..=255` | none |
| `at_hash` `1..=256` | `1..=255` | none |

Every test exercises only the rejecting side. Nothing asserts that the exact
limit is **accepted**, so any of these could ship one too tight.

**This is not a cosmetic bound.** RFC 096 `:679-681` makes a present-but-invalid
optional claim reject the **whole token**, so a bound one too tight does not
truncate a display hint — it fails the login. A 256-byte `acr` under a
`1..=255` bound would end the session attempt, over a claim `:670` describes as
*"ignored for authority and not persisted"*.

**Add one accepting-side test per bound, at the exact limit**, in the
`N accepted / N+1 refused` shape this RFC already uses — 6a's
`nine_audiences_are_refused_eight_are_accepted`, and every one of 6b's time
boundaries. Seven tests. Then re-run my mutation above and show it killing them.

You set this standard in 6b, when a mutation that killed nothing was treated as
a finding about the tests rather than a pass. That is exactly what this is.

## Item 2 — the mandatory dot in the domain (mine)

**Remove `domain.contains('.')` from `is_mailbox_shaped`. Keep every other
condition**, including no leading or trailing `.` and no `..` — those are
malformed on any reading and only apply when a dot is present.

You flagged the mailbox grammar as your own interpretation with nothing to
anchor it, and asked. Answering: a dotless domain must be accepted.

- `:664` — `email` is *"metadata only"*.
- `:693-695` — email is *"never a lookup key"*, and *"An existing link may
  authenticate without email."*
- `:709-710` — provisioning needs `email_verified=true`; absent or unverified
  denies, quietly.
- `:679-681` — yet a present-but-invalid optional claim rejects the token.

So `user@intranet` or `user@localhost` — dotless, unusual, entirely plausible
from an enterprise or internal IdP — would fail the login outright over a claim
that grants nothing and that the RFC is content to see absent. Being permissive
costs an odd string in a metadata column; being strict costs a user their
account access. Requiring a dot buys no security, because the claim carries no
authority either way.

Update `is_mailbox_shaped`'s doc comment to record the ruling and why, so the
next reader does not re-add the condition as an obvious improvement. Add a test
that a dotless domain **is** accepted, and keep the three existing shape
rejections (no `@`, two `@`, and now a leading/trailing-dot or `..` case in
place of the no-dot one).

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The seven accepting-side tests**, mapped to their bounds in a table.
2. **My mutation re-run**, showing it now kills them — ideally all seven at
   once, as I ran it.
3. **The mailbox change**, with the dotless-accepted test and the reworded doc
   comment.
4. **Per-hunk SHA-256** against the tip you named.
5. **Gate evidence** from a throwaway clone with its own `target/`.
6. **Anything you think is wrong with this.** If you think a dotless domain
   should be refused, say so — I have given my reasoning and it is a judgement
   about lockout risk, not a measurement, so it is arguable in a way the bounds
   are not.

**Accepted as-is and not to be touched:** the two-module split, the
byte-before-scalar ordering and its derivation, the bidi constants, the
capability's fields and sealing, the hand-written redacting `Debug` extended to
`sub` and `verified_email`, `verified_email` as `Option<String>`, and the
deletion of `email()`/`email_verified()`. `sub()` stays for now.

**Not 096-A's closure.** Stage 7 (the nonce rule), 8 (discovery) and 9 (the
corpus rows) remain.

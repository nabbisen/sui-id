# RFC 096-A stage 6a (fix) — a duplicate payload member must be refused by its own name

**Dispatched.** 2026-10-08, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** `ca613d7`, with stage 6a's returned working tree in place. **Do
not start from a clean tree** — this builds on the package you handed over,
which is accepted apart from this one point.
**Review result.** `.git-exclude/reviewed/rfc-096-a-stage6a-required-identity-claims-2026-10-08.md`
**Scope.** One change, and it is net-negative in lines. The four claim
validators are accepted as written; do not revisit them.

## The defect

A repeated `iss`, `sub` or `aud` member is refused with
`VerificationError::SignatureInvalid`, on a token whose signature is valid. Your
own test helper says *"a genuinely signed, structurally valid token must
verify"* — and it is right, which is what makes the error false rather than
merely vague. An operator reading it goes and looks at keys and algorithms.

Your finding that `jsonwebtoken`'s `ClaimsForValidation` already rejects those
three is correct, and I verified it. Not writing a dead check was the right
call. The conclusion to revise is only *where* the live check belongs.

## The change

**Move the duplicate scan into `verify_id_token_against_jwks`, before
`jsonwebtoken::decode`**, over `compact.payload` — already decoded, already
bounded by stage 2 — using the same `MemberNames` technique. Reject **any**
repeated top-level member with a new
`VerificationError::DuplicatePayloadMember(String)`.

It must run before `decode`, not after: `decode` fails first, which is the
defect.

I prototyped this and it works; all four duplicate cases are then caught in one
place, each named, with the other 22 tests untouched. The prototype's output is
quoted in the review result.

**Then delete what it obsoletes.** This is the part that matters as much as the
move:

- `VerifiedIdTokenClaims::raw_payload` (the field) and `raw_payload()`.
- `reject_duplicate_azp`.
- `IdentityClaimsError::DuplicateMember`, now unreachable.
- `MemberNames` reverts to private — drop the `pub(crate)` on the struct and
  its tuple field, and restore the original doc comment.
- The `verified_id_token_claims_cannot_be_constructed_directly` fixture and its
  pinned `.stderr` revert to their `ca613d7` form, since the second field goes.

**Why the deletions are required and not optional.** RFC 096 `:687-689` says
successful validation yields a capability carrying *"no raw token, nonce, or
upstream access token"*. `raw_payload` is the entire decoded payload, nonce
included, held for the life of the value, and `VerifiedIdTokenClaims` derives
`Debug`, so it reaches any `{:?}`. Stage 6c has to remove it; adding it now and
removing it later is strictly worse than never adding it. Scanning at the point
of parse keeps the bytes exactly as long as they are needed.

## Tests

Rename the three `..._one_layer_earlier_by_jsonwebtoken_itself` tests — the
premise is no longer true — and assert the specific variant:

- `a_repeated_iss_member_is_refused_by_its_own_name`
- `a_repeated_sub_member_is_refused_by_its_own_name`
- `a_repeated_aud_member_is_refused_by_its_own_name`
- `a_repeated_azp_member_is_refused_by_its_own_name` (already exists; retarget
  it from `IdentityClaimsError::DuplicateMember` to the new variant)

Each asserts `DuplicatePayloadMember` with the member's name in it, not
`is_err()`, and not `matches!` with a wildcard that would pass for any name —
check the string.

Add one test that a **non-adjacent** repeat is caught
(`{"iss":"a","sub":"s","iss":"b"}`), matching what stages 2 and 3a already do
for the header and the JWKS document. The scan is positional, so this is the
case a naive "compare neighbours" implementation would miss.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The four renamed tests**, and the non-adjacent one, mapped in a short table.
2. **Mutation evidence** that the scan is load-bearing: gate the comparison
   behind `false &&` and show all four (five, with the non-adjacent case)
   failing, and the rest passing.
3. **Confirmation of the six deletions**, with the diff against `ca613d7`
   demonstrably smaller than the package you just handed over — say the line
   counts both ways.
4. **Per-hunk SHA-256** from `python3.14 scripts/hunk-hashes.py --baseline ca613d7`,
   verbatim.
5. **Gate evidence** from a throwaway clone under `.git-exclude/tmp/clones/`
   with its own `target/`, as before.
6. **Anything you think is wrong with this.** You were right about
   `ClaimsForValidation` and right not to keep dead code; if the move has a
   consequence I have not seen, say so instead of implementing it.

**Still not 096-A's closure.** Four of nine categories were open before this
stage and this stage closes at most the issuer one, as your package correctly
said. Stages 6b, 6c, 7, 8 and 9 follow.

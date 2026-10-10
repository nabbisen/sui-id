# RFC 096-B1 stage 4 — code exchange, and 096-A stops being dormant

**Dispatched.** 2026-10-10 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
authorized 2026-10-09. This is its stage 4.
**Prior.** Stages 0–3 complete. `validate_nonce` has a production caller; the
other four validators and the capability constructor still have **none**.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## `handlers/federation.rs` is editable from this stage — my earlier reason was wrong

Stages 1, 2 and 3 were each told *"do not touch `handlers/federation.rs` — still
forbidden; stage 7 is what earns it."* **The second clause was my error.**

The constraint is **096-A's**, not a stage gate: `:63-66` forbids 096-A any
durable mutation and that file holds the live callback. 096-B1's purpose is the
opposite — `:72-73`: *"The defect is closed when the live callback is routed
through that verification."* So the file was withheld from 096-A, not until
stage 7. "Do not touch it" was right for stages 1–3 because none of them needed
it; the reason I attached was not.

**This stage edits it.** Stage 7 remains the *removal* of the two old paths, not
the unlocking of the file.

## What this stage is

**Every remaining 096-A validator gains its first production caller here**, which
is why the plan says this stage needs the heaviest review of the nine. Today
`identity_claims`, `time_claims`, `optional_claims` and `identity_capability`
have **zero** non-test callers — measured again before writing this.

Route the callback through, with the expected values taken from the claimed
attempt row rather than from anywhere else:

| Validator | Expected value, and where it comes from |
|---|---|
| `validate_identity_claims` | configured issuer and client ID for `provider_id` **as recorded on the attempt row**, not re-read from live config |
| `validate_time_claims` | `created_at` from the row, as the attempt binding `:676` requires |
| `validate_nonce` | already wired in stage 3 — reuse it, do not duplicate |
| `validate_optional_claims` | no expectation; bounds only |
| `construct_identity_capability` | the `CacheKey` triple from the row, plus a validation time |

**Why "from the row" matters and is not a style preference.** The row carries
`provider_config_version` and `provider_activation_generation` precisely so a
superseded attempt is identifiable (`:580-581`). Re-reading live provider config
at callback time would validate against whatever the config says *now*, which
defeats that. If the live config has moved on, the attempt is superseded and must
fail — not silently validate against the new values.

**Also in scope: the token-response envelope.** The corpus row stage 5 and stage
9 of 096-A both left to this stage — *"mandatory ID token ≤16 KiB; bounded
ignored tokens; missing/duplicate/oversize ID token; malformed JSON."*
`read_bounded_json` already parses `TokenResponse` on this path; what is missing
is the ID-token-specific bounds. **Note the measured fact from 096-A stage 8's
review**: a conforming ID token at RFC 096's own claim bounds is ~3,023 bytes, so
a 16 KiB cap is generous and the shared `MAX_STRING_LEN` is 8 KiB — check which
binds first and say so.

## Not in scope

**No identity mapping and no session.** `(provider_id, sub)` lookup is stage 5;
establishing a session is stage 6. This stage validates and builds the
capability. **What it does with the capability is: nothing yet** — hold it, and
let stage 5 consume it.

**Do not remove the old paths.** The trust-on-TLS `decode_id_token_claims` call
and the cookie-replay path stay until stage 7. That means a transitional state
where both exist; make the new path authoritative for validation and leave the
old one's removal alone.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **A table of the five validators and the production call site of each**, so
   "no longer dormant" is checkable rather than asserted. I will verify it by
   grepping for non-test callers, as I have each stage.
2. **A test per expected value showing it comes from the row**, not from live
   config — specifically, one where live config has moved on and the attempt
   therefore fails.
3. **The token-response envelope bounds**, with which cap binds first measured
   rather than assumed.
4. **Whether any validator's signature fought the call site**, and what you did.
   If one of 096-A's parameter shapes turns out wrong under a real caller, that
   is a finding about 096-A and I want it plainly — those five functions have
   never been called in production and this is the first time anything has
   pressed on them.
5. **Mutation evidence** per routed validator: disable each and show which test
   notices. A validator wired but unasserted is the failure mode here.
6. **Per-hunk SHA-256** against the tip you named.
7. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace` and **G17** if you add or change a command row.
8. **Anything you think is wrong with this dispatch.** Four of the last five
   packages corrected something of mine, including the file-constraint error
   above, which I found only by checking before writing this.

**Not 096-B1's closure.** Stages 5–8 carry mapping, session, removal and live
evidence.

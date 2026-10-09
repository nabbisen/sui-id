# RFC 096-A stage 8 (fix) — pin the shared caps, and enforce canonical endpoint URLs

**Dispatched.** 2026-10-09 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** **Start from your returned stage 8 tree**, not a clean one.
Everything in it is accepted except the two items below. Read the tip with
`git log -1`, hash against it, name the full SHA.
**Review result.** `.git-exclude/reviewed/rfc-096-a-stage8-the-discovery-profile-2026-10-09.md`
**Do not restore** the IP-literal or reserved-hostname checks. Keeping them
reverted is confirmed, and the review result explains why your reasoning was
sounder than the test-cost argument you led with.

## Item 1 — nothing pins which caps the shared transport path passes

Your parameterisation is the right design and I am keeping it. It also creates a
new way to be wrong that no test catches. I replaced one line:

```rust
check_caps(&value, &ResponseCaps::DISCOVERY)?;   // was JWKS_AND_TRANSPORT
```

and the entire workspace stayed green — lib 536, e2e 496, `compile_fail` 6,
everything.

**Why that is not theoretical.** `read_bounded_json` is the live parser for the
token response (`federation.rs:392`, into
`TokenResponse { access_token, id_token }`) and userinfo (`:748`). An `id_token`
is a single JSON string, and RFC 096's own matrix permits `name` up to 1,024
UTF-8 bytes (`:668`) and `preferred_username` up to 512 (`:667`). A conforming
token carrying those, with the required claim set and an RS256 signature:

```
RS256/2048: payload JSON 1971B -> id_token string 3023B
RS256/4096: payload JSON 1971B -> id_token string 3364B
```

Both over 2,048. So the mix-up would make us reject ID tokens that **RFC 096
itself declares valid**, and the failure would look like a provider problem.

**Add the pin on the side that carries live traffic.** `DISCOVERY` is already
pinned by your two "4x tighter than JWKS" tests; the transport side has no
equivalent. Two tests, asserting `read_bounded_json` **accepts**:

- a document containing a string longer than 2,048 bytes and shorter than
  `MAX_STRING_LEN`;
- a document containing an array of more than 32 and fewer than
  `MAX_ARRAY_LEN` elements.

Then re-run my mutation and show it killing both.

If `read_bounded_json`'s signature makes that awkward to test directly, say so
and pin it at whatever seam is honest — I care that the caps choice is
load-bearing in a test, not which door the test comes in through.

## Item 2 — `:553`'s "noncanonical URL" is unenforced

`check_endpoint` returns `value`, the raw input string. Measured against `url`
2.5, all three of these pass validation today and are kept in their
noncanonical form:

| Declared endpoint | origin allowed | canonical form |
|---|---|---|
| `https://idp.example.com:443/token` | yes | `https://idp.example.com/token` |
| `https://IDP.EXAMPLE.COM/token` | yes | `https://idp.example.com/token` |
| `https://idp.example.com/a/../token` | yes | `https://idp.example.com/token` |

RFC 096 `:553` lists *"noncanonical URL"* among the conditions that fail the
whole document. The origin check itself is sound — it compares the parsed origin
— but the string that survives validation is the unnormalised one, so what we
store and log can differ from what we fetch.

**Compare the endpoint to its canonical serialisation and refuse on difference**,
with its own error variant, alongside the `HasUserinfo`/`HasQuery`/`HasFragment`
family you added.

**One edge is yours to decide and state, not mine to prescribe.**
`url::Url::parse("https://idp.example.com")` serialises as
`https://idp.example.com/`, with a trailing slash — so a bare-origin endpoint
declared without one would count as noncanonical under a naive string compare.
Decide whether that is a refusal or a normalisation, and give the reasoning.
Every endpoint in the RFC's three rows carries a path, so this is about the
unusual case; I would rather you choose with the trade-off written down than
inherit a rule from me that turns out to reject something real.

Tests: each of the three forms above refused by name, the trailing-slash case
decided either way with a test pinning your decision, and the existing
`a_canonical_token_endpoint_is_accepted` still passing.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The two caps-pinning tests**, and my mutation re-run against them.
2. **The canonical-URL check**, its new error variant, the three refusals, and
   your trailing-slash decision with reasoning.
3. **Mutation evidence** for the canonical check — gate it and show exactly
   which tests fall.
4. **Confirmation the live path is still behaviourally unchanged**: `JWKS_AND_TRANSPORT`
   numbers untouched, `read_bounded_json`'s public signature untouched,
   `handlers/federation.rs` still showing an empty diff.
5. **Per-hunk SHA-256** against the tip you named.
6. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace` — which is what found both of this stage's real
   problems, yours and mine.
7. **Anything you think is wrong with this.** The trailing-slash question is
   genuinely open, and if you think comparing against the canonical form has a
   consequence I have not seen, that is worth more than implementing it.

**On the closure question you raised:** you were right, and it is recorded. Nine
of the eleven metadata rows go live with this stage; the two cross-checks and
the tightened bounds do not, because `validate`'s only non-test caller is in a
file 096-A may not touch. Discovery's category closes at the validation layer
and not on the live path, and the closure assessment will say so rather than
claim otherwise. **Stage 9** — the corpus rows stage 5 could not cover — is the
last stage, and it comes after this fix.

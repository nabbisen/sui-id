# RFC 096-A prerequisite — the preparatory `federation.rs` split

**RFC status: 096 Accepted.** This is one of 096-A's four implementation
prerequisites: *"the preparatory `federation.rs` split committed and
independently reviewed."* It has been named since 2026-07 and **specified
nowhere** — no handoff, no design. This is that design.

**Dispatched.** 096-A itself is **not** dispatched and this does not start it.

## Why the split exists, which is not "the file is big"

`handlers/federation.rs` is 937 lines, and that alone would be a weak reason.
**The real reason is where 096-A's code would otherwise land.**

`federated_callback`'s ninth phase is, verbatim from the code: *"Decode the ID
token claims (light validation — nonce check + sub extraction)"* — about 15
lines. **096-A replaces that with JOSE signature verification, a JWKS cache with
rotation, full claims validation, and one-time nonce consumption.** Dropped in
place, that is several hundred lines added to a 937-line file, inside a handler
that is already 362 lines of sequential error handling.

So the split is not cosmetic and it is not about ROADMAP R3's line count. **It is
about giving 096-A a module to land in.**

## The pattern already exists — follow it, do not invent one

RFC 134 established it three times in the last three days:
`http/discovery.rs` (365 lines), `http/response_bounds.rs` (476),
`http/dynamic_registration_validation.rs` (1120). In each,
**pure logic that can be unit-tested without I/O moved to `http/<topic>.rs`,
while the fetch and the handler flow stayed in the handler.**

`fetch_discovery` is the model: it does the I/O and delegates validation to
`crate::discovery`. **Do the same thing to the rest of the file.**

## What moves

| To | What | From |
|---|---|---|
| `http/federation_state.rs` | `FedState`, `seal_state`, `unseal_state`, `hmac_state`, and the three consts | `:30-93` |
| **`http/id_token.rs`** | `IdTokenClaims`, `decode_id_token_claims` | `:317-328`, `:858-866` |
| `http/federation_identity.rs` | `derive_username`, `resolve_shadow_username` | `:808-857` |

**`http/id_token.rs` is the point of the exercise.** Today it is about 20 lines.
It is the designated home for 096-A's signature verification, claims validation
and nonce consumption, and the dispatch for 096-A will say so. **Create it even
though it is nearly empty** — a module that exists is where the next author
puts things; one that does not, is not.

## What stays, and why

`handlers/federation.rs` keeps the three route handlers, `fetch_discovery`,
`fetch_userinfo`, `complete_federated_signin` and `emit_audit_soon`. **These are
I/O and orchestration.** Splitting a sequential flow into phase functions across
files makes it harder to read, not easier, and the flow is the thing a reviewer
most needs to follow end to end.

**This will not take `federation.rs` under 500 lines**, and it should not be
sold as doing so. It removes roughly 135 lines. **The measure of success is that
096-A adds its code to `id_token.rs` and not here.**

## Constraints

- **Behaviour-preserving. No logic changes at all.** If a move tempts a fix,
  stop and report it — a refactor that also fixes something is a refactor
  nobody can review.
- **`is_redirect_uri_registered` and `redirect_uri_matches` are not touched**;
  they are in `sui-id-core`, not this file.
- Every moved item keeps its doc comments verbatim, including the RFC
  references in them.
- The existing e2e tests must pass **unchanged**. Any test needing an edit means
  the move was not behaviour-preserving.

## Tests

No new tests. **The proof is that the existing suite passes unchanged** — 486
e2e and the `sui-id` lib tests — and that the diff is moves plus imports.

Say in the package **how you confirmed it is a pure move**: the most convincing
evidence is a normalised diff of each moved block showing the body is identical.

## The prerequisite's other half

096-A's prerequisite says *"committed **and independently reviewed**"*. **The
only reviewer available is the architect, who wrote this design** — the same R1
limitation on every review in this programme. I will review it and record the
non-independence. RFC 000 places that approval with him where no independent
role exists — as does RFC 129's re-scoping of 2026-10-01, which replaced
"independent" with a person who exists.

## Return

Per-hunk SHA-256 against a stated baseline, the pure-move evidence, and the full
local gate set — **including A3.2, G19 and G20**.

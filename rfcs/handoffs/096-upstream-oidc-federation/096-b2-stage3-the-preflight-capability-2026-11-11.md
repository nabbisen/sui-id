# RFC 096-B2 stage 3 — the policy digest, and the preflight capability

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S3 —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-11, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, §096-B2. Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S2b
must be in it.
**Read first:**
- RFC 096 `:403-437`: preflight, then the digest. §A and §B below turn those
  lines into a checklist; build against both;
- `architecture.md` `:39-49`;
- RFC 094 `rfcs/done/094-transactional-audit-registry.md:506-512`;
- `crates/sui-id-store/src/federation_policy.rs`;
- `crates/sui-id/src/http/identity_capability.rs`, for the sealing pattern
  and its `trybuild` fixtures (`crates/sui-id/tests/compile_fail/`).

**Not in this stage: the consumer.** C17's enable is S4, and how it consumes
the capability across the crate boundary is a question still open:
[`096-b2-c17-consumes-the-capability-decision-2026-11-11.md`](096-b2-c17-consumes-the-capability-decision-2026-11-11.md).
This stage builds the capability and **no caller of its contents**. If S3
seems to need one, stop and report.

## A — the policy digest (RFC 096 `:424-437`), as a checklist

In `federation_policy`, the *"private policy module"* (`:435`). **Each row
is built here.**

| # | Clause | Note |
|---|---|---|
| 1 | SHA-256 over the domain `sui-id/federation-provider-policy/v1\0` | the NUL is part of the domain |
| 2 | Fixed order: issuer, client ID, sealed-secret-envelope SHA-256 *"(explicit absent/present tag)"*, token-auth-method tag, scopes, ID-token algorithms, endpoint origins, HTTPS ports, provision mode | one field tag per field, **1 to 9 in this order**. That numbering is the architect's choice; the RFC does not give one. Record it in the code |
| 3 | *"Each scalar is encoded as a one-byte field tag, a big-endian `u32` byte length, and exact canonical bytes"* | |
| 4 | *"lists use their field tag, a big-endian `u16` item count, and independently length-prefixed canonical items sorted by the closed enum order or canonical bytes"* | scopes and algorithms in closed-enum order; origins by canonical bytes, as the **effective** set, so `""` means the issuer's origin |
| 5 | *"ports are sorted big-endian `u16` values"* | |
| 6 | *"Empty and absent are distinct"* | absent secret ≠ present-with-empty; test it |
| 7 | *"Enabled/review status, both counters, evidence, and display labels are excluded"* | a test changes each one and shows the digest unchanged |
| 8 | *"computed only by the private policy module from typed durable/proposed policy"* | computed from a stored row (decoded through `from_stored`, plus its stored envelope). No other module hashes policy |
| 9 | *"compared as exactly 32 bytes in constant time"* | a `PolicyDigest` newtype over `[u8; 32]`, with a `ct_eq` method and no `PartialEq` derive |
| 10 | *"never accepted from a caller, serialized, logged, audited, or exposed through metrics"* | no `Debug`, `Display`, `Serialize` or public constructor. Bytes leave only through a `pub(crate)` accessor, for S4's evidence column, which is the store persisting its own value |

**A known-answer vector, computed independently.** Write a short
`python3.14` script, `hashlib` only, that builds the encoding byte by byte
from §A and prints the digest for one fixed provider. Pin that digest in a
Rust test, and put the script in the return. **Two implementations of one
encoding agreeing is the evidence that the encoding is what §A says.** One
implementation compared with itself proves nothing.

**More tests:**
- changing each of the nine fields changes the digest;
- reordering origins does not;
- **resealing the same secret does change it** (the envelope hash). That is
  the design, and the reason S2b does not reseal an unchanged secret.

## B — the preflight (RFC 096 `:409-421`), as a checklist

In `sui-id`, a new module next to `identity_capability.rs`.

| # | Clause | Note |
|---|---|---|
| 1 | *"permitted only for a non-deleted, fully configured, disabled provider"* | "fully configured" means `from_stored` succeeds **and** a `[[federation_providers]]` block names it. Otherwise refuse **before any network I/O** |
| 2 | *"captures version, generation, and policy before network I/O"* | the policy is captured as its digest |
| 3 | *"Successful production transport/metadata/JWKS validation"* | **through the production functions**, with the egress client the callback uses (`app.http_client`). No second validation path. See §C |
| 4 | Compatibility, which is what *"proves current reachability/config compatibility"* means (`architecture.md:40`) | the discovery must support the policy's token-auth method (`supports_token_endpoint_auth_method`, as `federation.rs:954` does), and `id_token_algs_intersection` with the policy's algorithms must be non-empty. The JWKS must parse under `parse_jwks`'s rules |
| 5 | *"after all validation, re-reads them and the disabled state before its private constructor can return a capability"* | version, generation, digest and disabled are all equal, or no capability |
| 6 | Contents: *"provider ID/version, the exact disabled-state activation generation, a digest of the complete durable policy, trusted `observed_at`, and bounded validated metadata/key-set fingerprints"* | `observed_at` comes from the app's clock (`SharedClock`), read **once**, after validation and before the re-read. **The fingerprints are SHA-256 over the exact response bytes that passed validation**, discovery and JWKS, 32 bytes each, matching 0050's evidence columns |
| 7 | *"private, non-cloneable, non-debuggable, non-serializable"* | four `trybuild` fixtures: cannot be constructed, cloned, `Debug`-printed or serialized. Use the federated-MFA ones as the pattern |
| 8 | *"Failure or cancellation returns no capability and performs no write"* | the preflight writes nothing in any case. A test counts audit rows and the provider row before and after a success and a failure |

**Errors.** One closed enum of refusal reasons, in operator terms: not
disabled, not fully configured, transport, discovery, JWKS, auth method
unsupported, no common algorithm, and *changed during preflight*. S5 will
show these to an administrator. **No upstream body, URL query or key
material in any variant.**

## C — sharing the production fetch, not copying it

- `fetch_discovery` is private to `handlers/federation.rs` (`:87`). **Move**
  it to a shared module that both the callback and the preflight call,
  without changing its behaviour. The callback's tests must stay green
  unchanged.
- **The fingerprints need the bytes.** `fetch_discovery` and
  `jwks::fetch_jwks` (`jwks.rs:92`) return only parsed values. Give each one
  a form that also returns the exact bytes it validated, with the existing
  form built on top of it. **Do not read the body twice, and do not
  validate twice.**
- `discovery::validate_issuer` (`discovery.rs:511`) checks only the scheme,
  but S2a's `federation_policy` has the full issuer rule. The preflight's
  issuer comes from a stored policy that already passed it, so this stage
  changes nothing there. **Note it in the return:** two issuer checks exist,
  and the issuer-host-rule stage should collapse them into one.

## D — tests

Use the existing mock upstream (`crates/sui-id/tests/e2e/tls_mock.rs`,
`serve_https`) and the federation harness.
- **Success:** a capability whose accessors return the row's id, version,
  generation and digest, and fingerprints equal to SHA-256 of the bytes the
  mock served.
- **One test per refusal**, and each refusal that comes before network I/O
  must prove no request reached the mock.
- **The race, clause 5.** The mock's JWKS route bumps the provider's
  `activation_generation` in the database while it serves, so the final
  re-read sees a change. That must refuse with *changed during preflight*.
  **Mutation:** skip the re-read, and quote the test going red.
- **An enabled provider never gets a capability** (RFC 096 `:421-422`).

## E — records and constraints

- No contract rows: nothing here writes. G23 stays green.
- `CHANGELOG.md` needs no entry, because nothing reaches an operator yet.
  Say so in the return.
- **No tag, no dispatch.** Run `bash scripts/local-matrix.sh --changed`, the
  complete matrix, and quote the scope line and any failing check names.
- **Don't edit files while the matrix reads them** (S2b's own lesson).
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage3-the-preflight-capability-2026-11-11.md`,
with:
- the baseline;
- **§A's and §B's tables filled in**: each clause, the `file:line` that builds it, and its test;
- the Python known-answer script, its output, and the Rust test pinning it;
- the moved and split fetch functions, with the callback's tests named as unchanged;
- each refusal's test, and for the pre-network ones how "no request" is shown;
- the race test and its mutation;
- the four fixtures;
- the issuer-check note.

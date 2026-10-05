# RFC 095 M3 stage 1 — the request envelope and the redirect profile

**RFC status: Accepted** (`rfcs/accepted/095-dynamic-client-registration-transaction.md`).
**Entry gate: open.** All six conditions met; condition 4 was discharged and the
discharge approved by `@nabbisen` on 2026-10-05. Implementation may start.

**Dispatched: stage 1 only.** Stages 2 and 3 are **not** dispatched. The
normative source is
[`metadata-validation.md`](metadata-validation.md); stage 1 covers its
**Envelope and member policy** section and its **Redirect corpus**.

## The decision that shapes this stage

**Do not tighten `validate_redirect_uri`. Write a dynamic-registration
validator beside it.**

Measured: `crates/sui-id-core/src/identity/admin/clients.rs:324-346` is called
from **five administrator-client sites** (`:51, :54, :151, :192, :241`) and from
dynamic registration (`dynamic_register.rs:138`). **Its own tests assert that
`http://localhost:8080/cb` is accepted** (`:362`), and RFC 095's matrix requires
`http://localhost:49152/cb` to be **rejected** — only *numeric* loopback is
permitted, and only for a public-native profile.

So the matrix's rules are **not** a stricter version of the current function;
they are a different contract that happens to overlap. Tightening the shared
function would change administrator-created client behaviour, which RFC 095 does
not govern, and would break its tests for the right reason in the wrong place.

**What today's function actually does**, so the gap is not guessed at: scheme
must be `https`, or `http` with host in `localhost | 127.0.0.1 | [::1] | ::1`;
fragment rejected. Nothing else. No profile, no port rule, no userinfo check, no
punycode rule, no per-profile loopback distinction.

## 1a — The derived closed profile

**The type does not exist** — `ConfidentialHttps`, `PublicHttps` and
`PublicNativeLoopback` appear nowhere in `crates/` (grep: zero hits). Build it.

A registration request derives **exactly one** profile from its
`token_endpoint_auth_method` and its `redirect_uris`, and the profile is
**closed**: every redirect in the list must belong to the same one, or the
request is rejected. The matrix's 22 cases are the specification; the ones that
decide the shape:

- `none` + numeric loopback → `PublicNativeLoopback`, **PKCE required**;
- `none` + HTTPS → `PublicHttps`, **PKCE required**;
- Basic/POST + HTTPS → `ConfidentialHttps`;
- **Basic/POST + any HTTP loopback → reject** — the profile is not a property of
  the URI alone, which is why a per-URI helper cannot express it;
- **mixed HTTPS and HTTP-loopback list → reject**, because the profile is
  closed.

**`http://localhost:...` is rejected and numeric loopback is accepted.** That
distinction is deliberate and is the single most likely thing to be "fixed" by a
later reader: a name resolves through DNS and can be made to point anywhere, a
literal cannot.

**Port rules:** loopback without an explicit port, or with port zero, is
rejected. A registered numeric-loopback redirect matches a request-time redirect
differing only in port — **for `PublicNativeLoopback` only**. No other profile
gets port flexibility.

## 1b — The envelope

Per the matrix's first section. Note the bounds differ from RFC 134's response
bounds and are not interchangeable: **≤64 KiB, depth ≤16, top-level members
≤128.**

**Depth ≤16 is yours to enforce.** `serde_json` bounds depth at 128
(`src/de.rs:63`), which is eight times looser. Do not rely on it.

**Duplicate members are rejected.** serde's derived `Deserialize` already errors
on a duplicate *declared* field, but **not on a duplicate unknown one** — and
the matrix says `duplicate member`, unqualified. State which mechanism covers
which case, and close the gap if one remains.

**Unknown extension members are ignored and must not be persisted or echoed;
known-but-unsupported RFC/OIDC members are rejected.** Those two need a list to
tell them apart — the list is the contract, so write it down where the code is.

**`software_statement` gets its own error**, `unapproved_software_statement`,
not `invalid_client_metadata`.

**The bearer token is exactly one 64-character lowercase hex string.** Missing,
oversized, non-hex, uppercase, multiple, or wrong-scheme all take the **same
public invalid-token path** — the response must not distinguish them.

## Ordering, already satisfied — do not rebuild it

The matrix says *"Reject means before token consumption."* **C15 already
guarantees this**: `dynamic_register.rs` validates first and transacts second,
with rollback proven by an injected-failure test. Add your validation to the
existing validate-first section; **do not move the `consume` call**.

## Tests

The matrix is the corpus and is not negotiable down. Every row of **Envelope and
member policy** and every one of the **22 redirect cases** gets a test.

- **Both halves of each distinction**, not just the rejections: `localhost`
  rejected *and* `127.0.0.1` accepted; loopback-without-port rejected *and*
  with-port accepted. A test suite that only proves rejections passes when
  everything is rejected.
- **The profile is closed** — a mixed list rejected, and each uniform list
  accepted as the right profile.
- The removal check per control, as in your last four packages.

## Return

Per-hunk SHA-256 against a stated baseline, the removal evidence, and the full
local gate set — **including A3.2, G19 and G20**.

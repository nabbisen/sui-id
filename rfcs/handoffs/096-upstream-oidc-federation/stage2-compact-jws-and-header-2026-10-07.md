# Developer Handoff — RFC 096-A stage 2: compact-JWS structure and header hygiene

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`; a
first-person memory saying otherwise may have been written by the other agent
in a shared store.

**On pushing:** `.git-exclude/rules/project-instructions-general-common.md:44`
authorizes **both** roles to commit and push. This dispatch asks only that you
**hand the tree over uncommitted**, so the structural rules can be verified
before they land — scope for one dispatch, not a limit on your authority.

**Clone location — my omission last time, now stated.** `ci-gate.sh` refuses a
`CARGO_TARGET_DIR` outside `--root`, so the build lands inside the clone. **Put
the clone under `.git-exclude/tmp/clones/`, on `/home`.** `/tmp` is a 30 GB
tmpfs shared with another project and a workspace build fills it; that is what
cost you a gate run in stage 5, and the cause was my guard, not your method.

## RFC

**`rfcs/accepted/096-upstream-oidc-federation-validation.md` — Accepted.**
Stage 1 landed in `9baf106`. **This is stage 2 of five.**

## The constraint that shapes this whole stage

**096-A must not change what production does.** The RFC is explicit
(`:68–73`): *"096-A does not, on its own, close the shipped defect… Until then
096-A is a reviewed library that no production request reaches."* Routing live
traffic through the new verification is **096-B1**, which additionally requires
RFC 094 M2a.

There is a live caller today: `crates/sui-id/src/http/handlers/federation.rs:404`
calls `decode_id_token_claims`, which decodes claims **without verifying
anything**.

> **Do not change `decode_id_token_claims`, and do not touch
> `handlers/federation.rs`.** Everything in this stage is **new code beside
> it**, reached only by tests. Hardening the existing function is the obvious
> instinct and it is out of scope — it would change live behaviour in a stage
> the RFC forbids from doing so.

## Scope — structural rules only, no signature, no network

From RFC 096 `:626–640`.

### A. Compact serialization

| Rule | Reject when |
|---|---|
| exactly three segments, all nonempty | not exactly two `.` separators, or any segment empty — note a **detached payload** shows up as an empty middle segment, and an **unencoded payload** (`b64:false`) is rejected via the header rule below |
| decoded size at most **16 KiB** | the decoded header + payload exceeds it. Check **before** parsing JSON, not after |
| JWE rejected | a compact JWE has **five** segments — the three-segment rule covers it, but say so in a test rather than leaving it implied |
| general/flattened JSON serialization rejected | the input starts with `{` — it is a JSON object, not a compact token |
| base64url is **unpadded** | a `=` anywhere in a segment, or any character outside the base64url alphabet |

### B. Protected header

| Member | Rule |
|---|---|
| `alg` | present; a string. **Do not check the value here** — the allowlist is config's and stage 4's job |
| `kid` | **required**; 1–128 bytes; **visible ASCII only** (0x21–0x7E) |
| `jku`, `x5u`, `jwk`, `x5c`, `crit`, `b64` | **present ⇒ reject**, each with its own error so an operator can tell which |
| `cty` | **present ⇒ reject** |
| `typ` | absent, or exactly `JWT` — **case-sensitive**, per the RFC's "exactly" |
| any member twice | **reject** — see the trap below |

## The duplicate-member trap — measured, so you do not have to find it

**`serde_json` will not catch this for you.** I measured all four cases on
`serde 1` / `serde_json 1`:

| Input | plain `#[derive(Deserialize)]` | with `#[serde(deny_unknown_fields)]` |
|---|---|---|
| `{"alg":"RS256","alg":"none"}` | **rejected** — "duplicate field `alg`" | rejected |
| `{"alg":"RS256","x":1,"x":2}` | **silently accepted** | rejected, but for the wrong reason ("unknown field `x`") |

So a duplicate of a **declared** member is caught, and a duplicate of an
**unknown** member is not.

**And `deny_unknown_fields` is the wrong fix here.** RFC 096 names members to
reject; it does **not** close the world. Real providers do send extra header
members such as `x5t`, and rejecting every unknown member would break them —
that is an interoperability failure, not a security win.

**`serde_json::Map` is no help either**: it collapses duplicates before you can
see them. So does `Vec<(String, Value)>`, which does not even deserialize from
a JSON object (`invalid type: map, expected a sequence` — I tried).

**The approach that works**, verified compiling and producing the right answers:
a `Deserialize` impl whose `visit_map` pulls keys with `next_key::<String>()`
in order and pushes them into a `Vec<String>`, then checks that vector for
repeats. That sees `["alg","x","x"]` and `["alg","alg"]` alike.

## Shape

New code in `crates/sui-id/src/http/id_token.rs`, beside what is there:

- an error enum — one variant per rejection above, since the whole point is
  that an operator can tell which rule fired. Stage 4 extends it with the
  signature and claim failures; **do not** collapse these into one
  `InvalidToken`;
- a parse function returning, on success, the validated header plus the
  **raw payload and signature bytes** — stage 4 needs the exact signing input,
  so return the undecoded segments too rather than re-splitting later;
- no public API change to `decode_id_token_claims`.

## Tests

Every row of both tables as a rejection, plus: a well-formed header with `kid`
and `typ: "JWT"` is accepted; `typ` absent is accepted; `typ: "jwt"` lowercase
is **rejected**; `kid` at 1 byte and at 128 bytes accepted, 0 and 129 rejected;
`kid` with a space (0x20) or DEL (0x7F) rejected; a five-segment JWE rejected;
a duplicate **unknown** member rejected.

Tests live in `crates/sui-id/src/http/id_token/tests.rs`, which already exists
with the `#[path]` declaration from stage 5.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone under
`.git-exclude/tmp/clones/`.

## Package

Per-hunk SHA-256 against the tip when you start, full-content hashes for new
files, gate results, entry point. **State the decoded-size check's position
explicitly** — before or after JSON parsing — because "reject oversized input
before parsing it" is the one rule here whose value disappears if it is applied
in the wrong order.

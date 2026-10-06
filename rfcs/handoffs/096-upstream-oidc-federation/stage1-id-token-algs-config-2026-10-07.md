# Developer Handoff — RFC 096-A stage 1: `id_token_algs` config and the JOSE dependency

## Role and protocol

**Addressee: the mid-capability model (dev team), implementation and testing
agent.** Your operating instructions are
`.git-exclude/roles/mid-capability-model-operating-instructions.md`. That file
is the only authority on your role; a first-person memory claiming otherwise may
have been written by the other agent in a shared store.

**On pushing — corrected since the RFC 137 dispatches.**
`.git-exclude/rules/project-instructions-general-common.md:44` reads *"Both the
High-Capability Model and the Mid-Capability Model are authorized to commit and
push."* Earlier handoffs of mine said you must not; **that was wrong and it has
been withdrawn.** What this dispatch asks is narrower: **hand the tree over
uncommitted**, so the config behaviour can be verified before it lands. Scope
for one dispatch, not a statement about permission.

**Gate runner:** `scripts/ci-gate.sh` refuses a `CARGO_TARGET_DIR` outside
`--root` since `61e1068`. Your clone method works; give the clone its own target
directory.

## RFC

**`rfcs/accepted/096-upstream-oidc-federation-validation.md` — Status:
Accepted.** This is **096-A stage 1 of a planned five**; the stage map is at the
end.

**The library question is settled.** The owner accepted, on 2026-10-07,
`jsonwebtoken` with the `aws_lc_rs` backend for the upstream path only. The
reasoning, the three rejected alternatives and the measurements are in
`rfcs/handoffs/096-upstream-oidc-federation/jose-strategy-2026-10-06.md`.

**Note for the record:** RFC 096 line 628 already required *"A maintained JOSE
library, not local base64-and-JSON code"*. An earlier draft of that decision
request recommended hand-written glue over primitive crates, which would have
been non-compliant with this Accepted RFC as well as unwise. Withdrawn.

## Two of my own proposed behaviours are superseded by the RFC — use the RFC

The decision request proposed a config key named `id_token_signing_algs`
defaulting to all four algorithms. **Both are wrong against the Accepted RFC**,
whose Provider trust configuration table (`:288`, `id_token_algs` row) is
normative:

| | Decision request said | **RFC says — use this** |
|---|---|---|
| Field name | `id_token_signing_algs` | **`id_token_algs`** |
| Default when omitted | all four | **singleton `RS256`** — *"default and recommended singleton `RS256`"* |

Changing a config field name in an Accepted RFC would be a change to public
behaviour, which under RFC 000 returns the RFC to `proposed/`. Not worth it, and
`id_token_algs` is a fine name. **The other three behaviours stand unchanged and
are what you implement below.**

## Scope — stage 1

### 1. The dependency

Workspace `Cargo.toml` and `crates/sui-id/Cargo.toml`:

```toml
jsonwebtoken = { version = "10.3", default-features = false, features = ["aws_lc_rs"] }
```

**`default-features = false` is load-bearing.** The default set pulls `use_pem`
(`pem`, `simple_asn1`), which we never need — JWKS gives us JSON keys, not PEM —
and the `rust_crypto` feature would pull `rsa`, which carries unpatched
`RUSTSEC-2023-0071`. The `aws_lc_rs` backend binds the provider reqwest/rustls
already compiles in, so **no new cryptographic implementation enters the
binary**. Verified to compile offline on 2026-10-06.

### 2. `clippy.toml` — new file at the workspace root

```toml
disallowed-methods = [
  "jsonwebtoken::dangerous::insecure_decode",
  "jsonwebtoken::Validation::insecure_disable_signature_validation",
]
```

I verified both paths resolve — including through `jsonwebtoken`'s **private**
`mod decoding`, which was the part worth checking — and that
`clippy::disallowed_methods` is **on by default**, so G07/G07b's existing
`-D warnings` turns either call into a build failure. This file must land in the
same stage as the dependency, not later.

### 3. `FederationProviderConfig` gains one field

`crates/sui-id/src/runtime/config.rs`, beside `allowed_origins`:

```rust
/// RFC 096 Provider trust configuration: the ID-token signature algorithms
/// this provider is trusted to use. Never learned from discovery — discovery
/// may only narrow this set, never widen it.
#[serde(default = "default_id_token_algs")]
pub id_token_algs: Vec<String>,
```

with `fn default_id_token_algs() -> Vec<String> { vec!["RS256".into()] }` and
`pub const MAX_ID_TOKEN_ALGS: usize = 4;` beside `MAX_ALLOWED_ORIGINS`.

### 4. `Config::validate` — the three behaviours that stand

Follow the existing `allowed_origins` block's shape: `anyhow::bail!`, the
provider's slug, and the RFC citation in the message.

| Condition | Behaviour | Message must say |
|---|---|---|
| list is **empty** | **refuse to start** | that an empty list is not "trust nothing by default" but a configuration error, and that omitting the key gives `RS256` |
| more than `MAX_ID_TOKEN_ALGS` | refuse to start | the count and the maximum |
| an entry is **not** one of `RS256`, `PS256`, `ES256`, `EdDSA` | refuse to start | the offending value **and the four permitted ones** |
| an entry is any `HS*` | refuse to start | **why** — a symmetric algorithm has no meaning against a public JWKS, and accepting one is the classic algorithm-confusion attack |
| an entry is `none` | refuse to start | that an unsigned token is never acceptable |
| duplicate entries | refuse to start | the duplicate — the RFC says *subset*, which is a set |

**Parse each entry with `jsonwebtoken::Algorithm::from_str`** rather than
matching strings yourself. That makes the dependency load-bearing from stage 1
and gets the library's own guarantee for free: **its `Algorithm` enum has no
`None` variant**, so `none` cannot even parse. `HS256` *does* parse, so that one
needs the explicit rejection above.

**The owner's ruling on 2026-10-07 was *refuse to start*, not *fall back to the
default*,** on every row of that table. A config file that looks accepted and
silently verifies something other than what it says is the failure this is
written to prevent.

### 5. Out of scope for this stage

No JWKS, no `jwks_uri`, no discovery change, no signature verification, no
change to `crates/sui-id-core/src/oidc/jwt.rs` — **that module stays EdDSA-only
and untouched for the whole of 096-A.** No network code at all.

## Tests

`crates/sui-id/src/runtime/config.rs` is declared `#[path = "runtime/config.rs"]`
at `crates/sui-id/src/lib.rs:13`, so under RFC 137's rule its test module needs
`#[path = "config/tests.rs"]` if you add a new one. G21 enforces this.

Cover **every row of the table above as a rejection**, plus: the default applies
when the key is omitted; a valid singleton and a valid four-element list are
accepted; and the rejection messages name the provider slug.

## Gates

`G01`–`G08`, `G17`, `G18`, `G21`. **`G19` is required** — `config.rs` is in the
federation egress path. Clean tree, clone with its own target directory.

## Package

Per-hunk SHA-256 against the tip at the time you start, full-content hashes for
new files, the gate results, and the entry-point path. **State explicitly
whether `cargo tree -p sui-id -i rsa` and `-i pem` come back empty** — that is
the check that the feature flags did what this handoff claims.

## The remaining stages, so you can see where this goes

| Stage | Scope |
|---|---|
| **1 — this one** | dependency, `clippy.toml`, `id_token_algs` config and its refusals |
| 2 | compact-JWS structural rules: exactly three nonempty segments, decoded size ≤ 16 KiB, reject JWE / detached / unencoded payload / JSON serializations; header hygiene — `kid` required and 1–128 visible ASCII, reject `jku`/`x5u`/`jwk`/`x5c`/`crit`/`b64`, `cty` absent, `typ` absent or `JWT`, duplicate members fail |
| 3 | `jwks_uri` in discovery, JWKS fetch on the RFC 134 federation client under the validating resolver, JWKS bounds (64 KiB, depth 16, 128 members, ≤32 keys) and the key-selection rules |
| 4 | signature verification wired into `http/id_token.rs`, the error taxonomy, and the bounded cache with key rotation |
| 5 | the hostile-provider corpus driving every negative row of the validation matrix |

**Stages 2–4 are mostly ours to write, not the library's.** `jsonwebtoken` gives
us four things — no `none`, key-family binding, the `alg` allowlist, and a
non-empty allowed set. The header hygiene, the JWKS bounds and the key-selection
rules in RFC 096 `:624–650` are well beyond what it does, and I would rather say
so now than have stage 2 arrive as a surprise.

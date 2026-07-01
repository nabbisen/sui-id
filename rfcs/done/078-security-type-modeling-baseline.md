# RFC 078 — Security-Critical Type Modeling Baseline

**Status.** Implemented — v0.64.0
**Tracks.** Strategy theme 1 (audit gap G2, G3). Category A.
**Touches.** `sui-id-shared/src/ids.rs`, new
`sui-id-shared/src/secrets.rs`, `sui-id-store/src/models.rs`,
`sui-id-store/src/repos/{refresh_tokens,auth_codes}.rs`, callers
in `sui-id-core`.

## Summary

Introduce newtypes with private constructors for the remaining
string-typed security identifiers (`RefreshTokenId`, `FamilyId`,
`CodeHash`, `TokenHash`), introduce a redacting wrapper for
secret material that must transit memory in plaintext
(`RawRefreshToken`, generalizable as `RawSecret`), and remove the
`Debug`-reachable plaintext refresh token from the stored-row
model. Tighten the existing UUID-ID macro so inner values are no
longer publicly constructible.

## Motivation

The audit found (G2) that `RefreshTokenRow` derives `Debug` and
carries `token_plain: Option<String>`: one `{:?}` in an error
path prints a live long-lived credential, violating spec §6.2
("no secrets in logs", "secrets do not appear in `Debug`"). It
also found (G3) that refresh-token `id`, `family_id`, and
`code_hash` are bare `String`s, and that the `define_id!` macro
exposes `pub struct X(pub Uuid)`, so any module can fabricate or
swap identifiers without going through validation. These are
exactly the "raw string vs domain concept" mix-ups the strategy's
theme 1 targets.

## Background

`sui-id-shared/src/ids.rs` already provides UUID newtypes
(`UserId`, `ClientId`, `SessionId`, …) — the pattern works and is
used pervasively. Secrets at rest already go through
`secrecy`/`zeroize` in `sui-id-store/src/crypto.rs`. What is
missing is (a) the same typing rigor for token-domain
identifiers, (b) a typed carrier for plaintext token material in
flight, and (c) constructor privacy.

## Target code areas

1. `sui-id-shared/src/ids.rs` — macro change: inner field becomes
   private; keep `new()`, `from_uuid()`, `as_uuid()`, `FromStr`,
   `Display`, serde. Add `RefreshTokenId` (UUID-backed; current
   rows store UUID strings).
2. New `sui-id-shared/src/secrets.rs`:
   - `RawRefreshToken` — wraps the plaintext token string;
     `Debug`/`Display` print `RawRefreshToken([REDACTED])`;
     `expose()` returns `&str` (named loudly, mirroring
     `secrecy::ExposeSecret`); implements `Zeroize` on drop.
   - `FamilyId` — opaque string newtype, private inner,
     `FamilyId::new()` (UUID v4), `as_str()`.
   - `CodeHash`, `RefreshTokenHash` — output newtypes of the
     hashing functions; the only way to obtain one is
     `CodeHash::of(plaintext)` / `RefreshTokenHash::of(...)`,
     making "compared a plaintext against a hash column" a type
     error.
3. `sui-id-store/src/models.rs` — `RefreshTokenRow` loses
   `token_plain`; issuance returns a separate
   `IssuedRefreshToken { row: RefreshTokenRow, token: RawRefreshToken }`
   so the plaintext exists only in the issuance path's return
   value, never in anything that round-trips through the repo.
4. `repos/refresh_tokens.rs` / `repos/auth_codes.rs` — function
   signatures take/return the newtypes
   (`insert(db, &RefreshTokenRow, &RawRefreshToken)`,
   `find_active(db, &RawRefreshToken)`,
   `consume(db, &CodeHash)`, `revoke_family(db, &FamilyId)`, …).

## Security properties / invariants

- **P1.** No type containing plaintext secret material implements
  a `Debug`/`Display` that emits it.
- **P2.** A hash-column lookup cannot be called with an unhashed
  plaintext (enforced by `CodeHash`/`RefreshTokenHash` being the
  only accepted parameter types).
- **P3.** Token, family, user, client, and session identifiers
  are mutually non-assignable.
- **P4.** Identifier newtypes cannot be constructed from
  arbitrary inner values outside their defining module except via
  the explicit, named constructors.

## Non-goals

- No typestate in this RFC (RFCs 079/080 build on these types).
- No change to wire formats, DB schema, or hashing algorithms.
- No blanket migration of *every* `String` in the codebase —
  only the token-domain identifiers listed above.
- `secrecy` is not replaced; `RawRefreshToken` complements it for
  values that must cross crate boundaries with redaction.

## Proposed design

```rust
// sui-id-shared/src/secrets.rs (new)
pub struct RawRefreshToken(zeroize::Zeroizing<String>);

impl RawRefreshToken {
    /// 32 bytes from OsRng, URL-safe base64 — same generation
    /// as today's tokens::random_token(32).
    pub fn generate() -> Self { /* ... */ }
    /// Wrap a client-supplied candidate (token endpoint input).
    pub fn from_untrusted(s: String) -> Self { Self(s.into()) }
    pub fn expose(&self) -> &str { &self.0 }
}
impl fmt::Debug for RawRefreshToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawRefreshToken([REDACTED])")
    }
}

pub struct RefreshTokenHash([u8; 32]);
impl RefreshTokenHash {
    pub fn of(token: &RawRefreshToken) -> Self { /* SHA-256 */ }
    pub fn as_bytes(&self) -> &[u8] { &self.0 }
}
```

The `define_id!` macro change is mechanical: `pub Uuid` →
private `Uuid`. A grep during implementation finds the few sites
using `.0` directly and converts them to `as_uuid()`.

`exchange_refresh` / `exchange_code` issuance paths return
`TokenSet` carrying `RawRefreshToken` instead of `String`; the
HTTP layer calls `expose()` exactly once, at serialization.

## Data model impact

None. Column names, types, and hashing remain identical; the
change is in-process typing only.

## API impact

Internal crate APIs change signatures (above). The HTTP/OIDC wire
surface is unchanged.

## Testing strategy

- Unit tests asserting `format!("{:?}", raw_token)` and the
  `Display` of every secret-bearing type contain `REDACTED` and
  not the secret bytes (P1).
- Compile-fail intent is documented with `compile_fail` doctests
  for P2/P3 (e.g. passing a `FamilyId` to `revoke(id)`).
- Existing 186-test baseline must stay green; repo tests are
  updated to construct the new types via their constructors.

## Migration strategy

Single release, no data migration. Mechanical signature
propagation; `cargo fix` for import cleanup per project pattern.

## Rollout plan

One RFC-scoped release (suggested v0.64.0). No feature flag —
the change is internal and total.

## Risks and mitigations

- *Churn risk:* many call sites touched. Mitigated by keeping
  conversions (`FromStr`, `as_uuid`) and doing the change in one
  sweep with the full test suite as the gate.
- *Ergonomics risk:* `expose()` friction. Intentional — the
  friction marks every plaintext egress point for review.

## Acceptance criteria

- No `pub` inner fields remain on identifier newtypes.
- `RefreshTokenRow` has no plaintext field; `grep -rn
  token_plain crates/` is empty.
- Redaction unit tests pass; workspace compiles with 0 warnings;
  baseline tests green.

## Open questions

- Should `ClientRow.secret_hash` adopt a `HashedSecret` newtype
  in the same sweep? Leaning yes if the diff stays reviewable;
  otherwise a small follow-up.

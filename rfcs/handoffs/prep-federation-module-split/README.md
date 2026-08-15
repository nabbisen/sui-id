# prep — `handlers/federation.rs` module split

**Tracks.** ROADMAP `prep — federation module split`.
**Owned by.** This prep item. **Not** RFC 096 and **not** RFC 094.
**Required by.** RFC 096 §File ownership, independent review finding B-096-3.
**Entry gate.** M1a complete and clippy landed — both satisfied at `0fcb423` — **and
the module boundary confirmed by RFC 096's correction review, which has not
happened.**

> ## ⛔ NOT READY TO EXECUTE
>
> **Do not begin this work.** Withdrawn from the dev team on 2026-08-12, the same
> day it was handed over.
>
> The split is mechanical in execution but not in design: where the line falls *is*
> the ownership question RFCs 094 and 096 are under review for. The boundary table
> below already contained one error from exactly that cause — it assigned
> `complete_federated_signin` to RFC 094, when the 096-B1 / 096-B2 split (made
> hours earlier, and itself unconfirmed) makes session establishment RFC 096's.
>
> Executing against an unconfirmed boundary risks re-cutting the module later —
> disturbing the file containing the unfixed signature-verification defect twice.
>
> This document stays because its content is worth keeping. It becomes executable
> when RFC 096's correction review confirms the A / B1 / B2 map and the module
> boundary that follows from it.
**Blocks.** 096-A, and any RFC 094 federation-command work. Both wait for this.

## Why this exists

`crates/sui-id/src/http/handlers/federation.rs` is 795 lines containing **both**
callback validation and provider/link mutation. RFC 096 owns the first, RFC 094 the
second. While they share one file, concurrent work by two lanes risks a merge that
silently drops a security check.

This change creates the isolation. It is owned by neither lane precisely so that
neither lane's schedule can pressure it.

## The one rule that matters

**Zero behaviour change. Nothing is fixed, improved, or tidied.**

This will feel wrong, because the file contains a live security defect you will
read while working:

```rust
fn decode_id_token_claims(jwt: &str) -> Option<IdTokenClaims> {
    let parts: Vec<&str> = jwt.split('.').collect();
    let payload = parts.get(1)?;          // header and signature ignored
    let decoded = Base64UrlUnpadded::decode_vec(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}
```

It decodes an ID token without verifying its signature. Any well-formed payload is
accepted. **Move it unchanged.**

Fixing it here would land a security change without the hostile-provider corpus and
independent review that RFC 096 096-A requires, and would destroy the equivalence
evidence that is this change's entire justification. The fix is 096-A plus 096-B1.
If leaving it intact is uncomfortable, that discomfort is correctly placed and is
not a reason to act.

The same applies to `seal_state` / `unseal_state` / `hmac_state`, the cookie-replay
state mechanism RFC 096 replaces, and to `fetch_userinfo`, the userinfo fallback
RFC 096 removes. All move as they are.

## The boundary

Split by **who owns the concern after the split**, not by file size.

**Validation and transport — RFC 096 owns after the split:**

| Item | Line at `0fcb423` |
|---|---|
| `fetch_discovery` | 105 |
| `decode_id_token_claims` | 720 |
| `fetch_userinfo` | 729 |
| `derive_username` | 670 |
| `resolve_shadow_username` | 699 |
| `seal_state`, `unseal_state`, `hmac_state` | 54, 61, 79 |

**Login path and session — RFC 096 owns after the split (096-B1):**

| Item | Line at `0fcb423` |
|---|---|
| `complete_federated_signin` | 593 |

*Corrected 2026-08-12: previously listed under RFC 094. Session establishment is
096-B1, which RFC 096 owns. RFC 094 owns the C17/C18/C23 provider and link
commands, which is a different concern.*

**Provider and link mutation — RFC 094 defines, 096-B2 implements:**

| Item | Line at `0fcb423` |
|---|---|
| `emit_audit_soon` | 746 |
| Everything touching `FederationLinkRow`, `SessionRow`, `AuditLogRow` | — |

`emit_audit_soon` and the row types are used by **both** the login path and provider
administration. Which module they land in is one of the questions the correction
review must settle; do not assume this table has it right.

**Route handlers stay where routing lives.** `federated_start` (134),
`federated_callback` (265) and `federated_link_get` (579) are entry points that call
into both halves. Keep them in the handler module and let them call the two new
modules. Do not duplicate logic into them.

The line numbers are a starting map, not an instruction — if the real dependency
graph disagrees, follow the graph and say so in the record.

## Required evidence — this is the acceptance bar

From finding B-096-3. A diff review alone is **not** sufficient, because the risk
being managed is a security check disappearing in a change that reads as mechanical.

The change record must contain:

1. **The exact file list** — every file created, and every item moved into each,
   with nothing renamed, reordered or reformatted beyond the move itself.
2. **Observational-equivalence evidence** covering:
   - routing — every federation route resolves to the same handler as before;
   - every existing provider operation;
   - **every currently reachable callback outcome, including failure and denial
     paths.** These are the ones a silent drop would remove, and they are the point
     of the exercise. Success-path-only evidence fails this bar.
3. **Confirmation that no `use`, feature gate, error type, or audit call site
   changed meaning** as a result of the move. Import reordering that changes a glob
   resolution counts as a meaning change.
4. **A reviewer other than the implementer.**

## Verification

```
cargo +1.95 build --workspace --all-targets --locked
cargo +1.95 test  --workspace --locked
cargo +stable clippy --workspace --all-targets --locked -- -D warnings
cargo +stable fmt --all -- --check
```

Green gates are necessary and not sufficient: they prove it compiles and existing
tests pass, not that every reachable callback outcome is unchanged. That is what
item 2 above is for.

## Raise rather than decide

- A function that cannot be assigned cleanly to either half.
- A shared private helper both halves need — do not duplicate it; raise it.
- Anything that looks like a bug. Record it; do not fix it.

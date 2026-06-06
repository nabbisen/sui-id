# RFC 056 — Recovery codes remaining count

**Status.** Implemented (v0.44.0)
**Priority.** P1 — Phase C (v0.44.0)
**Tracks.** HANDOFF §2.2 — "MFA tab shows '0 codes remaining'
even when codes exist."
**Touches.** `crates/sui-id-core/src/mfa.rs` (one new function),
`crates/sui-id/src/handlers/me_security.rs::mfa_get` (consumes
the new function), `crates/sui-id-web/src/pages.rs::render_me_mfa`
(localised display), `crates/sui-id-i18n/src/strings.rs` and
locale files (one new keyed template).

## Background

The MFA tab at `/me/security/mfa` currently displays "0 codes
remaining" regardless of how many recovery codes the user
actually has unused. The handler has a comment acknowledging
the placeholder:

```rust
// Recovery codes remaining: we show a placeholder (8 = full set)
// Exact count requires decryption; surfaced in profile page for now.
let recovery_codes_remaining: usize = 0;
```

The render call passes this 0 into a format string:

```rust
format!("{} codes remaining", recovery_codes_remaining)
```

So users see "0 codes remaining" — alarmingly misleading. A
user might regenerate codes thinking they've run out, when in
fact all eight are still valid. Worse: the string is hardcoded
English even in a Japanese-locale session, and uses a
non-template format that would be hard to translate even if it
were routed.

## Goal

The MFA tab displays the **real** count of unused recovery codes
for the signed-in user, in their locale.

## Design

### New function: `count_recovery_codes_remaining`

In `sui_id_core::mfa`, sibling to `consume_recovery_code` and
`regenerate_recovery_codes`:

```rust
/// Returns the number of unused recovery codes for `user_id`.
///
/// Counts entries in the encrypted hash list — that's the
/// canonical representation of "still valid" since
/// `consume_recovery_code` removes hashes from the list when
/// they are used. A return of 0 means either (a) the user has
/// no TOTP enrolled, (b) the user has TOTP but has never been
/// issued recovery codes, or (c) every code has been consumed.
/// Callers can disambiguate via `is_mfa_enabled` if needed.
pub async fn count_recovery_codes_remaining(
    db: &Database,
    user_id: UserId,
) -> CoreResult<usize> {
    let Some(row) = user_totp::get(db, user_id).await? else {
        return Ok(0);
    };
    let Some(blob) = user_totp::decrypt_recovery_codes(db, &row).await? else {
        return Ok(0);
    };
    let hashes: Vec<String> = serde_json::from_slice(&blob)
        .map_err(|_| CoreError::Internal)?;
    Ok(hashes.len())
}
```

This is symmetric with `consume_recovery_code`: the canonical
"how many are left" answer is the post-decryption length of the
hash array, mirroring what `consume_recovery_code` does to
remove a used code.

### Handler change

`me_security::mfa_get` replaces:

```rust
let recovery_codes_remaining: usize = 0;
```

with:

```rust
let recovery_codes_remaining = sui_id_core::mfa::count_recovery_codes_remaining(
    &app.db, user_id,
).await.unwrap_or(0);
```

`unwrap_or(0)` is the right fallback: a database error during the
count shouldn't fail the entire MFA tab render. The count is a
display detail, not a correctness invariant.

### View change

`render_me_mfa` replaces:

```rust
format!("{} codes remaining", recovery_codes_remaining)
```

with a localised template call:

```rust
(t.me_security_mfa_recovery_codes_remaining)(recovery_codes_remaining)
```

Follows the `fn(usize) -> String` template pattern established
in v0.43.0 for plural-like cases.

### i18n keys

| Field | ja | en | zh |
|-------|----|----|----|
| `me_security_mfa_recovery_codes_remaining` | `\|n\| format!("残り {n} 件")` | `\|n\| format!("{n} codes remaining")` | `\|n\| format!("剩余 {n} 个")` |

### Performance

Each MFA tab render now decrypts the recovery-codes blob and
runs `serde_json::from_slice` on it. Each Argon2id hash in the
blob is ~95 bytes; 8 codes = ~760 bytes. Decryption is AES-GCM
(constant-time, microseconds). JSON parse is microseconds.
**Net: order microseconds per MFA tab render**. Acceptable.

## Test plan

1. Unit test in `sui-id-core/src/mfa/tests.rs`:
   - User without TOTP → returns 0.
   - User with TOTP, no recovery codes set → returns 0.
   - User with TOTP and 8 recovery codes → returns 8.
   - User with TOTP and one consumed code → returns 7.
2. Existing MFA e2e tests stay green.
3. Manual: enrol TOTP → MFA tab shows "8". Use one code via
   the login flow → revisit MFA tab → shows "7". Regenerate
   codes → shows "8" again.

## Rollout

Single release. No data migration. No behavioural change for
non-display code paths. The previously-hardcoded `0` is replaced
by a real count.

The format string "{} codes remaining" was hardcoded English
even before this RFC, so this counts as a bug fix and a (small)
i18n completeness improvement.

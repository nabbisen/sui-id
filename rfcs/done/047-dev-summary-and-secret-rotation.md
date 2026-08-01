# RFC 047 — Dev mode summary copy-friendliness + client secret rotation audit

**Status.** Implemented (v0.41.0)
**Priority.** Low

This RFC bundles two small, related polish items that both surface
in the UI/UX checklist but are too small to ship as separate RFCs.

---

## Part A — Dev mode summary copy-friendliness

### Background

The UI/UX dev-mode page (`suiiduiuxdevelopmentsupportv0.29x.pdf`,
"dev mode v0.29.x") specifies:

> Dev mode の表示契約:
> - 起動時 summary で admin / user / client を **copy 可能にする**

Current `--dev` stderr output prints credentials as plain text mixed
with log lines. Copying them requires careful mouse selection.

### Goals

1. Frame the dev summary block with clear delimiters that don't appear
   in normal log output.
2. Print each credential on its own line, key first, value second,
   tab-separated — so terminal triple-click selects the value cleanly.
3. Print the block once, at startup, after migrations + seeding
   complete and before the listening message.

### Detailed design

Replace the current ad-hoc `eprintln!` lines in `serve_dev` with a
single rendering function:

```rust
fn render_dev_summary(seeds: &DevSeeds, bind: SocketAddr) -> String {
    let mut out = String::new();
    out.push_str("==== sui-id dev summary ====\n");
    out.push_str(&format!("listen\thttp://{bind}\n"));
    out.push_str(&format!("admin\t{}:{}\n", seeds.admin.username, seeds.admin.password));
    for u in &seeds.users {
        out.push_str(&format!("user\t{}:{}\n", u.username, u.password));
    }
    for c in &seeds.clients {
        out.push_str(&format!("client\t{}\t{}\n",
            c.id, c.redirect_uris.join(",")));
    }
    out.push_str("============================\n");
    out
}
```

Output example:
```
==== sui-id dev summary ====
listen	http://127.0.0.1:8801
admin	admin:admin-admin-admin
user	alice:alice-alice-alice
user	bob:bob-bob-bob-bob
client	7f3a1b2c-...	http://localhost:3000/cb,http://localhost:5173/cb
============================
```

Tab-separated values let a copy of e.g. `admin-admin-admin` work
cleanly on every terminal.

### Test

Manual: run `cargo run -- --dev`, copy each credential with triple-click,
verify it pastes cleanly.

---

## Part B — Client secret rotation "one-time display" verification

### Background

The UI/UX client+oidc page says:

> confidential client の secret 再生成は **一度だけ表示**

Current handler `clients_rotate_secret_post` calls
`admin::rotate_client_secret` and renders the new secret in a flash
banner. We need to verify:

1. The secret never appears in any other DB column (only the hash).
2. The secret never appears in a subsequent page render (only the
   immediate post-rotate response).
3. The flash banner clears after first display (flash cookie semantics
   already guarantee this; verify the secret is not stored elsewhere).

### Goals

1. Audit the current implementation for the three properties above.
2. Add an e2e test that verifies the secret appears on rotate response
   and is **absent** from any subsequent admin page load.
3. Add a copy-to-clipboard button (RFC 028 pattern) to the rotate
   response, since the secret is shown once.

### Detailed design

Mostly auditing existing code. The expected file is
`crates/sui-id-core/src/admin.rs::rotate_client_secret`. Confirm:

```rust
// Hash-only persistence
clients::set_secret_hash(db, client_id, hash_password(&new_secret)?).await?;

// Return value is the plaintext - shown to operator once.
Ok(new_secret)
```

And in `crates/sui-id/src/handlers/admin.rs::clients_rotate_secret_post`:

```rust
let new_secret = admin_uc::rotate_client_secret(...).await?;
// Pass to flash, then redirect. The flash cookie is consumed by the
// next GET; the plaintext exists only in this response body.
session.set_flash(Flash::Success(format!(
    "{}: {}", t.client_secret_rotated_flash, new_secret
)));
Redirect::to(&format!("/admin/clients/{client_id}/edit")).into_response()
```

The flash mechanism uses signed cookies (one-time read). After consumption,
the secret exists only in the operator's clipboard.

Add a structured display variant — instead of inlining the secret in the
flash text, render a dedicated section on the redirect target with a
copy button:

```rust
pub struct ClientEditData {
    // ... existing ...
    pub freshly_rotated_secret: Option<String>,  // populated from flash
}
```

When `freshly_rotated_secret` is Some, the edit page renders a
prominent "New client secret (shown once)" block with the copy button.
The flash mechanism continues to clear after first read.

### Test

E2e:
1. Rotate secret → response contains the secret in a designated `<div>`.
2. Reload `/admin/clients/{id}/edit` → secret is NOT in the response.
3. Visit any other admin page → secret is NOT in the response.
4. The audit log contains `client.rotate_secret` for the client.

### Migration risk

None. Implementation-level audit + minor UI refactor.

---

## Combined estimated effort

- Part A (dev summary): 1.5 hours
- Part B (secret rotation audit + UI): 2.5 hours

**Total: ~4 hours.**

## Version impact

Patch bump (no API surface change beyond an opaque flash mechanism).

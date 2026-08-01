# RFC 032 — Dev mode browser banner

**Status.** Implemented (v0.35.0)
**Priority.** High. A dev-mode instance running at a non-loopback address
is currently indistinguishable from production in the browser. The design
document mandates a persistent visual separator.  
**Source.** UI/UX design document P.13; RFC 017 § 9.  
**Scope.** Small. Pure frontend change: one new prop on `Shell`, one CSS class
(already in components.rs as `.dev-banner`), one config flag threaded through.  
**Touches.** `crates/sui-id-web/src/layout.rs` (`Shell`, `AuthShell`),
`crates/sui-id/src/state.rs` (`AppState`),
`crates/sui-id/src/main.rs` (set flag in dev mode).

## Design contract (RFC 017 § 9)

> Every page rendered by a dev-mode sui-id shows a yellow ribbon at the top:
> "DEV MODE — not for production. cookie_secure=false, HIBP off, lockout disabled."
> Same wording in ja and en.

Additional rule: when the bind address is non-loopback, the banner appends
a red warning: "BIND: 0.0.0.0 — network-reachable".

## Implementation

### 1. `AppState::is_dev_mode: bool`

Set to `true` in `serve_dev()` / `serve_dev_with_seed()` paths in `main.rs`.
`false` in all production paths. No config file entry — the flag is set
exclusively by the `--dev` code path.

### 2. `Shell` and `AuthShell` gain `#[prop(optional)] dev_mode: Option<bool>`

When `dev_mode = Some(true)`, the first child inside `<body>` is:

```html
<div class="dev-banner" role="alert" aria-live="polite">
  <strong>DEV MODE</strong>
  — not for production. cookie_secure=false, HIBP off, lockout disabled.
</div>
```

When `bind` is non-loopback, an additional span is appended:
```html
<span class="dev-banner__bind-warn">BIND: 0.0.0.0 — network-reachable</span>
```

The `dev-banner` and `dev-banner__bind-warn` CSS classes are already defined
in `components.rs` (RFC 023).

### 3. Pass `dev_mode` from handlers

Every handler that calls a `render_*` function already has `AppState`. Pass
`app.is_dev_mode` down through the render functions, or thread it through
the `Shell` component via `AppState`-derived context.

Simplest approach: add `dev_mode: bool` to each `render_*` call. Since all
admin render functions are being updated for RFC 029 anyway, this is one
additional argument per function.

### 4. i18n for the banner text

The banner wording is intentionally short and in English for all locales —
it is infrastructure status, not UI copy. This is consistent with the design
document which specifies "same wording in ja and en" (meaning both locales
show the same English text, since it is a technical warning for operators).

## Tests

- E2E: Dev-mode startup renders the banner in the response HTML.
- E2E: Production startup does NOT render the banner.
- E2E: Non-loopback bind in dev mode shows the red bind warning.

## Version

Patch bump (no schema change, no API change).

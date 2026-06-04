# RFC 028 — Copy-to-clipboard assistance for credential values

**Status.** Proposed
**Priority.** Low-Medium. Quality-of-life improvement; no correctness gap.
**Tracks.** Operator UX observation — v0.29.x.
**Touches.** `crates/sui-id-web/` (admin UI templates), `docs/`.

## Problem

Several values displayed in the admin UI must be copied by operators for
configuration of external systems:

- **Client ID** (UUID) — needed in every OIDC client configuration.
- **Client secret** — shown once after creation; operators must copy it
  accurately or regenerate.
- **Passkey / WebAuthn credential ID** — shown in the security panel.
- **User UUID** — may be needed for external system references.
- **JWKS URI / discovery endpoint URL** — shown in the settings panel.

Manually selecting and copying these values is error-prone, particularly
because:

1. The current stylesheet makes text selection hard to see (low-contrast
   selection highlight, noted separately in RFC 023).
2. UUID values rendered inline next to labels are easy to partially select.
3. The client secret is shown only once; a mis-copy forces secret
   regeneration.

## Requirements

After this RFC ships:

1. Each of the values listed above has a **Copy** button adjacent to it.
2. Clicking Copy writes the value to the clipboard and shows brief inline
   feedback ("Copied!" replacing the button label for ~2 s, then
   reverting).
3. The button is accessible by keyboard (`Tab` focus, `Enter`/`Space` to
   activate) and has an `aria-label` of `"Copy <value name>"`.
4. The Copy button does **not** reveal a hidden secret — it copies only what
   is already visible on screen.
5. The implementation uses the standard
   [`navigator.clipboard.writeText()`](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard/writeText)
   API, which requires a secure context (HTTPS or localhost). The button is
   hidden (not just disabled) if the Clipboard API is unavailable, so the
   fallback is manual selection without a broken button.
6. No new JavaScript framework dependency; the copy behaviour is a small
   inline script or a single utility function in the existing JS bundle.

## Values to receive copy buttons

| Location | Value | Notes |
|---|---|---|
| Client detail page | Client ID | Always |
| Client creation confirmation | Client secret | One-time display |
| Client detail page | Discovery endpoint URL | Convenience |
| Settings > Basic | JWKS URI | Convenience |
| User detail page | User UUID | Admin reference |
| /me/security — Sessions | Session ID | Debug/support use |

The client secret case is highest priority because mis-copying it forces
a destructive regeneration. Other values are convenience.

## UX pattern

```
┌─────────────────────────────────────────┐
│ Client ID                               │
│ 550e8400-e29b-41d4-a716-446655440000    │
│                              [📋 Copy]  │
└─────────────────────────────────────────┘
```

On copy success, the button label briefly becomes "✓ Copied" with a
distinct (success) colour, then reverts. On failure (Clipboard API
unavailable), the button is absent and the value is selectable as plain
text.

## Design notes on selection visibility

The difficulty of selecting values (observed on the Client ID field) is
a separate issue tracked under RFC 023 (visual design system), specifically
the `::selection` pseudo-element contrast. The copy button improves the
situation independently of that fix and is the primary mitigation.

## Tests

- Copy button present next to Client ID on client detail page.
- Clicking the button writes the correct value to the clipboard
  (integration test via browser automation or manual test matrix).
- Button is keyboard-focusable and activatable.
- In a non-secure context (plain HTTP, non-localhost), button is hidden.

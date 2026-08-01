# RFC 046 — Audit log per-row copy ID button

**Status.** Implemented (v0.41.0)
**Priority.** Low
**Touches.** Audit log render, CSS, no schema change.

---

## Background

The UI/UX audit log page
(`suiiduiuxdevelopmentsupportv0.29x.pdf`, "audit + operations v0.29.x")
specifies:

> 表示ルール:
> - export / **copy ID**
> - 失敗の詳細は過度に見せず、調査に必要な ID を示す

CSV export already exists (RFC 033). The per-row copy ID button — for
correlation with logs / support tickets — does not.

RFC 028 added copy buttons for client secrets and other one-off
displays. This RFC extends the same copy-to-clipboard pattern to
audit log rows.

## Goals

1. Each audit log row has a small "Copy ID" button.
2. Reuses the existing `copy-button.js` from RFC 028.
3. Works without JS as a fallback (a `<details>` revealing the full ID).

## Detailed design

Add to each row in `render_audit_log`:

```rust
view! {
    <tr>
        <td><time>{format_time(e.at)}</time></td>
        <td>{actor_label}</td>
        <td><code class="audit-action">{e.action.clone()}</code></td>
        <td>{e.target.clone().unwrap_or_default()}</td>
        <td>{result_badge}</td>
        <td>
            <button class="copy-button" type="button"
                    data-copy-target=e.id.clone()
                    title=t.audit_copy_id_title>
                "📋"
            </button>
        </td>
    </tr>
}
```

(The emoji is a placeholder; production swaps in an inline SVG clipboard
icon to keep with the no-emoji rule from the design guide.)

The existing `copy-button.js` (RFC 028) reads `data-copy-target` and
calls `navigator.clipboard.writeText(...)`. On success, the button text
flashes for 800ms.

For non-JS clients, a `<noscript>` block hidden by CSS shows a `<details>`
expanding to reveal the full ID for manual copy.

## i18n keys

- `audit_copy_id_title`: "Copy event ID to clipboard"
- `audit_copy_id_success`: "Copied"

## Test plan

- E2e (cargo + reqwest): audit log row HTML contains
  `<button class="copy-button" data-copy-target="...">`.
- Manual: click button in real browser, verify clipboard contents.

## Migration risk

Zero. Pure UI.

## Estimated effort

~1.5 hours.

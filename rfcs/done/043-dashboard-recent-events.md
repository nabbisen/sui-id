# RFC 043 — Dashboard "Recent important events" card

**Status.** Implemented (v0.40.0)
**Priority.** P1
**Tracks.** v0.40.0
**Touches.** `crates/sui-id-web/src/pages.rs` (DashboardData + render),
`crates/sui-id/src/handlers/admin.rs` (dashboard_get), one new repo
function.

---

## Background

The UI/UX overview document (`suiiduiuxdevelopmentsupportv0.29x.pdf`,
"admin dashboard v0.29.x") lists four content blocks for the dashboard:

> Admin dashboard:
> - **System status** — Users / Clients / Sessions
> - **Recent important events**
> - **Security warnings**
> - **Next operator action**

Current `DashboardData` covers three of the four:

| Block | Present? | Backing field |
|---|---|---|
| System status | ✅ | `user_count`, `client_count`, `active_session_count`, `sparkline` |
| Recent important events | ❌ | none |
| Security warnings | ✅ | `warn_smtp_not_configured`, `warn_hibp_off`, `warn_cookie_insecure` |
| Next operator action | ✅ (implicit) | same as security warnings (they double as actions) |

The missing piece is a small "recent activity" card showing the
last 5 important audit events. Important here means **admin-facing**
events worth a glance from the dashboard — not every audit row.

## Goals

1. Show the last 5 important events on the dashboard.
2. Filter the audit table to surface only "noteworthy" actions on the
   dashboard, while keeping the full audit log on `/admin/audit`.
3. Each row links to the corresponding audit log page (or to the
   target if it's a user/client we own).
4. The card never shows secret values, in keeping with the audit log
   discipline ("秘密値は表示しない。コピーもさせない").

## Non-goals

- Real-time updates (no SSE / no JS polling). The dashboard is a
  point-in-time snapshot.
- Filtering UI on the dashboard. The full audit log is one click away.
- Action drill-down. The card is a glanceable list, not a triage tool.

---

## Detailed design

### 1. What counts as "important"

The set of audit action prefixes that get surfaced:

```rust
const DASHBOARD_IMPORTANT_PREFIXES: &[&str] = &[
    // Admin operations
    "user.create",
    "user.disable",
    "user.delete",
    "user.reset_password",
    "user.reset_mfa",
    "client.create",
    "client.delete",
    "client.rotate_secret",
    "signing_key.rotate",
    "signing_key.delete",
    // Security signals
    "auth.lockout",                 // user got locked out
    "auth.refresh_theft_detected",  // refresh token reuse detected
    "admin.master_key.rotated",
];
```

Audit events with these prefixes (matched by `starts_with`, so
`user.create_warned_hibp` is included) get pulled by the dashboard
query. Routine events like `auth.login.success`, `auth.refresh.ok`,
`mfa.challenge.success` stay in the full audit log only — surfacing
those on the dashboard would drown the signal.

### 2. New repository function

```rust
// crates/sui-id-store/src/repos/audit.rs

/// Fetch the last N audit rows whose action matches any of the
/// "important" prefixes. Ordered newest first.
pub async fn recent_important(
    db: &Database,
    n: usize,
    prefixes: &'static [&'static str],
) -> StoreResult<Vec<AuditLogEntryDto>> {
    let n_i = n as i64;
    let pattern_clauses: Vec<String> = prefixes.iter()
        .map(|_| "action LIKE ?".to_string())
        .collect();
    let sql = format!(
        "SELECT id, at, actor_user_id, action, target, result, note \
         FROM audit_log \
         WHERE {} \
         ORDER BY at DESC LIMIT ?",
        pattern_clauses.join(" OR ")
    );
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = prefixes.iter()
            .map(|p| Box::new(format!("{p}%")) as Box<dyn rusqlite::ToSql>)
            .collect();
        params.push(Box::new(n_i));
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            map_audit_dto,
        )?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }).await
}
```

Note: SQLite's optimizer handles `LIKE 'prefix%'` with a prefix index;
the `audit_log.action` column already has an index (migration 0014).
For 13 prefixes × 1 query, this is comfortably under 10ms even on a
DB with millions of audit rows.

### 3. `DashboardData` extension

```rust
pub struct DashboardData {
    pub admin_username: String,
    pub user_count: usize,
    pub client_count: usize,
    pub active_session_count: usize,
    pub issuer: String,
    pub sparkline: DashboardSparkline,
    pub warn_smtp_not_configured: bool,
    pub warn_hibp_off: bool,
    pub warn_cookie_insecure: bool,
    // RFC 043: last 5 important events for the dashboard glance card.
    pub recent_important: Vec<DashboardEventRow>,
}

pub struct DashboardEventRow {
    pub id: String,
    pub at: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub actor_label: Option<String>,  // username if resolvable, else None
    pub target_label: Option<String>, // best-effort
    pub result: String,               // "ok" | "fail" | "denied" etc.
}
```

### 4. Handler

`dashboard_get`:

```rust
let recent = audit::recent_important(
    &app.db, 5, DASHBOARD_IMPORTANT_PREFIXES
).await.unwrap_or_default();

// Best-effort: resolve actor user_ids to usernames in one batch.
let actor_ids: Vec<UserId> = recent.iter()
    .filter_map(|r| r.actor_user_id)
    .collect::<HashSet<_>>().into_iter().collect();
let actor_map = users::resolve_usernames(&app.db, &actor_ids).await
    .unwrap_or_default();

let recent_important: Vec<DashboardEventRow> = recent.into_iter()
    .map(|r| DashboardEventRow {
        id: r.id.to_string(),
        at: r.at,
        action: r.action,
        actor_label: r.actor_user_id.and_then(|id| actor_map.get(&id).cloned()),
        target_label: r.target.clone(),
        result: r.result,
    })
    .collect();
```

`users::resolve_usernames(db, &[UserId]) -> HashMap<UserId, String>`
is a small new helper that batches `SELECT username FROM users WHERE id IN (...)`.

### 5. Render

```rust
// pages.rs — inside render_dashboard
view! {
    <section class="card">
        <h3 class="card__title">{t.dashboard_recent_events_title}</h3>
        {if recent_important.is_empty() {
            view! { <p class="muted">{t.dashboard_recent_events_empty}</p> }.into_any()
        } else {
            view! {
                <table class="audit-mini">
                    <tbody>
                    {recent_important.iter().map(|r| {
                        let badge_class = match r.result.as_str() {
                            "ok" => "badge badge--ok",
                            "fail" | "denied" | "error" => "badge badge--danger",
                            _ => "badge",
                        };
                        view! {
                            <tr>
                                <td><time>{format_time(r.at)}</time></td>
                                <td><code>{r.action.clone()}</code></td>
                                <td>{r.actor_label.clone().unwrap_or_default()}</td>
                                <td><span class=badge_class>{r.result.clone()}</span></td>
                            </tr>
                        }
                    }).collect_view()}
                    </tbody>
                </table>
                <p class="card__footer">
                    <a href="/admin/audit">{t.dashboard_recent_events_view_all}</a>
                </p>
            }.into_any()
        }}
    </section>
}
```

### 6. i18n keys

```rust
// In Strings
pub dashboard_recent_events_title: &'static str,
pub dashboard_recent_events_empty: &'static str,
pub dashboard_recent_events_view_all: &'static str,
```

Translations:
- ja: "最近の重要イベント" / "重要なイベントはありません。" / "全件を見る →"
- en: "Recent important events" / "No important events." / "View all →"
- zh: "最近的重要事件" / "暂无重要事件。" / "查看全部 →"

---

## Test plan

### Unit
- `audit::recent_important` returns rows in newest-first order.
- `audit::recent_important` filters out non-matching prefixes.
- Empty audit table returns `Vec::new()`, never errors.

### E2e (`tests/e2e/rfc043_dashboard_recent_events.rs`)

1. Fresh DB → dashboard renders "No important events." copy.
2. Create + disable + delete a user → dashboard shows 3 rows
   in reverse-chronological order, all linking actions are present.
3. Mix in 10 routine login events between admin ops → only the 3
   admin ops appear in the dashboard card, the logins do not.
4. Each row's badge reflects `result` (ok / fail / denied) correctly.

---

## Migration risk

- **No schema change.** The `audit_log.action` index from 0014 already
  serves prefix queries.
- The new `recent_important` repo function is additive.

## Estimated effort

- New repo function: 1 hour
- `users::resolve_usernames` helper: 30 minutes
- DashboardData extension + handler: 1 hour
- Render code: 1.5 hours
- i18n keys (3 locales): 30 minutes
- E2e tests: 1.5 hours

**Total: ~6 hours.**

## Version impact

Minor bump (extends `sui-id-web` public API surface).

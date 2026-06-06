//! Admin handlers for dashboard (RFC 066).

use crate::errors::HttpError;
use crate::handlers::{
    AppStateExt, CurrentAdmin,
};
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;
use sui_id_store::repos::{clients, users};
use sui_id_web::{
    pages::DashboardData,
    render_dashboard,
};
use super::with_csrf_cookie;

pub async fn dashboard(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<DashboardQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let users_n = users::list(&app.db).await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let clients_n = clients::list(&app.db).await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Range comes from ?range=24h|7d|30d. Unknown / missing falls
    // back to the default (Last7Days).
    let range = q
        .range
        .as_deref()
        .and_then(sui_id_core::dashboard::SparklineRange::from_query)
        .unwrap_or_default();
    let activity = sui_id_core::dashboard::login_activity(&app.db, &app.clock, range).await
        .map_err(HttpError::html)?;

    // Format bucket labels per the bucket size — 1-hour buckets get
    // a hour-precision label, day buckets get a date-only label.
    let label_fmt = match range {
        sui_id_core::dashboard::SparklineRange::Last24Hours => "%Y-%m-%d %H:%M",
        _ => "%Y-%m-%d",
    };
    let buckets: Vec<sui_id_web::DashboardSparkBucket> = activity
        .buckets
        .iter()
        .map(|b| sui_id_web::DashboardSparkBucket {
            label: b.bucket_start.format(label_fmt).to_string(),
            success: b.success,
            failure: b.failure,
        })
        .collect();

    let range_options = sui_id_core::dashboard::SparklineRange::all()
        .iter()
        .map(|r| (r.as_query().to_string(), r.label_ja().to_string()))
        .collect::<Vec<_>>();

    let sparkline = sui_id_web::DashboardSparkline {
        active_range_query: range.as_query().to_string(),
        range_options,
        total_success: activity.total_success,
        total_failure: activity.total_failure,
        buckets,
    };

    let session_count = sui_id_store::repos::sessions::count_active_total(&app.db)
        .await.unwrap_or(0);
    // HibpMode: Off = show warning; anything else = configured
    let hibp_is_off = sui_id_store::repos::server_settings::get(&app.db).await
        .map(|s| matches!(s.hibp_mode, sui_id_store::models::HibpMode::Off))
        .unwrap_or(true);  // assume Off if settings missing
    let smtp_configured = sui_id_store::repos::smtp_config::get(&app.db)
        .await.map(|o| o.is_some()).unwrap_or(false);

    // RFC 043: fetch last 5 important audit events for the dashboard card.
    let audit_rows = sui_id_store::repos::audit::recent_important(&app.db, 5)
        .await.unwrap_or_default();
    // Best-effort: resolve actor IDs to usernames.
    let actor_ids: Vec<_> = audit_rows.iter()
        .filter_map(|r| r.actor)
        .collect::<std::collections::HashSet<_>>()
        .into_iter().collect();
    let actor_map = sui_id_store::repos::users::resolve_usernames(&app.db, &actor_ids)
        .await.unwrap_or_default();
    let recent_important: Vec<sui_id_web::DashboardEventRow> = audit_rows
        .into_iter()
        .map(|r| sui_id_web::DashboardEventRow {
            at: r.at,
            action: r.action,
            actor_label: r.actor
                .and_then(|id| actor_map.get(&id).cloned())
                .unwrap_or_default(),
            result: r.result,
        })
        .collect();

    let data = DashboardData {
        admin_username: admin.username,
        user_count: users_n,
        client_count: clients_n,
        active_session_count: session_count,
        issuer: app.issuer().to_owned(),
        sparkline,
        warn_smtp_not_configured: !smtp_configured,
        warn_hibp_off: hibp_is_off,
        warn_cookie_insecure: !app.config.server.cookie_secure,
        recent_important,
    };
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_dashboard(data, None, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, serde::Deserialize, Default)]

pub struct DashboardQuery {
    /// `?range=24h` / `?range=7d` / `?range=30d`. Anything else
    /// (or absence) means "use the default".
    pub range: Option<String>,
}

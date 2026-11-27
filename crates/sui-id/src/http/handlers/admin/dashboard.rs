//! Admin handlers for dashboard (RFC 066).

use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdminOrAuditor};
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;
use sui_id_store::repos::{clients, federation_provider, users};
use sui_id_web::{
    pages::{DashboardData, GettingStartedData, KeptBackupOverdue},
    render_dashboard,
};

pub async fn dashboard(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role, _, _): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<DashboardQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let users_n = users::list(&app.db)
        .await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let clients_n = clients::list(&app.db)
        .await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Range comes from ?range=24h|7d|30d. Unknown / missing falls
    // back to the default (Last7Days).
    let range = q
        .range
        .as_deref()
        .and_then(sui_id_core::dashboard::SparklineRange::from_query)
        .unwrap_or_default();
    let activity = sui_id_core::dashboard::login_activity(&app.db, &app.clock, range)
        .await
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
        .map(|r| {
            let t = lang.strings();
            let label = match r {
                sui_id_core::dashboard::SparklineRange::Last24Hours => t.dashboard_range_24h,
                sui_id_core::dashboard::SparklineRange::Last7Days => t.dashboard_range_7d,
                sui_id_core::dashboard::SparklineRange::Last30Days => t.dashboard_range_30d,
            };
            (r.as_query().to_string(), label.to_string())
        })
        .collect::<Vec<_>>();

    let sparkline = sui_id_web::DashboardSparkline {
        active_range_query: range.as_query().to_string(),
        range_options,
        total_success: activity.total_success,
        total_failure: activity.total_failure,
        buckets,
    };

    let session_count = sui_id_store::repos::sessions::count_active_total(&app.db)
        .await
        .unwrap_or(0);
    // HibpMode: Off = show warning; anything else = configured
    let hibp_is_off = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map(|s| matches!(s.hibp_mode, sui_id_store::models::HibpMode::Off))
        .unwrap_or(true); // assume Off if settings missing
    let smtp_configured = sui_id_store::repos::smtp_config::get(&app.db)
        .await
        .map(|o| o.is_some())
        .unwrap_or(false);

    // RFC 073 action items (v0.58.0). Each is a small read-only
    // aggregate; total added latency is well under 20ms on indexed
    // columns. All conditions fail open (best-effort) so a single
    // failing query never breaks the whole dashboard.
    let admins_without_mfa = sui_id_store::repos::users::count_admins_without_mfa(&app.db)
        .await
        .unwrap_or(0);
    let oldest_active_key_age_days = sui_id_store::repos::signing_keys::list_active(&app.db)
        .await
        .ok()
        .and_then(|keys| keys.iter().map(|k| k.created_at).min())
        .map(|oldest| (app.clock.now() - oldest).num_days());
    let outbox_stuck_count = sui_id_store::repos::email_outbox::count_stuck_pending(
        &app.db,
        chrono::Duration::hours(1),
        app.clock.now(),
    )
    .await
    .unwrap_or(0);
    let pending_password_resets =
        sui_id_store::repos::password_reset_tokens::count_outstanding(&app.db, app.clock.now())
            .await
            .unwrap_or(0);

    // RFC 149 §2 / Q3 §2: the first-run path's own done-ness.
    let gs_admin_mfa = sui_id_store::repos::users::has_mfa(&app.db, &admin_id)
        .await
        .unwrap_or(false);
    let federation_providers = federation_provider::list(&app.db).await.unwrap_or_default();
    let getting_started = GettingStartedData {
        mfa_done: gs_admin_mfa,
        smtp_done: smtp_configured,
        first_app_done: clients_n > 0,
        federation_done: !federation_providers.is_empty(),
    };
    let federation_providers_requiring_review = federation_providers
        .iter()
        .filter(|p| p.validation_status == sui_id_store::models::ValidationStatus::RequiresReview)
        .count();

    // RFC 149 §3 / Q3 §3: a kept pre-rotation master-key backup past its
    // own review date, read fresh on every request (never cached) so a
    // `backup renew`/`backup remove` made while the server runs is
    // reflected without a restart. Shares `overdue_backup_review` — the
    // one predicate that decides what counts as overdue — with
    // `admin master-key rotate --check`'s own report; only the
    // presentation differs (no filesystem path here).
    let backup_read_started = std::time::Instant::now();
    let kept_backup_overdue = sui_id_store::master_key_rotation::read(&app.db)
        .await
        .ok()
        .flatten()
        .and_then(|row| {
            let now = app.clock.now();
            sui_id_store::master_key_rotation::overdue_backup_review(&row, now).map(
                |(review_at, custody_reference)| KeptBackupOverdue {
                    review_at,
                    custody_reference: custody_reference.to_owned(),
                },
            )
        });
    tracing::trace!(
        micros = backup_read_started.elapsed().as_micros(),
        "dashboard: kept-backup-overdue read"
    );

    // RFC 043: fetch last 5 important audit events for the dashboard card.
    let audit_rows = sui_id_store::repos::audit::recent_important(&app.db, 5)
        .await
        .unwrap_or_default();
    // Best-effort: resolve actor IDs to usernames.
    let actor_ids: Vec<_> = audit_rows
        .iter()
        .filter_map(|r| r.actor)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let actor_map = sui_id_store::repos::users::resolve_usernames(&app.db, &actor_ids)
        .await
        .unwrap_or_default();
    let recent_important: Vec<sui_id_web::DashboardEventRow> = audit_rows
        .into_iter()
        .map(|r| sui_id_web::DashboardEventRow {
            at: r.at,
            action: r.action,
            actor_label: r
                .actor
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
        sparkline,
        warn_smtp_not_configured: !smtp_configured,
        warn_hibp_off: hibp_is_off,
        warn_cookie_insecure: !app.config.server.cookie_secure,
        admins_without_mfa,
        oldest_active_key_age_days,
        outbox_stuck_count,
        pending_password_resets,
        kept_backup_overdue,
        federation_providers_requiring_review,
        is_administrator: role.is_admin(),
        getting_started,
        recent_important,
    };
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_dashboard(
        data,
        None,
        token.clone(),
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, serde::Deserialize, Default)]

pub struct DashboardQuery {
    /// `?range=24h` / `?range=7d` / `?range=30d`. Anything else
    /// (or absence) means "use the default".
    pub range: Option<String>,
}

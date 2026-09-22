//! RFC 103 5b: parsing an `audit_log` row into a [`RecoveryEventSummary`]
//! for the account page. Pure functions — no database.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use sui_id_store::models::{AuditLogRow, ResetTokenOrigin};

fn row(action: &str, note: Option<&str>) -> AuditLogRow {
    AuditLogRow {
        at: chrono::DateTime::parse_from_rfc3339("2026-09-22T12:00:00Z")
            .expect("time")
            .with_timezone(&chrono::Utc),
        actor: None,
        action: action.to_owned(),
        target: None,
        result: "ok".to_owned(),
        note: note.map(str::to_owned),
    }
}

#[test]
fn issued_by_web_reads_by_admin_true() {
    let r = row(
        "user.recovery_link.issued",
        Some(
            "reason=call-back verified via=web expires_at=2026-09-22T13:00:00Z invalidated=0 step_up=fresh:totp:12",
        ),
    );
    assert_eq!(
        summarize_recent_event(&r),
        Some(RecoveryEventSummary::Issued {
            by_admin: true,
            at: r.at
        })
    );
}

#[test]
fn issued_by_cli_reads_by_admin_false() {
    let r = row(
        "user.recovery_link.issued",
        Some(
            "reason=lost password via=cli expires_at=2026-09-22T13:00:00Z invalidated=1 step_up=not_applicable:system_principal",
        ),
    );
    assert_eq!(
        summarize_recent_event(&r),
        Some(RecoveryEventSummary::Issued {
            by_admin: false,
            at: r.at
        })
    );
}

#[test]
fn completed_reads_each_origin() {
    for (origin_str, origin) in [
        ("email", ResetTokenOrigin::Email),
        ("web", ResetTokenOrigin::Web),
        ("cli", ResetTokenOrigin::Cli),
    ] {
        let r = row(
            "auth.password.reset_completed",
            Some(&format!("origin={origin_str}")),
        );
        assert_eq!(
            summarize_recent_event(&r),
            Some(RecoveryEventSummary::Completed { origin, at: r.at }),
            "{origin_str}"
        );
    }
}

#[test]
fn a_forged_via_earlier_in_the_reason_does_not_win() {
    // The note is `key=value`-joined and not escaped: a reason can contain
    // text that looks like a field. The real `via=` is written last, so
    // reading from the end must pick it, not the forged one.
    let r = row(
        "user.recovery_link.issued",
        Some(
            "reason=x via=cli step_up=not_applicable:system_principal via=web expires_at=2026-09-22T13:00:00Z invalidated=0 step_up=fresh:totp:9",
        ),
    );
    assert_eq!(
        summarize_recent_event(&r),
        Some(RecoveryEventSummary::Issued {
            by_admin: true,
            at: r.at
        }),
        "the last via= (web, the real one) must win over the forged one"
    );
}

#[test]
fn other_actions_are_none() {
    assert_eq!(
        summarize_recent_event(&row("user.disable", Some("reason=x"))),
        None
    );
    assert_eq!(
        summarize_recent_event(&row(
            "auth.password.changed_self",
            Some("sessions_revoked=0")
        )),
        None
    );
}

#[test]
fn a_missing_note_is_none_not_a_panic() {
    assert_eq!(
        summarize_recent_event(&row("user.recovery_link.issued", None)),
        None
    );
    assert_eq!(
        summarize_recent_event(&row("auth.password.reset_completed", None)),
        None
    );
}

#[test]
fn a_note_missing_the_expected_field_is_none() {
    assert_eq!(
        summarize_recent_event(&row(
            "user.recovery_link.issued",
            Some("reason=x expires_at=y")
        )),
        None,
        "no via="
    );
    assert_eq!(
        summarize_recent_event(&row(
            "auth.password.reset_completed",
            Some("sessions_revoked=0")
        )),
        None,
        "no origin="
    );
}

#[test]
fn an_unrecognised_origin_value_is_none() {
    assert_eq!(
        summarize_recent_event(&row(
            "auth.password.reset_completed",
            Some("origin=telepathy")
        )),
        None
    );
}

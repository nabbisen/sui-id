//! Tests for RFC 082 — Authorization Decision Core.
//!
//! Two layers of verification:
//!
//! 1. **Exhaustive table test** — iterates all roles × all actions against
//!    a hand-written expected matrix. This is "double-entry bookkeeping":
//!    the matrix in the test must agree with the match arms in `authz.rs`,
//!    and any discrepancy is caught.
//!
//! 2. **Property tests** — quantify the structural guarantees P1–P5 without
//!    enumerating every cell. These catch rule changes that look locally
//!    correct but break a global invariant.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use sui_id_store::models::Role;

use crate::authz::{authorize, Action, Decision};
use Decision::{Deny, Permit};
use Role::{Admin, Auditor, User};

// ── Action enumeration (test-only) ───────────────────────────────────────────
//
// A hand-maintained list of every Action variant used for exhaustive iteration.
// All variants with embedded booleans appear twice (true + false).

fn all_actions() -> Vec<Action> {
    use Action::*;
    vec![
        AdminReadUsers,
        AdminReadClients,
        AdminReadAudit,
        AdminReadDashboard,
        AdminReadSettings,
        AdminReadSigningKeys,
        AdminWriteUsers,
        AdminWriteClients,
        AdminWriteSettings,
        AdminRotateSigningKey,
        AdminRotateClientSecret,
        AdminResetUserMfa,
        AdminForceLogout,
        AdminChangeUserRole { target_is_last_admin: false },
        AdminChangeUserRole { target_is_last_admin: true },
        AdminDisableUser { target_is_last_admin: false },
        AdminDisableUser { target_is_last_admin: true },
        AdminDeleteUser { target_is_last_admin: false },
        AdminDeleteUser { target_is_last_admin: true },
        SelfReadSecurity,
        SelfWriteSecurity,
        SelfRevokeOwnSessions,
        SelfRevokeConsent,
    ]
}

fn all_roles() -> Vec<Role> {
    vec![Admin, Auditor, User]
}

// ── Expected matrix (hand-written "second copy") ──────────────────────────────
//
// Returns the authoritative expected decision for every (role, action) pair.
// This is the normative reference that the implementation must match.

fn expected(role: Role, action: Action) -> Decision {
    use Action::*;
    use Decision::*;
    use Role::*;

    // Last-admin variants → Deny for every role, every action type.
    match action {
        AdminChangeUserRole { target_is_last_admin: true }
        | AdminDisableUser { target_is_last_admin: true }
        | AdminDeleteUser { target_is_last_admin: true } => return Deny,
        _ => {}
    }

    match (role, action) {
        // Admin: all reads, all writes, non-last-admin mutations.
        (Admin, AdminReadUsers)
        | (Admin, AdminReadClients)
        | (Admin, AdminReadAudit)
        | (Admin, AdminReadDashboard)
        | (Admin, AdminReadSettings)
        | (Admin, AdminReadSigningKeys)
        | (Admin, AdminWriteUsers)
        | (Admin, AdminWriteClients)
        | (Admin, AdminWriteSettings)
        | (Admin, AdminRotateSigningKey)
        | (Admin, AdminRotateClientSecret)
        | (Admin, AdminResetUserMfa)
        | (Admin, AdminForceLogout)
        | (Admin, AdminChangeUserRole { target_is_last_admin: false })
        | (Admin, AdminDisableUser { target_is_last_admin: false })
        | (Admin, AdminDeleteUser { target_is_last_admin: false }) => Permit,

        // Auditor: reads only.
        (Auditor, AdminReadUsers)
        | (Auditor, AdminReadClients)
        | (Auditor, AdminReadAudit)
        | (Auditor, AdminReadDashboard)
        | (Auditor, AdminReadSettings)
        | (Auditor, AdminReadSigningKeys) => Permit,

        // All roles: self-service.
        (_, Action::SelfReadSecurity)
        | (_, Action::SelfWriteSecurity)
        | (_, Action::SelfRevokeOwnSessions)
        | (_, Action::SelfRevokeConsent) => Permit,

        // Everything else: Deny.
        _ => Deny,
    }
}

// ── Exhaustive table test ─────────────────────────────────────────────────────

/// Every (role, action) pair: implementation must match the expected matrix.
#[test]
fn exhaustive_table_matches_expected_matrix() {
    let mut mismatches = Vec::new();
    for role in all_roles() {
        for action in all_actions() {
            let got = authorize(role, action);
            let want = expected(role, action);
            if got != want {
                mismatches.push(format!(
                    "({role:?}, {action:?}): got {got:?}, expected {want:?}"
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "authorization table does not match expected matrix:\n{}",
        mismatches.join("\n")
    );
}

// ── P1: deny-by-default — no panics, full coverage ───────────────────────────

/// The function must not panic for any (role, action) combination.
/// (If a match arm were missing the compiler would catch it, but this
/// test also acts as a canary for any future refactor that uses
/// `#[allow(unreachable_patterns)]`.)
#[test]
fn authorize_is_total_no_panics() {
    for role in all_roles() {
        for action in all_actions() {
            let _ = authorize(role, action); // must not panic
        }
    }
}

// ── P2: role monotonicity ─────────────────────────────────────────────────────

/// If User is permitted an action, Admin must also be permitted.
#[test]
fn user_permit_implies_admin_permit() {
    let mut violations = Vec::new();
    for action in all_actions() {
        if authorize(User, action) == Permit && authorize(Admin, action) != Permit {
            violations.push(format!("{action:?}"));
        }
    }
    assert!(
        violations.is_empty(),
        "P2 violation: User permitted but Admin denied:\n{}",
        violations.join("\n")
    );
}

/// For read actions, if Auditor is permitted, Admin must also be permitted.
#[test]
fn auditor_permit_implies_admin_permit_on_reads() {
    let read_actions: Vec<Action> = all_actions()
        .into_iter()
        .filter(|a| {
            matches!(
                a,
                Action::AdminReadUsers
                    | Action::AdminReadClients
                    | Action::AdminReadAudit
                    | Action::AdminReadDashboard
                    | Action::AdminReadSettings
                    | Action::AdminReadSigningKeys
            )
        })
        .collect();
    let mut violations = Vec::new();
    for action in read_actions {
        if authorize(Auditor, action) == Permit && authorize(Admin, action) != Permit {
            violations.push(format!("{action:?}"));
        }
    }
    assert!(
        violations.is_empty(),
        "P2 violation: Auditor permitted read but Admin denied:\n{}",
        violations.join("\n")
    );
}

// ── P3: read/write separation — Auditor cannot mutate ────────────────────────

fn mutation_actions() -> Vec<Action> {
    use Action::*;
    vec![
        AdminWriteUsers,
        AdminWriteClients,
        AdminWriteSettings,
        AdminRotateSigningKey,
        AdminRotateClientSecret,
        AdminResetUserMfa,
        AdminForceLogout,
        AdminChangeUserRole { target_is_last_admin: false },
        AdminDisableUser { target_is_last_admin: false },
        AdminDeleteUser { target_is_last_admin: false },
    ]
}

/// Auditor must be denied every mutation action.
#[test]
fn auditor_is_denied_all_mutations() {
    let mut violations = Vec::new();
    for action in mutation_actions() {
        if authorize(Auditor, action) == Permit {
            violations.push(format!("{action:?}"));
        }
    }
    assert!(
        violations.is_empty(),
        "P3 violation: Auditor permitted mutation:\n{}",
        violations.join("\n")
    );
}

// ── P4: last-admin protection ─────────────────────────────────────────────────

/// Every role must be denied the three last-admin actions.
#[test]
fn last_admin_actions_deny_for_all_roles() {
    use Action::*;
    let last_admin_actions = [
        AdminChangeUserRole { target_is_last_admin: true },
        AdminDisableUser { target_is_last_admin: true },
        AdminDeleteUser { target_is_last_admin: true },
    ];
    let mut violations = Vec::new();
    for role in all_roles() {
        for action in last_admin_actions {
            if authorize(role, action) == Permit {
                violations.push(format!("({role:?}, {action:?})"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "P4 violation: last-admin action permitted:\n{}",
        violations.join("\n")
    );
}

// ── P5: self-scope — User permits ⊆ Self* actions ────────────────────────────

fn admin_scoped_actions() -> Vec<Action> {
    use Action::*;
    vec![
        AdminReadUsers,
        AdminReadClients,
        AdminReadAudit,
        AdminReadDashboard,
        AdminReadSettings,
        AdminReadSigningKeys,
        AdminWriteUsers,
        AdminWriteClients,
        AdminWriteSettings,
        AdminRotateSigningKey,
        AdminRotateClientSecret,
        AdminResetUserMfa,
        AdminForceLogout,
        AdminChangeUserRole { target_is_last_admin: false },
        AdminDisableUser { target_is_last_admin: false },
        AdminDeleteUser { target_is_last_admin: false },
    ]
}

/// User must be denied every admin-scoped action.
#[test]
fn user_is_denied_all_admin_actions() {
    let mut violations = Vec::new();
    for action in admin_scoped_actions() {
        if authorize(User, action) == Permit {
            violations.push(format!("{action:?}"));
        }
    }
    assert!(
        violations.is_empty(),
        "P5 violation: User permitted admin action:\n{}",
        violations.join("\n")
    );
}

// ── Spot checks for key invariants ───────────────────────────────────────────

#[test]
fn admin_permitted_non_last_admin_mutations() {
    use Action::*;
    assert_eq!(
        authorize(Admin, AdminChangeUserRole { target_is_last_admin: false }),
        Permit
    );
    assert_eq!(
        authorize(Admin, AdminDisableUser { target_is_last_admin: false }),
        Permit
    );
    assert_eq!(
        authorize(Admin, AdminDeleteUser { target_is_last_admin: false }),
        Permit
    );
}

#[test]
fn self_service_permitted_for_all_roles() {
    use Action::*;
    for role in all_roles() {
        assert_eq!(authorize(role, SelfReadSecurity), Permit, "{role:?}");
        assert_eq!(authorize(role, SelfWriteSecurity), Permit, "{role:?}");
        assert_eq!(authorize(role, SelfRevokeOwnSessions), Permit, "{role:?}");
        assert_eq!(authorize(role, SelfRevokeConsent), Permit, "{role:?}");
    }
}

#[test]
fn auditor_permitted_all_read_surfaces() {
    use Action::*;
    for action in [
        AdminReadUsers,
        AdminReadClients,
        AdminReadAudit,
        AdminReadDashboard,
        AdminReadSettings,
        AdminReadSigningKeys,
    ] {
        assert_eq!(authorize(Auditor, action), Permit, "{action:?}");
    }
}

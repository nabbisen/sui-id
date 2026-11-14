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

use crate::authz::{Action, Decision, authorize};
use Decision::Permit;
use Role::{Admin, Auditor, User};

// ── Every Action and every Role, from one list each (0.91.0 S3) ──────────────
//
// Each macro below takes its list once and generates two things from it:
// the function the tests iterate (`all_actions()`, `all_roles()`), and a
// `match` over that same list with no wildcard arm. So the list cannot fall
// behind its enum:
// - a new variant that is not listed is a non-exhaustive match (E0004), and
//   the build fails;
// - listing it is what makes the tests iterate it, since both come from the
//   same tokens;
// - a variant with a `bool` must be listed with both values, or the match is
//   non-exhaustive;
// - an entry listed twice is an unreachable pattern, which `-D warnings`
//   refuses.
// These replace a hand-kept list that a new variant could silently skip.

macro_rules! every_action {
    ($($v:ident $({ $f:ident : $b:literal })?),+ $(,)?) => {
        fn all_actions() -> Vec<Action> {
            use Action::*;
            vec![$($v $({ $f: $b })?),+]
        }

        /// Compiled, never called: it fails to build if `every_action!`'s
        /// list misses a value of `Action`.
        #[allow(dead_code)]
        fn every_action_is_listed(a: Action) {
            use Action::*;
            match a {
                $($v $({ $f: $b })? => {})+
            }
        }
    };
}

every_action! {
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
}

macro_rules! every_role {
    ($($v:ident),+ $(,)?) => {
        fn all_roles() -> Vec<Role> {
            vec![$($v),+]
        }

        /// Compiled, never called: it fails to build if `every_role!`'s list
        /// misses a value of `Role`.
        #[allow(dead_code)]
        fn every_role_is_listed(r: Role) {
            match r {
                $($v => {})+
            }
        }
    };
}

every_role! { Admin, Auditor, User }

/// The counts, measured 2026-11-14: 20 `Action` variants, three of them
/// carrying a `bool`, so 23 values; and 3 roles. A change to either is a
/// change to the authorization table, so this says so in the test output.
#[test]
fn the_lists_have_the_measured_sizes() {
    assert_eq!(
        all_actions().len(),
        23,
        "Action values changed: update the expected matrix below"
    );
    assert_eq!(
        all_roles().len(),
        3,
        "Role values changed: update the expected matrix below"
    );
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
        AdminChangeUserRole {
            target_is_last_admin: true,
        }
        | AdminDisableUser {
            target_is_last_admin: true,
        }
        | AdminDeleteUser {
            target_is_last_admin: true,
        } => return Deny,
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
        | (
            Admin,
            AdminChangeUserRole {
                target_is_last_admin: false,
            },
        )
        | (
            Admin,
            AdminDisableUser {
                target_is_last_admin: false,
            },
        )
        | (
            Admin,
            AdminDeleteUser {
                target_is_last_admin: false,
            },
        ) => Permit,

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
        AdminChangeUserRole {
            target_is_last_admin: false,
        },
        AdminDisableUser {
            target_is_last_admin: false,
        },
        AdminDeleteUser {
            target_is_last_admin: false,
        },
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
        AdminChangeUserRole {
            target_is_last_admin: true,
        },
        AdminDisableUser {
            target_is_last_admin: true,
        },
        AdminDeleteUser {
            target_is_last_admin: true,
        },
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
        AdminChangeUserRole {
            target_is_last_admin: false,
        },
        AdminDisableUser {
            target_is_last_admin: false,
        },
        AdminDeleteUser {
            target_is_last_admin: false,
        },
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
        authorize(
            Admin,
            AdminChangeUserRole {
                target_is_last_admin: false
            }
        ),
        Permit
    );
    assert_eq!(
        authorize(
            Admin,
            AdminDisableUser {
                target_is_last_admin: false
            }
        ),
        Permit
    );
    assert_eq!(
        authorize(
            Admin,
            AdminDeleteUser {
                target_is_last_admin: false
            }
        ),
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

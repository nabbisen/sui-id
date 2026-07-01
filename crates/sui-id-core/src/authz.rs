//! Authorization decision core (RFC 082).
//!
//! A single pure function — [`authorize`] — that maps (role, action) pairs
//! to [`Decision`]s. Every enforcement point in the codebase delegates to
//! this table; the table is the single normative artifact for the project's
//! authorization rules.
//!
//! **Design principles (from the security strategy):**
//! - *Deny by default.* The match table's final arm is `=> Deny`; any
//!   unlisted pair denies without a code-review finding.
//! - *Pure and total.* No I/O, no DB, no clock. Environmental facts (e.g.
//!   "is this the last admin?") are computed by the caller and passed in
//!   as data on the [`Action`] variant.
//! - *Exhaustive enum.* Every privileged action is a variant. Adding a new
//!   privileged handler without adding a variant and a table row is a
//!   compile error (the `Action` must be matched somewhere).
//!
//! Security properties verified by the tests in `authz/tests.rs`:
//! - **P1** Deny-by-default: all (role, action) combinations are covered;
//!   no panic; no missing arm.
//! - **P2** Role monotonicity: permit(User, a) ⇒ permit(Admin, a) for
//!   every action.
//! - **P3** Read/write separation: Auditor permits ⊆ read-only actions;
//!   authorize(Auditor, mut_action) = Deny for every mutation.
//! - **P4** Last-admin protection: the three last-admin variants deny for
//!   every role, including Admin.
//! - **P5** Self-scope: User permits ⊆ Self* actions.

use sui_id_store::models::Role;

/// Every privileged action in the system, enumerated.
///
/// Environmental booleans (e.g. `target_is_last_admin`) are embedded in
/// the variant rather than passed as separate parameters so the function
/// remains a pure `(Role, Action) → Decision` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // ── Admin read surfaces ───────────────────────────────────────────────
    AdminReadUsers,
    AdminReadClients,
    AdminReadAudit,
    AdminReadDashboard,
    AdminReadSettings,
    AdminReadSigningKeys,

    // ── Admin mutations ───────────────────────────────────────────────────
    AdminWriteUsers,
    AdminWriteClients,
    AdminWriteSettings,
    AdminRotateSigningKey,
    AdminRotateClientSecret,
    AdminResetUserMfa,
    AdminForceLogout,

    // ── Last-admin–protected mutations ────────────────────────────────────
    /// Change a user's role. Denied for every role when the target is the
    /// last active admin (passing `target_is_last_admin = true`).
    AdminChangeUserRole {
        target_is_last_admin: bool,
    },
    /// Disable a user account. Denied when the target is the last admin.
    AdminDisableUser {
        target_is_last_admin: bool,
    },
    /// Soft-delete a user account. Denied when the target is the last admin.
    AdminDeleteUser {
        target_is_last_admin: bool,
    },

    // ── Self-service ──────────────────────────────────────────────────────
    SelfReadSecurity,
    SelfWriteSecurity,
    SelfRevokeOwnSessions,
    SelfRevokeConsent,
}

/// The outcome of an authorization check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Permit,
    Deny,
}

/// Evaluate whether `role` may perform `action`.
///
/// This is the single normative authorization table. All enforcement points
/// — extractor conversions ([`crate::actor`]), the last-admin safeguard,
/// handler guards — must call this function rather than re-implementing
/// any slice of the rules.
///
/// # Deny-by-default
///
/// The final match arm is `_ => Deny`. Any (role, action) pair that is not
/// explicitly permitted here is denied. A new action variant without a
/// corresponding table row will fall through to `Deny`.
pub fn authorize(role: Role, action: Action) -> Decision {
    use Action::*;
    use Decision::*;
    use Role::*;

    match (role, action) {
        // ── Admin: all read surfaces ──────────────────────────────────────
        (Admin, AdminReadUsers)
        | (Admin, AdminReadClients)
        | (Admin, AdminReadAudit)
        | (Admin, AdminReadDashboard)
        | (Admin, AdminReadSettings)
        | (Admin, AdminReadSigningKeys) => Permit,

        // ── Auditor: read surfaces only (P3) ─────────────────────────────
        (Auditor, AdminReadUsers)
        | (Auditor, AdminReadClients)
        | (Auditor, AdminReadAudit)
        | (Auditor, AdminReadDashboard)
        | (Auditor, AdminReadSettings)
        | (Auditor, AdminReadSigningKeys) => Permit,

        // ── Admin: mutations ──────────────────────────────────────────────
        (Admin, AdminWriteUsers)
        | (Admin, AdminWriteClients)
        | (Admin, AdminWriteSettings)
        | (Admin, AdminRotateSigningKey)
        | (Admin, AdminRotateClientSecret)
        | (Admin, AdminResetUserMfa)
        | (Admin, AdminForceLogout) => Permit,

        // ── Last-admin–protected: deny when target is the last admin ──────
        // Even an Admin cannot demote, disable, or delete the last admin
        // (P4). When `target_is_last_admin = false` the Admin is permitted.
        (
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

        // Any role, target is the last admin → always deny (P4).
        (
            _,
            AdminChangeUserRole {
                target_is_last_admin: true,
            },
        )
        | (
            _,
            AdminDisableUser {
                target_is_last_admin: true,
            },
        )
        | (
            _,
            AdminDeleteUser {
                target_is_last_admin: true,
            },
        ) => Deny,

        // Non-last-admin targets, non-Admin roles → deny (Auditor/User
        // cannot mutate users regardless of last-admin status).
        (_, AdminChangeUserRole { .. })
        | (_, AdminDisableUser { .. })
        | (_, AdminDeleteUser { .. }) => Deny,

        // ── Self-service: any authenticated user (P5) ─────────────────────
        (_, SelfReadSecurity)
        | (_, SelfWriteSecurity)
        | (_, SelfRevokeOwnSessions)
        | (_, SelfRevokeConsent) => Permit,

        // ── Deny by default (P1) ─────────────────────────────────────────
        _ => Deny,
    }
}

/// Kani verification harnesses (RFC 086, Pilot K).
///
/// These harnesses are only compiled when `--cfg=kani` is active (i.e. when
/// running `cargo kani`). They have zero cost in normal builds. They prove
/// RFC 082 properties P1–P5 exhaustively over the finite input space.
///
/// To run (requires Kani to be installed):
/// ```sh
/// cargo kani --tests -- authz::verify_p1 authz::verify_p2 authz::verify_p3 \
///            authz::verify_p4 authz::verify_p5
/// ```
#[cfg(kani)]
mod kani_proofs {
    use super::*;
    use kani::Arbitrary;

    /// P1 (deny-by-default): the function terminates for every input.
    #[kani::proof]
    fn verify_p1_totality() {
        let role: Role = kani::any();
        let action: Action = kani::any();
        // If this proof succeeds, the function is total (no panics, no hangs).
        let _ = authorize(role, action);
    }

    /// P2 (role monotonicity): if User is permitted, Admin must also be permitted.
    #[kani::proof]
    fn verify_p2_monotonicity_user_implies_admin() {
        let action: Action = kani::any();
        if authorize(Role::User, action) == Decision::Permit {
            kani::assert(
                authorize(Role::Admin, action) == Decision::Permit,
                "P2: User permitted but Admin denied",
            );
        }
    }

    /// P3 (read/write separation): Auditor is denied every mutation action.
    #[kani::proof]
    fn verify_p3_auditor_write_impossibility() {
        let action: Action = kani::any();
        kani::assume(matches!(
            action,
            Action::AdminWriteUsers
                | Action::AdminWriteClients
                | Action::AdminWriteSettings
                | Action::AdminRotateSigningKey
                | Action::AdminRotateClientSecret
                | Action::AdminResetUserMfa
                | Action::AdminForceLogout
                | Action::AdminChangeUserRole { .. }
                | Action::AdminDisableUser { .. }
                | Action::AdminDeleteUser { .. }
        ));
        kani::assert(
            authorize(Role::Auditor, action) == Decision::Deny,
            "P3: Auditor permitted a mutation action",
        );
    }

    /// P4 (last-admin protection): last-admin variants deny for every role.
    #[kani::proof]
    fn verify_p4_last_admin_denial() {
        let role: Role = kani::any();
        let action: Action = kani::any();
        kani::assume(matches!(
            action,
            Action::AdminChangeUserRole {
                target_is_last_admin: true
            } | Action::AdminDisableUser {
                target_is_last_admin: true
            } | Action::AdminDeleteUser {
                target_is_last_admin: true
            }
        ));
        kani::assert(
            authorize(role, action) == Decision::Deny,
            "P4: last-admin action permitted",
        );
    }

    /// P5 (self-scope): User permits are confined to Self* actions.
    #[kani::proof]
    fn verify_p5_user_self_scope() {
        let action: Action = kani::any();
        kani::assume(!matches!(
            action,
            Action::SelfReadSecurity
                | Action::SelfWriteSecurity
                | Action::SelfRevokeOwnSessions
                | Action::SelfRevokeConsent
        ));
        kani::assert(
            authorize(Role::User, action) == Decision::Deny,
            "P5: User permitted a non-Self action",
        );
    }
}

#[cfg(test)]
mod tests;

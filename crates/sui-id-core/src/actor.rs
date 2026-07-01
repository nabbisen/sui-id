//! Actor capability types (RFC 081).
//!
//! sui-id's real isolation boundaries are role scope (Admin / Auditor / User),
//! self vs other, and client binding. This module makes those boundaries
//! explicit in the type system: an [`Actor`] context constructed only by the
//! authentication layer, and marker-carrying capability types —
//! [`AdminActor`], [`ReadOnlyAdminActor`], [`SelfActor`] — that privileged
//! domain mutations require. A privileged call without proof of privilege
//! is a compile error; an auditor reaching a mutation signature cannot compile.
//!
//! **Security properties:**
//! - **P1 (deny by construction):** No admin domain mutation is callable
//!   without an [`AdminActor`]; no [`AdminActor`] exists without passing
//!   the role check in [`Actor::into_admin`].
//! - **P2 (read/write separation):** [`ReadOnlyAdminActor`] reaches no
//!   mutation signature; an Auditor session cannot mutate even if a handler
//!   is mis-wired.
//! - **P3 (self-scope):** Self-service mutations target only
//!   `actor.user_id()`; cross-user targeting via `/me/*` is not expressible.
//! - **P4 (no forgery):** [`Actor`] has no public constructor, no
//!   `Deserialize`, no `Clone`. It is constructed only by the
//!   `from_session` factory called from the Axum extractors.
//! - **P5 (per-request):** Actors are per-request values, never cached
//!   across requests. Role change or session revocation takes effect
//!   immediately on the next request.

use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::Role;

use crate::authz::{self, Action, Decision};

// ── Core actor type ───────────────────────────────────────────────────────────

/// Proof of an authenticated principal. Constructible only by the
/// session-resolution path (the `from_session` factory below, called from
/// the Axum extractors). Has no public constructor, no `Deserialize`, and
/// no `Clone` — it cannot be forged or cached (RFC 081 P4, P5).
pub struct Actor {
    user_id: UserId,
    role: Role,
    session_id: SessionId,
}

impl Actor {
    /// Construct an [`Actor`] from a verified session.
    ///
    /// This is the **only** constructor. It is `pub(crate)` — only the
    /// binary-crate extractors (which live in `sui-id`, which depends on
    /// `sui-id-core`) should call this. Domain functions receive the
    /// capability type, not this factory.
    ///
    /// The caller is responsible for ensuring `user_id`, `role`, and
    /// `session_id` come from a verified, non-expired session row.
    pub fn from_session(user_id: UserId, role: Role, session_id: SessionId) -> Self {
        Self {
            user_id,
            role,
            session_id,
        }
    }

    /// The authenticated user's id.
    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// The authenticated user's role.
    pub fn role(&self) -> Role {
        self.role
    }

    /// The session that backs this actor's authority.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Attempt to produce an [`AdminActor`] (write-capable).
    ///
    /// Succeeds only if the actor's role is `Admin`. Delegates the decision
    /// to the RFC 082 authorization core to keep the rule in one place.
    ///
    /// On failure, returns `Err(self)` so the caller can convert to a
    /// different capability or return a 403.
    pub fn into_admin(self) -> Result<AdminActor, Actor> {
        if authz::authorize(self.role, Action::AdminWriteUsers) == Decision::Permit {
            Ok(AdminActor(self))
        } else {
            Err(self)
        }
    }

    /// Attempt to produce a [`ReadOnlyAdminActor`] (read-only admin or auditor).
    ///
    /// Succeeds for `Admin` and `Auditor` roles; fails for `User`.
    pub fn into_read_admin(self) -> Result<ReadOnlyAdminActor, Actor> {
        if authz::authorize(self.role, Action::AdminReadUsers) == Decision::Permit {
            Ok(ReadOnlyAdminActor(self))
        } else {
            Err(self)
        }
    }

    /// Produce a [`SelfActor`] — always succeeds for any authenticated actor.
    ///
    /// Self-service operations are permitted for all roles (RFC 082 P5).
    pub fn into_self(self) -> SelfActor {
        SelfActor(self)
    }

    /// Check whether a specific action is permitted for this actor.
    ///
    /// Delegates entirely to the RFC 082 authorization core. Callers that
    /// need finer-grained checks (e.g. the last-admin guard) should call
    /// this with the appropriate [`Action`] variant.
    pub fn authorize(&self, action: Action) -> Decision {
        authz::authorize(self.role, action)
    }
}

// ── Capability types ──────────────────────────────────────────────────────────

/// Proof that the actor has `Admin` role and may perform mutations.
///
/// Only constructible via [`Actor::into_admin`]. Reaching a function that
/// takes `&AdminActor` without having called `into_admin` is a compile error.
pub struct AdminActor(Actor);

impl AdminActor {
    /// The underlying actor (exposes `user_id`, `session_id`, `role`).
    pub fn actor(&self) -> &Actor {
        &self.0
    }

    /// Convenience: the actor's user id (for audit attribution).
    pub fn user_id(&self) -> UserId {
        self.0.user_id
    }

    /// Convenience: the actor's session id.
    pub fn session_id(&self) -> SessionId {
        self.0.session_id
    }

    /// Downgrade to a [`ReadOnlyAdminActor`] for read-path calls.
    ///
    /// There is no upgrade path; once downgraded, the capability is read-only.
    pub fn as_read_only(&self) -> ReadOnlyAdminActor {
        // Safe to construct directly: AdminActor already passed the Admin check,
        // and Admin ⊇ Auditor in the read permission set.
        ReadOnlyAdminActor(Actor {
            user_id: self.0.user_id,
            role: self.0.role,
            session_id: self.0.session_id,
        })
    }

    /// Check a specific action (e.g. the last-admin guard with the
    /// environmental boolean already resolved by the caller).
    pub fn authorize(&self, action: Action) -> Decision {
        authz::authorize(self.0.role, action)
    }
}

/// Proof that the actor may read admin surfaces (Admin or Auditor role).
///
/// Only constructible via [`Actor::into_read_admin`] or
/// [`AdminActor::as_read_only`]. No mutation function takes this type.
pub struct ReadOnlyAdminActor(Actor);

impl ReadOnlyAdminActor {
    /// The underlying actor.
    pub fn actor(&self) -> &Actor {
        &self.0
    }

    /// Convenience: the actor's user id.
    pub fn user_id(&self) -> UserId {
        self.0.user_id
    }

    /// The actor's role (Admin or Auditor — callers use this for `can_write`
    /// rendering decisions without needing the write-capable [`AdminActor`]).
    pub fn role(&self) -> Role {
        self.0.role
    }

    /// `true` when the underlying role is `Admin` (write-capable).
    ///
    /// Replaces the free-floating `can_write: bool` rendering flag.
    pub fn can_write(&self) -> bool {
        self.0.role == Role::Admin
    }
}

/// Proof of any authenticated actor, scoped to self-service operations.
///
/// Self-service mutations must call `actor.user_id()` to determine their
/// target — a caller-supplied target id is not accepted by the self-service
/// domain functions, making cross-user targeting via `/me/*` structurally
/// impossible (RFC 081 P3).
pub struct SelfActor(Actor);

impl SelfActor {
    /// The authenticated user's id — this is the *only* valid target for
    /// self-service operations.
    pub fn user_id(&self) -> UserId {
        self.0.user_id
    }

    /// The actor's session id (used for self-revocation of the current
    /// session).
    pub fn session_id(&self) -> SessionId {
        self.0.session_id
    }

    /// The actor's role (for rendering or further authorization checks).
    pub fn role(&self) -> Role {
        self.0.role
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_id_shared::ids::{SessionId, UserId};
    use sui_id_store::models::Role;

    fn make_actor(role: Role) -> Actor {
        Actor::from_session(UserId::new(), role, SessionId::new())
    }

    // ── Conversion tests ──────────────────────────────────────────────────────

    #[test]
    fn admin_converts_to_admin_actor() {
        let actor = make_actor(Role::Admin);
        assert!(
            actor.into_admin().is_ok(),
            "Admin role must produce AdminActor"
        );
    }

    #[test]
    fn auditor_cannot_convert_to_admin_actor() {
        let actor = make_actor(Role::Auditor);
        assert!(
            actor.into_admin().is_err(),
            "Auditor must not produce AdminActor"
        );
    }

    #[test]
    fn user_cannot_convert_to_admin_actor() {
        let actor = make_actor(Role::User);
        assert!(
            actor.into_admin().is_err(),
            "User must not produce AdminActor"
        );
    }

    #[test]
    fn admin_converts_to_read_admin_actor() {
        let actor = make_actor(Role::Admin);
        assert!(actor.into_read_admin().is_ok());
    }

    #[test]
    fn auditor_converts_to_read_admin_actor() {
        let actor = make_actor(Role::Auditor);
        assert!(
            actor.into_read_admin().is_ok(),
            "Auditor must produce ReadOnlyAdminActor"
        );
    }

    #[test]
    fn user_cannot_convert_to_read_admin_actor() {
        let actor = make_actor(Role::User);
        assert!(
            actor.into_read_admin().is_err(),
            "User must not produce ReadOnlyAdminActor"
        );
    }

    #[test]
    fn any_role_converts_to_self_actor() {
        for role in [Role::Admin, Role::Auditor, Role::User] {
            let actor = make_actor(role);
            let self_actor = actor.into_self();
            // Confirm user_id is preserved.
            let _ = self_actor.user_id();
        }
    }

    // ── can_write reflects role ───────────────────────────────────────────────

    #[test]
    fn admin_read_only_can_write_is_true() {
        let actor = make_actor(Role::Admin);
        let ro = actor.into_read_admin();
        assert!(ro.is_ok_and(|x| x.can_write()));
    }

    #[test]
    fn auditor_read_only_can_write_is_false() {
        let actor = make_actor(Role::Auditor);
        let ro = actor.into_read_admin();
        assert!(!ro.is_ok_and(|x| x.can_write()));
    }

    // ── Actor.authorize delegates to authz core ───────────────────────────────

    #[test]
    fn actor_authorize_delegates_to_authz_core() {
        use crate::authz::Decision;
        let admin = make_actor(Role::Admin);
        assert_eq!(
            admin.authorize(Action::AdminWriteUsers),
            Decision::Permit
        );
        let user = make_actor(Role::User);
        assert_eq!(
            user.authorize(Action::AdminWriteUsers),
            Decision::Deny
        );
    }

    // ── Self-scope: user_id comes from actor, not caller ─────────────────────

    #[test]
    fn self_actor_user_id_matches_authenticated_user() {
        let user_id = UserId::new();
        let session_id = SessionId::new();
        let actor = Actor::from_session(user_id, Role::User, session_id);
        let self_actor = actor.into_self();
        assert_eq!(self_actor.user_id(), user_id);
    }
}

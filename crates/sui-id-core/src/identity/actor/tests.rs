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
    let ro = actor.into_read_admin().unwrap();
    assert!(ro.can_write());
}

#[test]
fn auditor_read_only_can_write_is_false() {
    let actor = make_actor(Role::Auditor);
    let ro = actor.into_read_admin().unwrap();
    assert!(!ro.can_write());
}

// ── Actor.authorize delegates to authz core ───────────────────────────────

#[test]
fn actor_authorize_delegates_to_authz_core() {
    use crate::authz::Decision;
    let admin = make_actor(Role::Admin);
    assert_eq!(admin.authorize(Action::AdminWriteUsers), Decision::Permit);
    let user = make_actor(Role::User);
    assert_eq!(user.authorize(Action::AdminWriteUsers), Decision::Deny);
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

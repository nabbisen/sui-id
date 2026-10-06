use super::*;
use crate::actor::Actor;
use crate::time::system_clock;
use sui_id_shared::ids::SessionId;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::Role;

fn admin_actor_for(user_id: UserId) -> crate::actor::AdminActor {
    Actor::from_session(user_id, Role::Admin, SessionId::new())
        .into_admin()
        .expect("admin actor")
}

#[tokio::test]
async fn create_user_allocates_fresh_user_id_not_actor_id() {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let clock = system_clock();
    let actor_id = UserId::new();
    let actor = admin_actor_for(actor_id);

    let created = create_user(
        &db,
        &clock,
        &actor,
        CreateUserSpec {
            username: "created",
            display_name: None,
            email: None,
            is_admin: false,
        },
    )
    .await
    .expect("create user");

    assert_ne!(created.id, actor_id);
    // RFC 115 D4: no password is chosen at creation, so no credential row.
    assert!(matches!(
        sui_id_store::repos::credentials::get(&db, created.id).await,
        Err(sui_id_store::StoreError::NotFound)
    ));
}

//! RFC 094 M2b wave 4 stage 4, §1 — `create_client` rebuilds the
//! redirect-origins cache `crates/sui-id/src/http/cors.rs:100`'s comment
//! has always claimed it does ("The cache is rebuilt on every client
//! mutation"), a claim the fix landed in stage 3 made true for the first
//! time and nothing had guarded until now.

use super::common::*;

#[tokio::test]
async fn create_client_makes_its_redirect_origin_visible_to_cors_without_a_restart() {
    use sui_id_core::admin::CreateClientSpec;

    let state = test_app();
    complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;

    let origin = "https://new-rp.test";
    assert!(
        !state.caches.redirect_origins.contains(origin).await,
        "sanity: this origin must not already be in the cache"
    );

    sui_id_core::admin::create_client(
        &state.db,
        &state.clock,
        &admin_actor_for(admin_id),
        CreateClientSpec {
            name: "new-rp",
            redirect_uris: &[format!("{origin}/cb")],
            confidential: false,
            allowed_scopes: "",
            post_logout_redirect_uris: &[],
        },
        &state.caches,
    )
    .await
    .expect("create");

    assert!(
        state.caches.redirect_origins.contains(origin).await,
        "the new client's redirect origin must be visible to CORS \
         immediately after create -- no restart, no second mutation"
    );
}

//! RFC 134 step 3 (D2) — proving the validating resolver is actually
//! wired into the real federation egress client, not merely correct in
//! isolation. The resolver's own rules (every vendored prefix, both
//! edges, the count bound, the "reject the whole answer, don't filter"
//! behavior) are unit-tested directly in `crates/sui-id/src/runtime/
//! resolver.rs` — pure functions, no network needed. This file needs
//! real (loopback) network because what it proves can't be proven any
//! other way: that `egress::build_federation_client()` really refuses a
//! name that resolves to a denied address, not just that the resolver
//! *would* refuse it if asked directly.

use sui_id::egress::build_federation_client;

#[tokio::test]
async fn the_real_egress_client_refuses_a_name_that_resolves_to_loopback() {
    // A real server, really listening -- so a refused connection below
    // is provably the resolver's doing, not "nothing was there to
    // answer."
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
        axum::serve(listener, app).await.expect("mock server");
    });

    let client = build_federation_client();
    // "localhost" resolves through `tokio::net::lookup_host` (the real
    // DNS path `ValidatingResolver` sits in front of) to 127.0.0.1 and/or
    // ::1 -- Loopback, denied. An IP literal would bypass resolution (and
    // this test) entirely, which is deliberately not what this uses.
    let result = client
        .get(format!("http://localhost:{}/", addr.port()))
        .send()
        .await;
    assert!(
        result.is_err(),
        "a loopback-resolving name must be refused even though a real \
         server is listening there"
    );

    // Control: the same server, reached by its real address directly
    // with a plain client (no resolver involved -- nothing to resolve),
    // answers -- proving the refusal above is the resolver's doing, not
    // a network or server problem that would have failed either way.
    let direct = reqwest::Client::new()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .expect("a direct connection to the real address must succeed");
    assert!(direct.status().is_success());
}

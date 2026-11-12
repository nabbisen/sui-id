//! RFC 096-B2 S6b — proving the one assumption the rest of this stage's
//! migration depends on, rather than taking it on faith: that `reqwest`'s
//! custom-resolver connector uses the request URL's own port, and only
//! takes an *address* from the resolver's answer.
//!
//! [`LoopbackFixtureResolver`] (`tls_mock.rs`) resolves [`FIXTURE_HOST`] to
//! `127.0.0.1:0` -- port `0`, which answers nothing on its own -- on the
//! premise that the real port always comes from the URL being connected
//! to. If that premise were wrong, every `serve_https` mock sharing one
//! name would collide on whichever port the resolver happened to name,
//! and the migration in this stage (moving mocks off IP-literal URLs onto
//! `fixture.test`) would be unsound. This test runs two mocks, on two
//! different real ports, through one client built with one
//! `LoopbackFixtureResolver`, and checks that each answers with its own
//! distinct body -- not the other's, and not a connection failure.

use super::tls_mock::{FIXTURE_HOST, LoopbackFixtureResolver, federation_test_client, serve_https};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_mocks_on_two_ports_reach_through_one_client_and_one_resolver() {
    let first_url = serve_https(|_base| {
        axum::Router::new().route("/", axum::routing::get(|| async { "first mock" }))
    })
    .await;
    let second_url = serve_https(|_base| {
        axum::Router::new().route("/", axum::routing::get(|| async { "second mock" }))
    })
    .await;

    assert!(
        first_url.starts_with(&format!("https://{FIXTURE_HOST}:")),
        "both mocks must advertise the fixture host, not an IP literal: {first_url}"
    );
    assert_ne!(
        first_url, second_url,
        "the two mocks must be on two different real ports"
    );

    let client = federation_test_client(LoopbackFixtureResolver);

    let first_body = client
        .get(&first_url)
        .send()
        .await
        .expect("reach the first mock")
        .text()
        .await
        .expect("read first body");
    let second_body = client
        .get(&second_url)
        .send()
        .await
        .expect("reach the second mock")
        .text()
        .await
        .expect("read second body");

    assert_eq!(
        first_body, "first mock",
        "the connection to the first URL must use ITS OWN port, not the \
         resolver's placeholder port 0 -- a wrong answer here would mean \
         the resolver's port is winning, not the URL's"
    );
    assert_eq!(
        second_body, "second mock",
        "same proof, for the second mock on its own different port -- \
         together these rule out 'it happened to work for one port'"
    );
}

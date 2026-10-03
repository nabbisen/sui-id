//! RFC 134 step 1 — the federation egress client's policy, each setting
//! proven present by a test that fails when the line is removed.
//!
//! `reqwest::Client` exposes no public getters for most builder settings,
//! and `{:?}` on a built `Client` surfaces fewer of them than a reading of
//! `Config::fmt_fields` in the vendored source suggests — measured
//! directly (printing a real built client, not assumed from the source):
//! `redirect_policy`, the total timeout (keyed `reqwest::config::
//! TotalTimeout`) and `read_timeout` appear; `connect_timeout`,
//! `http1_only` and both TLS version bounds do not, regardless of value.
//! Those, plus `no_proxy`'s two internal flags and the three D5 Tier 2
//! toggles (which print only when `true`, and ours are `false`, identical
//! to never having called them), get a source-text test instead: the
//! dispatch's own instruction for the Tier 2 toggles ("testing hyper's
//! parsing behaviour is not required and is not what the control is")
//! extends to the others for the same reason — there is no runtime
//! distinction `Debug` exposes to observe.

use axum::http::StatusCode;
use sui_id::egress::build_federation_client;

fn debug_of(client: &reqwest::Client) -> String {
    format!("{client:?}")
}

#[test]
fn redirect_policy_is_none_not_default() {
    let d = debug_of(&build_federation_client());
    assert!(
        d.contains("redirect_policy"),
        "a non-default redirect policy must be visible in Debug: {d}"
    );
}

#[test]
fn referer_is_disabled_not_default() {
    let d = debug_of(&build_federation_client());
    // Default is `referer: true`, and reqwest's Debug impl prints the
    // field only when it is `true` -- so the *default* client's Debug
    // output would contain "referer" and ours, with it disabled, must not.
    let default_debug = format!("{:?}", reqwest::Client::new());
    assert!(
        default_debug.contains("referer"),
        "sanity: the default client's referer must be visible (true) -- \
         otherwise this test cannot tell the two apart: {default_debug}"
    );
    assert!(
        !d.contains("referer"),
        "referer(false) must not appear as enabled: {d}"
    );
}

#[test]
fn read_timeout_and_total_timeout_are_set() {
    let d = debug_of(&build_federation_client());
    assert!(
        d.contains("read_timeout"),
        "a read timeout must be set (default is none, invisible): {d}"
    );
    assert!(
        d.contains("TotalTimeout"),
        "a total timeout must be set (default is none, invisible); note \
         that a bare \"timeout\" substring check would pass even with this \
         setting removed, since \"read_timeout\" also contains it: {d}"
    );
}

/// `connect_timeout`, `http1_only`, both TLS version bounds, `no_proxy`,
/// and the three D5 Tier 2 toggles leave no trace in `Client`'s `Debug`
/// output -- measured directly against a real built client (see this
/// module's own doc comment), not assumed from reading `reqwest`'s
/// source. The only way to prove the constructor still calls them is to
/// read the constructor's own source.
#[test]
fn settings_invisible_to_debug_are_present_in_source() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/runtime/egress.rs"
    ))
    .expect("read egress.rs");
    for call in [
        ".connect_timeout(",
        ".http1_only()",
        ".tls_version_min(",
        ".tls_version_max(",
        ".no_proxy()",
        ".http1_allow_obsolete_multiline_headers_in_responses(false)",
        ".http1_ignore_invalid_headers_in_responses(false)",
        ".http1_allow_spaces_after_header_name_in_responses(false)",
    ] {
        assert!(
            source.contains(call),
            "build_federation_client() must call {call}; it has no observable \
             effect in Debug output (measured directly), so the source is the \
             only place this can be checked"
        );
    }
}

/// The behaviour that matters for the redirect refusal, not the config
/// value: a server returning a 3xx must produce an error from the egress
/// client, and critically, **no second request is made** -- proving the
/// policy is actually enforced, not merely configured.
#[tokio::test]
async fn a_redirect_is_refused_and_no_second_request_is_made() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let second_hits = Arc::new(AtomicUsize::new(0));
    let hits_for_handler = second_hits.clone();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let app = axum::Router::new()
        .route(
            "/start",
            axum::routing::get(move || {
                let target = format!("http://{addr}/second");
                async move { (StatusCode::FOUND, [(axum::http::header::LOCATION, target)]) }
            }),
        )
        .route(
            "/second",
            axum::routing::get(move || {
                let hits = hits_for_handler.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    StatusCode::OK
                }
            }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock upstream");
    });

    let client = build_federation_client();
    let result = client.get(format!("http://{addr}/start")).send().await;

    assert!(
        result.is_ok(),
        "the redirect response itself should be returned, not an error \
         constructing the request"
    );
    let resp = result.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::FOUND,
        "the 3xx is returned to the caller unfollowed, not an error -- \
         reqwest's Policy::none() surfaces the redirect response itself"
    );
    assert_eq!(
        second_hits.load(Ordering::SeqCst),
        0,
        "the redirect target must never have been requested"
    );
}

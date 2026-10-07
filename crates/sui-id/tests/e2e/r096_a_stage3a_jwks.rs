//! RFC 096-A stage 3a: the JWKS fetch on the production federation client, against
//! the same hostile fixture as the stage-1 harness. Each bound is driven over real
//! TLS by bytes the test chose, not by a unit-level constructed value.
//!
//! The client is [`federation_test_client`]: the production constructor, with two
//! named differences (its resolver, and one extra trusted root). Verification stays
//! on.

use std::time::{Duration, Instant};

use super::tls_mock::{AfterWrite, FixtureResolver, federation_test_client, serve_raw_https};
use sui_id::jwks::{JwksError, fetch_jwks};
use sui_id::response_bounds::BoundsError;

fn response(status: &str, content_type: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn honest(body: &str) -> Vec<u8> {
    response("200 OK", "application/json", body)
}

/// A JWKS whose `x` member nests `n` arrays deep.
fn nested_jwks(n: usize) -> String {
    format!(
        r#"{{"keys":[{{"kty":"RSA","kid":"a","x":{}{}}}]}}"#,
        "[".repeat(n),
        "]".repeat(n)
    )
}

#[tokio::test]
async fn an_honest_jwks_over_tls_is_fetched_and_returned() {
    let body = r#"{"keys":[{"kty":"RSA","kid":"a","n":"x","e":"AQAB"}]}"#;
    let (base, addr) = serve_raw_https(|_| honest(body), AfterWrite::Close).await;
    let client = federation_test_client(FixtureResolver { addr });

    let jwks = fetch_jwks(&client, &format!("{base}/jwks"))
        .await
        .expect("an honest JWKS within every bound is accepted");
    assert_eq!(jwks.keys.len(), 1);
    assert_eq!(jwks.keys[0]["kid"], "a");
}

#[tokio::test]
async fn a_non_200_status_is_refused_by_the_transport_bounds() {
    let (base, addr) = serve_raw_https(
        |_| response("404 Not Found", "application/json", "{}"),
        AfterWrite::Close,
    )
    .await;
    let client = federation_test_client(FixtureResolver { addr });
    assert!(matches!(
        fetch_jwks(&client, &format!("{base}/jwks")).await,
        Err(JwksError::Bounds(BoundsError::WrongStatus(_)))
    ));
}

#[tokio::test]
async fn a_non_json_media_type_is_refused_by_the_transport_bounds() {
    let (base, addr) = serve_raw_https(
        |_| response("200 OK", "text/html", r#"{"keys":[]}"#),
        AfterWrite::Close,
    )
    .await;
    let client = federation_test_client(FixtureResolver { addr });
    assert!(matches!(
        fetch_jwks(&client, &format!("{base}/jwks")).await,
        Err(JwksError::Bounds(BoundsError::WrongContentType(_)))
    ));
}

/// A `Content-Length` that claims ten mebibytes, a body of 100 KiB, and the
/// connection held open. The cap must end the read on bytes actually received,
/// well inside the client's own timeout.
#[tokio::test]
async fn a_lying_content_length_is_capped_on_bytes_read() {
    let mut bytes = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        10 * 1024 * 1024
    )
    .into_bytes();
    bytes.extend(std::iter::repeat_n(b' ', 100 * 1024));
    let (base, addr) = serve_raw_https(move |_| bytes, AfterWrite::HoldOpen).await;
    let client = federation_test_client(FixtureResolver { addr });

    let started = Instant::now();
    let result = fetch_jwks(&client, &format!("{base}/jwks")).await;
    let elapsed = started.elapsed();

    assert!(
        matches!(result, Err(JwksError::Bounds(BoundsError::TooLarge { .. }))),
        "{result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "the cap must end the read: {elapsed:?}"
    );
}

#[tokio::test]
async fn two_keys_sharing_a_kid_are_refused_over_the_wire() {
    let body = r#"{"keys":[{"kid":"a"},{"kid":"a"}]}"#;
    let (base, addr) = serve_raw_https(|_| honest(body), AfterWrite::Close).await;
    let client = federation_test_client(FixtureResolver { addr });
    assert!(matches!(
        fetch_jwks(&client, &format!("{base}/jwks")).await,
        Err(JwksError::DuplicateKid(kid)) if kid == "a"
    ));
}

#[tokio::test]
async fn nesting_past_sixteen_levels_is_refused_over_the_wire() {
    let body = nested_jwks(14);
    let (base, addr) = serve_raw_https(move |_| honest(&body), AfterWrite::Close).await;
    let client = federation_test_client(FixtureResolver { addr });
    assert!(matches!(
        fetch_jwks(&client, &format!("{base}/jwks")).await,
        Err(JwksError::TooDeep { .. })
    ));
}

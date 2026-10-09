use super::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
struct Doc {
    a: String,
    b: Option<String>,
}

fn deep_array_json(depth: usize) -> String {
    let mut s = String::new();
    for _ in 0..depth {
        s.push('[');
    }
    for _ in 0..depth {
        s.push(']');
    }
    s
}

async fn serve_once(status_line: &str, extra_headers: &str, body: &str) -> reqwest::Response {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let status_line = status_line.to_owned();
    let extra_headers = extra_headers.to_owned();
    let body = body.to_owned();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut discard = [0u8; 1024];
        let _ = stream.read(&mut discard).await;
        let framed = format!(
            "{status_line}\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        let _ = stream.write_all(framed.as_bytes()).await;
        let _ = stream.shutdown().await;
    });
    reqwest::Client::new()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .expect("connect")
}

#[tokio::test]
async fn a_non_json_content_type_is_rejected() {
    let resp = serve_once(
        "HTTP/1.1 200 OK",
        "Content-Type: text/html\r\n",
        "<html></html>",
    )
    .await;
    let result: Result<serde_json::Value, BoundsError> = read_bounded_json(resp).await;
    assert!(matches!(result, Err(BoundsError::WrongContentType(_))));
}

#[tokio::test]
async fn a_json_plus_suffix_content_type_is_accepted() {
    let resp = serve_once(
        "HTTP/1.1 200 OK",
        "Content-Type: application/trust+json\r\n",
        r#"{"a":"x"}"#,
    )
    .await;
    let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn a_missing_content_type_is_rejected() {
    let resp = serve_once("HTTP/1.1 200 OK", "", r#"{"a":"x"}"#).await;
    let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
    assert!(matches!(result, Err(BoundsError::MissingContentType)));
}

#[tokio::test]
async fn a_non_200_status_is_rejected() {
    let resp = serve_once(
        "HTTP/1.1 201 Created",
        "Content-Type: application/json\r\n",
        r#"{"a":"x"}"#,
    )
    .await;
    let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
    assert!(matches!(result, Err(BoundsError::WrongStatus(_))));
}

#[test]
fn depth_is_bounded_by_serde_json_itself() {
    // Pinning the library's own guarantee (RFC 134 D5 Tier 1's "pin
    // the floor, record what is relied on" for the two caps that
    // already hold): a 200-deep array -- past serde_json's default
    // 128 -- is rejected.
    let ok: Result<serde_json::Value, _> = serde_json::from_str(&deep_array_json(100));
    assert!(ok.is_ok(), "100 is within the default depth limit");
    let err: Result<serde_json::Value, _> = serde_json::from_str(&deep_array_json(200));
    assert!(err.is_err(), "200 exceeds the default depth limit");
}

#[test]
fn duplicate_declared_key_is_rejected_by_serde_itself() {
    let dup = r#"{"a":"x","b":"y","a":"z"}"#;
    let result: Result<Doc, _> = serde_json::from_str(dup);
    assert!(
        result.is_err(),
        "a duplicate declared field must be rejected"
    );
}

#[test]
fn duplicate_unknown_key_is_not_caught_deliberately_not_deny_unknown_fields() {
    // The one limit RFC 134 D5 Tier 1 states rather than discovers
    // later: duplicate *unknown* keys are not caught, because serde
    // skips unknown fields without duplicate detection.
    // `deny_unknown_fields` is deliberately not added -- it would
    // reject real providers' legitimate extra members.
    let dup_unknown = r#"{"a":"x","unknown":"first","unknown":"second"}"#;
    let result: Result<Doc, _> = serde_json::from_str(dup_unknown);
    assert!(
        result.is_ok(),
        "an unknown field appearing twice is not a rejection -- Doc has no \
             deny_unknown_fields, and this must stay true"
    );
}

#[test]
fn too_many_members_is_rejected() {
    let mut obj = serde_json::Map::new();
    for i in 0..(MAX_MEMBERS + 1) {
        obj.insert(format!("k{i}"), serde_json::Value::Bool(true));
    }
    let err = check_caps(
        &serde_json::Value::Object(obj),
        &ResponseCaps::JWKS_AND_TRANSPORT,
    )
    .expect_err("must reject");
    assert!(matches!(err, BoundsError::TooManyMembers { .. }));
}

#[test]
fn exactly_the_member_limit_is_accepted() {
    let mut obj = serde_json::Map::new();
    for i in 0..MAX_MEMBERS {
        obj.insert(format!("k{i}"), serde_json::Value::Bool(true));
    }
    assert!(
        check_caps(
            &serde_json::Value::Object(obj),
            &ResponseCaps::JWKS_AND_TRANSPORT
        )
        .is_ok()
    );
}

#[test]
fn too_long_an_array_is_rejected() {
    let arr = vec![serde_json::Value::Bool(true); MAX_ARRAY_LEN + 1];
    let err = check_caps(
        &serde_json::Value::Array(arr),
        &ResponseCaps::JWKS_AND_TRANSPORT,
    )
    .expect_err("must reject");
    assert!(matches!(err, BoundsError::ArrayTooLong { .. }));
}

#[test]
fn exactly_the_array_limit_is_accepted() {
    let arr = vec![serde_json::Value::Bool(true); MAX_ARRAY_LEN];
    assert!(
        check_caps(
            &serde_json::Value::Array(arr),
            &ResponseCaps::JWKS_AND_TRANSPORT
        )
        .is_ok()
    );
}

#[test]
fn too_long_a_string_is_rejected() {
    let s = "x".repeat(MAX_STRING_LEN + 1);
    let err = check_caps(
        &serde_json::Value::String(s),
        &ResponseCaps::JWKS_AND_TRANSPORT,
    )
    .expect_err("must reject");
    assert!(matches!(err, BoundsError::StringTooLong { .. }));
}

#[test]
fn exactly_the_string_limit_is_accepted() {
    let s = "x".repeat(MAX_STRING_LEN);
    assert!(
        check_caps(
            &serde_json::Value::String(s),
            &ResponseCaps::JWKS_AND_TRANSPORT
        )
        .is_ok()
    );
}

#[tokio::test]
async fn a_body_with_no_content_length_is_still_capped_on_bytes_read() {
    // RFC 134 D5 Tier 1 S4b: "A Content-Length pre-check is necessary
    // but not sufficient -- it is attacker-supplied and may be
    // absent or lie." This is the absent case: chunked
    // transfer-encoding, no Content-Length header at all, streaming
    // past the cap. A pre-check alone would have nothing to check
    // against and would have to trust the server; the cap here
    // holds on bytes actually received instead.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut discard = [0u8; 1024];
        let _ = stream.read(&mut discard).await;
        let total = MAX_RESPONSE_BYTES + 1000;
        let chunk_body = "x".repeat(total);
        let framed = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{chunk_body}\r\n0\r\n\r\n",
            chunk_body.len(),
        );
        let _ = stream.write_all(framed.as_bytes()).await;
        let _ = stream.shutdown().await;
    });

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/"))
        .send()
        .await
        .expect("connect");
    assert!(
        resp.headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .is_none(),
        "the server must not have declared a length -- this is the absent case, not the lying one"
    );
    let result: Result<serde_json::Value, BoundsError> = read_bounded_json(resp).await;
    assert!(
        matches!(result, Err(BoundsError::TooLarge { .. })),
        "{result:?}"
    );
}

#[test]
fn a_long_string_nested_inside_an_object_is_still_caught() {
    // The caps apply at every depth, not only the top level.
    let mut obj = serde_json::Map::new();
    obj.insert(
        "nested".into(),
        serde_json::Value::Array(vec![serde_json::Value::String(
            "x".repeat(MAX_STRING_LEN + 1),
        )]),
    );
    let err = check_caps(
        &serde_json::Value::Object(obj),
        &ResponseCaps::JWKS_AND_TRANSPORT,
    )
    .expect_err("must reject");
    assert!(matches!(err, BoundsError::StringTooLong { .. }));
}

// ---- RFC 096-A stage 8 (fix): pinning `read_bounded_json` to the shared
// transport caps, not discovery's -- the live token-response and userinfo
// paths carry an `id_token` string that RFC 096's own claim matrix permits
// up to 1,024/512 bytes per optional claim, comfortably over discovery's
// tighter 2,048-byte cap once a real signed token is built. These two
// tests fail if `read_bounded_json`'s internal `check_caps` call is ever
// pointed at `ResponseCaps::DISCOVERY` instead of `JWKS_AND_TRANSPORT`.

#[tokio::test]
async fn read_bounded_json_accepts_a_string_over_discoverys_bound_but_under_the_shared_one() {
    let s = "x".repeat(2_049);
    assert!(s.len() > 2_048 && s.len() < MAX_STRING_LEN);
    let body = serde_json::json!({ "a": s }).to_string();
    let resp = serve_once(
        "HTTP/1.1 200 OK",
        "Content-Type: application/json\r\n",
        &body,
    )
    .await;
    let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn read_bounded_json_accepts_an_array_over_discoverys_bound_but_under_the_shared_one() {
    let extra = vec![serde_json::Value::Bool(true); 33];
    assert!(extra.len() > 32 && extra.len() < MAX_ARRAY_LEN);
    let body = serde_json::json!({ "a": "x", "extra": extra }).to_string();
    let resp = serve_once(
        "HTTP/1.1 200 OK",
        "Content-Type: application/json\r\n",
        &body,
    )
    .await;
    let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
    assert!(result.is_ok(), "{result:?}");
}

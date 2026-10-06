use super::decode_id_token_claims;
use base64ct::{Base64UrlUnpadded, Encoding};

fn jwt_with_payload(payload: &[u8]) -> String {
    let header = Base64UrlUnpadded::encode_string(br#"{"alg":"none"}"#);
    let payload = Base64UrlUnpadded::encode_string(payload);
    format!("{header}.{payload}.")
}

#[test]
fn decode_id_token_claims_accepts_unpadded_jwt_payload() {
    let jwt = jwt_with_payload(
        br#"{"sub":"upstream-123","email":"alice@example.com","email_verified":true,"nonce":"n"}"#,
    );

    let claims = decode_id_token_claims(&jwt).expect("claims should decode");

    assert_eq!(claims.sub, "upstream-123");
    assert_eq!(claims.email.as_deref(), Some("alice@example.com"));
    assert!(claims.email_verified);
    assert_eq!(claims.nonce.as_deref(), Some("n"));
}

#[test]
fn decode_id_token_claims_rejects_malformed_payload() {
    assert!(decode_id_token_claims("header.not-base64url.signature").is_none());
}

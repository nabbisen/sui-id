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

// ---- RFC 096-A stage 2: compact-JWS structure and header hygiene -----------
//
// Structural only. Nothing here checks a signature, an algorithm allowlist or
// a claim: those are stage 4's, and `decode_id_token_claims` is untouched.

use super::{
    CompactJws, CompactJwsError, MAX_DECODED_HEADER_AND_PAYLOAD, Segment, parse_compact_jws,
};

const GOOD: &str = r#"{"alg":"RS256","kid":"k1","typ":"JWT"}"#;

fn enc(bytes: &[u8]) -> String {
    Base64UrlUnpadded::encode_string(bytes)
}

fn token(header: &[u8], payload: &[u8]) -> String {
    format!("{}.{}.{}", enc(header), enc(payload), enc(b"sig"))
}

fn token_with_header(header: &str) -> String {
    token(header.as_bytes(), b"{}")
}

fn accepted(input: &str) -> CompactJws {
    parse_compact_jws(input).expect("must be accepted")
}

fn refused(input: &str) -> CompactJwsError {
    parse_compact_jws(input).expect_err("must be refused")
}

// ---- compact serialization (table A) ---------------------------------------

#[test]
fn accepts_a_well_formed_token_with_kid_and_typ_jwt() {
    let payload = br#"{"sub":"upstream-1"}"#;
    let jws = accepted(&token(GOOD.as_bytes(), payload));
    assert_eq!(jws.header.alg, "RS256");
    assert_eq!(jws.header.kid, "k1");
    assert_eq!(jws.header.typ.as_deref(), Some("JWT"));
    assert_eq!(
        jws.signing_input,
        format!("{}.{}", enc(GOOD.as_bytes()), enc(payload))
    );
    assert_eq!(jws.payload, payload);
    assert_eq!(jws.signature, b"sig");
}

#[test]
fn json_serialization_is_refused_before_the_segments_are_counted() {
    // Has two dots, so a segment-first check would call it WrongSegmentCount.
    assert_eq!(
        refused(r#"{"protected":"a.b.c","payload":"x","signature":"y"}"#),
        CompactJwsError::JsonSerialization
    );
}

#[test]
fn two_segments_are_refused() {
    assert_eq!(
        refused("a.b"),
        CompactJwsError::WrongSegmentCount { found: 2 }
    );
}

#[test]
fn four_segments_are_refused() {
    assert_eq!(
        refused("a.b.c.d"),
        CompactJwsError::WrongSegmentCount { found: 4 }
    );
}

/// A compact JWE has five segments. The three-segment rule refuses it; this test
/// says so rather than leaving it implied.
#[test]
fn a_five_segment_compact_jwe_is_refused() {
    assert_eq!(
        refused("a.b.c.d.e"),
        CompactJwsError::WrongSegmentCount { found: 5 }
    );
}

#[test]
fn a_detached_payload_is_refused_as_an_empty_middle_segment() {
    let input = format!("{}..{}", enc(GOOD.as_bytes()), enc(b"sig"));
    assert_eq!(
        refused(&input),
        CompactJwsError::EmptySegment(Segment::Payload)
    );
}

#[test]
fn an_empty_header_is_refused() {
    let input = format!(".{}.{}", enc(b"p"), enc(b"sig"));
    assert_eq!(
        refused(&input),
        CompactJwsError::EmptySegment(Segment::Header)
    );
}

#[test]
fn an_empty_signature_is_refused() {
    let input = format!("{}.{}.", enc(GOOD.as_bytes()), enc(b"p"));
    assert_eq!(
        refused(&input),
        CompactJwsError::EmptySegment(Segment::Signature)
    );
}

#[test]
fn padding_in_the_header_is_refused() {
    let input = format!("{}==.{}.{}", enc(GOOD.as_bytes()), enc(b"p"), enc(b"sig"));
    assert_eq!(
        refused(&input),
        CompactJwsError::NotBase64Url(Segment::Header)
    );
}

#[test]
fn padding_in_the_payload_is_refused() {
    let input = format!("{}.{}=.{}", enc(GOOD.as_bytes()), enc(b"p"), enc(b"sig"));
    assert_eq!(
        refused(&input),
        CompactJwsError::NotBase64Url(Segment::Payload)
    );
}

#[test]
fn standard_base64_characters_are_refused_in_the_signature() {
    let input = format!("{}.{}.+/+/", enc(GOOD.as_bytes()), enc(b"p"));
    assert_eq!(
        refused(&input),
        CompactJwsError::NotBase64Url(Segment::Signature)
    );
}

/// The encoded length is checked before any decoding. Twenty thousand bytes of
/// non-JSON is refused as oversized, not as a bad header: size comes first.
#[test]
fn an_oversized_encoded_header_is_refused_before_decoding_or_json() {
    let header = vec![b'a'; 20_000];
    assert_eq!(refused(&token(&header, b"{}")), CompactJwsError::Oversized);
}

/// The decoded header and payload together exceed the limit, but each segment's
/// text is within the encoded bound. Refused as oversized, not as "not JSON":
/// the size check runs before the JSON parser sees the header.
#[test]
fn the_decoded_size_is_checked_before_any_json_parsing() {
    let header = vec![b'x'; 12 * 1024];
    let payload = vec![b'x'; 6 * 1024];
    assert_eq!(
        refused(&token(&header, &payload)),
        CompactJwsError::Oversized
    );
}

#[test]
fn exactly_the_decoded_limit_is_accepted() {
    let payload = vec![b' '; MAX_DECODED_HEADER_AND_PAYLOAD - GOOD.len()];
    accepted(&token(GOOD.as_bytes(), &payload));
}

#[test]
fn one_byte_over_the_decoded_limit_is_refused() {
    let payload = vec![b' '; MAX_DECODED_HEADER_AND_PAYLOAD - GOOD.len() + 1];
    assert_eq!(
        refused(&token(GOOD.as_bytes(), &payload)),
        CompactJwsError::Oversized
    );
}

// ---- protected header (table B) --------------------------------------------

#[test]
fn a_header_that_is_not_json_is_refused() {
    assert_eq!(
        refused(&token(b"not json", b"{}")),
        CompactJwsError::HeaderNotJson
    );
}

#[test]
fn a_header_that_is_json_but_not_an_object_is_refused() {
    assert_eq!(
        refused(&token(b"[1,2]", b"{}")),
        CompactJwsError::HeaderNotObject
    );
}

#[test]
fn a_repeated_declared_member_is_refused() {
    assert_eq!(
        refused(&token_with_header(
            r#"{"alg":"RS256","alg":"none","kid":"k1"}"#
        )),
        CompactJwsError::DuplicateMember("alg".into())
    );
}

/// `serde_json` will not catch a repeated unknown member. This does.
#[test]
fn a_repeated_unknown_member_is_refused() {
    assert_eq!(
        refused(&token_with_header(
            r#"{"alg":"RS256","x":1,"x":2,"kid":"k1"}"#
        )),
        CompactJwsError::DuplicateMember("x".into())
    );
}

#[test]
fn a_member_repeated_non_adjacently_is_refused() {
    assert_eq!(
        refused(&token_with_header(
            r#"{"alg":"RS256","x":1,"kid":"k1","x":2}"#
        )),
        CompactJwsError::DuplicateMember("x".into())
    );
}

#[test]
fn a_missing_alg_is_refused() {
    assert_eq!(
        refused(&token_with_header(r#"{"kid":"k1"}"#)),
        CompactJwsError::AlgMissing
    );
}

#[test]
fn a_non_string_alg_is_refused() {
    assert_eq!(
        refused(&token_with_header(r#"{"alg":5,"kid":"k1"}"#)),
        CompactJwsError::AlgNotString
    );
}

/// The `alg` value is not checked here. `none` and `HS256` pass this stage; the
/// allowlist that refuses them is stage 4's, and config's before that.
#[test]
fn the_alg_value_is_not_checked_at_this_stage() {
    accepted(&token_with_header(r#"{"alg":"none","kid":"k1"}"#));
    accepted(&token_with_header(r#"{"alg":"HS256","kid":"k1"}"#));
}

#[test]
fn each_forbidden_member_is_refused_by_its_own_name() {
    let cases = [
        ("jku", r#""https://keys.example/jwks""#),
        ("x5u", r#""https://certs.example/c.pem""#),
        ("jwk", r#"{"kty":"RSA"}"#),
        ("x5c", r#"["MIIB"]"#),
        ("crit", r#"["exp"]"#),
        ("b64", "false"),
    ];
    for (name, value) in cases {
        let header = format!(r#"{{"alg":"RS256","kid":"k1","{name}":{value}}}"#);
        assert_eq!(
            refused(&token_with_header(&header)),
            CompactJwsError::ForbiddenMember(name),
            "{name} must be refused under its own name"
        );
    }
}

#[test]
fn cty_is_refused_whatever_its_value() {
    assert_eq!(
        refused(&token_with_header(
            r#"{"alg":"RS256","kid":"k1","cty":"JWT"}"#
        )),
        CompactJwsError::CtyPresent
    );
}

#[test]
fn typ_must_be_exactly_jwt_and_case_sensitive() {
    assert_eq!(
        refused(&token_with_header(
            r#"{"alg":"RS256","kid":"k1","typ":"jwt"}"#
        )),
        CompactJwsError::TypNotJwt
    );
    assert_eq!(
        refused(&token_with_header(r#"{"alg":"RS256","kid":"k1","typ":7}"#)),
        CompactJwsError::TypNotJwt
    );
}

#[test]
fn typ_absent_is_accepted() {
    let jws = accepted(&token_with_header(r#"{"alg":"RS256","kid":"k1"}"#));
    assert_eq!(jws.header.typ, None);
}

#[test]
fn a_missing_kid_is_refused() {
    assert_eq!(
        refused(&token_with_header(r#"{"alg":"RS256"}"#)),
        CompactJwsError::KidMissing
    );
}

fn kid_header(kid: &str) -> String {
    format!(r#"{{"alg":"RS256","kid":"{kid}"}}"#)
}

#[test]
fn kid_length_bounds_are_one_to_one_hundred_and_twenty_eight_bytes() {
    accepted(&token_with_header(&kid_header("a")));
    accepted(&token_with_header(&kid_header(&"a".repeat(128))));
    assert_eq!(
        refused(&token_with_header(&kid_header(""))),
        CompactJwsError::KidInvalid
    );
    assert_eq!(
        refused(&token_with_header(&kid_header(&"a".repeat(129)))),
        CompactJwsError::KidInvalid
    );
}

#[test]
fn kid_accepts_the_edges_of_visible_ascii() {
    accepted(&token_with_header(&kid_header("!~")));
}

#[test]
fn kid_with_a_space_or_delete_or_non_ascii_is_refused() {
    assert_eq!(
        refused(&token_with_header(&kid_header("a b"))),
        CompactJwsError::KidInvalid
    );
    assert_eq!(
        refused(&token_with_header(&kid_header("a\u{7f}b"))),
        CompactJwsError::KidInvalid
    );
    assert_eq!(
        refused(&token_with_header(&kid_header("é"))),
        CompactJwsError::KidInvalid
    );
}

#[test]
fn a_non_string_kid_is_refused() {
    assert_eq!(
        refused(&token_with_header(r#"{"alg":"RS256","kid":12}"#)),
        CompactJwsError::KidInvalid
    );
}

#[test]
fn padding_in_the_signature_is_refused() {
    let input = format!("{}.{}.{}=", enc(GOOD.as_bytes()), enc(b"p"), enc(b"sig"));
    assert_eq!(
        refused(&input),
        CompactJwsError::NotBase64Url(Segment::Signature)
    );
}

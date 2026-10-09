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
    CompactJws, CompactJwsError, MAX_DECODED_HEADER_AND_PAYLOAD, MAX_ENCODED_JWS_LEN, Segment,
    VerificationError, parse_compact_jws, verify_id_token, verify_id_token_against_jwks,
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

/// The first check: the whole input, on its length, before the `{` test. Thirty
/// thousand `{` is also a JSON serialization, and is refused as oversized.
#[test]
fn a_token_over_the_total_bound_is_refused_before_anything_else() {
    let input = "{".repeat(MAX_ENCODED_JWS_LEN + 1);
    assert_eq!(refused(&input), CompactJwsError::Oversized);
}

/// A token whose total encoded length is exactly `MAX_ENCODED_JWS_LEN`: the
/// largest the RFC's 16 KiB decoded header plus payload can produce, plus a
/// signature sized to fill the rest. Accepted. One more character is refused.
#[test]
fn the_total_bound_is_exact_and_accepts_the_rfc_decoded_limit() {
    let mut header_len = GOOD.len();
    let (at_limit, signature_len) = loop {
        let mut header = GOOD.to_string();
        header.push_str(&" ".repeat(header_len - GOOD.len()));
        let payload = vec![b' '; MAX_DECODED_HEADER_AND_PAYLOAD - header.len()];
        let hb = enc(header.as_bytes());
        let pb = enc(&payload);
        let rest = MAX_ENCODED_JWS_LEN - hb.len() - pb.len() - 2;
        if rest % 4 != 1 && rest >= 2 {
            let sig = "A".repeat(rest);
            break (format!("{hb}.{pb}.{sig}"), rest);
        }
        header_len += 1;
    };
    assert_eq!(at_limit.len(), MAX_ENCODED_JWS_LEN);
    assert!(
        signature_len > 684,
        "the signature allowance must exceed an RSA-4096 signature"
    );
    accepted(&at_limit);
    assert_eq!(refused(&format!("{at_limit}A")), CompactJwsError::Oversized);
}

// ---- RFC 096-A stage 4a: signature verification ----------------------------
//
// `verify_id_token_against_jwks` joins stage 2's structural parse and stage
// 3b's key selection into a real signature check, with `jsonwebtoken` doing
// the cryptographic work. The sealing of `VerifiedIdTokenClaims` (RFC 096
// :648) is proven by a compile-fail fixture, not by a test here -- a test
// alone cannot establish that a type is not constructible; see
// `tests/compile_fail/verified_id_token_claims_cannot_be_constructed_directly.rs`.
//
// Key material: real RSA-2048, EC P-256 and Ed25519 keys, generated once with
// `openssl` and embedded as DER literals, because `jsonwebtoken::decode` does
// its own cryptographic check -- a hand-built or structurally-plausible
// signature would not exercise it.

use crate::jwks::{Jwks, KeySelectionError};
use jsonwebtoken::{Algorithm as JwtAlgorithm, EncodingKey, Header as JwtHeader, encode};

/// PKCS#1 DER. `aws-lc-rs`'s `RsaKeyPair::from_der` wants this, not the
/// PKCS#8 that `openssl rsa -outform DER` emits by default in this OpenSSL
/// version -- confirmed by reading `jsonwebtoken`'s `crypto/aws_lc/rsa.rs`,
/// which calls `RsaKeyPair::from_der` directly, and by `openssl rsa
/// -traditional -outform DER`, which forces the older PKCS#1 form.
const RSA_DER: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";

/// A different RSA key, same JWK shape, for the "signed by a key in the set
/// but under the wrong kid" case: this key's public JWK, not `RSA_N`/`RSA_E`'s.
const RSA2_N: &str = "r1sA8H6OY5dOF4BqP2hE9DehYvf9d4lxhE3KODkk96Tlg-YQvIAnALeKgsjYjAEl0p-JaiCITm5QKu1UBviFD_12cFD-chG9XaAVYqpVyqniVGtHeQADvg1Lax-QV39uyAryDqPL1c5cYRYOtV8ovehJC_bJC_C_PZbUw-DPilLqp6tU8I0WazreHMqMiStCIh08ey2fm1Ac_CQKWKwwKMr30MKtqHPVa-cK4NyCuGs3vvahfjE86evWGqCwl4qTGCsSbs5X6I5wKHtWN6YZmg6s4e-5S6kWD2wyeefwbpovOuywKnOgtmWME9uPi_7MQanLDqrR8NAtk9iD2e8yOw";

/// PKCS#8 DER. `jsonwebtoken`'s `aws_lc_rs` backend uses
/// `EcdsaKeyPair::from_pkcs8` for EC, which this format matches directly.
const EC_DER: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgSH/6RF7od9bPppu0TDTUTFTAbZWcLXrlwYsk0fZWNIChRANCAAT0Ng0YT36e4gG+i7JSRyf/uLHDP2oY8xpCd4AYIrkSzSX9SQ6d3X0ZeElMUeU9L2/vxvpKpIHC/21D1fvDXydR";
const EC_X: &str = "9DYNGE9-nuIBvouyUkcn_7ixwz9qGPMaQneAGCK5Es0";
const EC_Y: &str = "Jf1JDp3dfRl4SUxR5T0vb-_G-kqkgcL_bUPV-8NfJ1E";

/// PKCS#8 DER, same reason as `EC_DER`: `Ed25519KeyPair::from_pkcs8`.
const ED_DER: &str = "MC4CAQAwBQYDK2VwBCIEIB6kTFrMoy5I8bzWvDj0pioYjECAsuYY7G9wMxfdte+8";
const ED_X: &str = "QHhsQNftLqLhlfW3-Ipr_F57dCfpR_VinEDkVeLILMs";

fn der(b64_std: &str) -> Vec<u8> {
    base64ct::Base64::decode_vec(b64_std).expect("valid standard base64 DER")
}

#[derive(serde::Serialize)]
struct FutureClaims {
    sub: String,
    // A real OIDC ID token always has one (it is REQUIRED): present on
    // purpose, to exercise `validate_aud = false` rather than a fixture
    // that happens not to need it.
    aud: String,
    // Far future: `validate_exp` defaults on, and a token that was already
    // expired would be refused for a reason this suite is not testing.
    exp: i64,
}

fn future_claims() -> FutureClaims {
    FutureClaims {
        sub: "upstream-user".into(),
        aud: "this-rp-client-id".into(),
        exp: 9_999_999_999,
    }
}

fn sign(algorithm: JwtAlgorithm, kid: &str, key: &EncodingKey) -> String {
    let mut header = JwtHeader::new(algorithm);
    header.kid = Some(kid.into());
    encode(&header, &future_claims(), key)
        .expect("signing with a freshly generated key must succeed")
}

fn rsa_jwk(kid: &str, n: &str) -> serde_json::Value {
    serde_json::json!({"kty": "RSA", "kid": kid, "n": n, "e": RSA_E})
}

fn ec_jwk(kid: &str) -> serde_json::Value {
    serde_json::json!({"kty": "EC", "kid": kid, "crv": "P-256", "x": EC_X, "y": EC_Y})
}

fn okp_jwk(kid: &str) -> serde_json::Value {
    serde_json::json!({"kty": "OKP", "kid": kid, "crv": "Ed25519", "x": ED_X})
}

fn jwks_of(keys: Vec<serde_json::Value>) -> Jwks {
    Jwks { keys }
}

// ---- the happy path, one per accepted algorithm ------------------------------

#[test]
fn rs256_verifies_end_to_end_against_a_matching_jwks() {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let token = sign(JwtAlgorithm::RS256, "k1", &key);
    let jwks = jwks_of(vec![rsa_jwk("k1", RSA_N)]);
    let claims = verify_id_token_against_jwks(&token, &["RS256".into()], &jwks)
        .expect("a correctly signed RS256 token must verify");
    assert_eq!(claims.sub(), Some("upstream-user"));
}

#[test]
fn ps256_verifies_end_to_end_against_a_matching_jwks() {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let token = sign(JwtAlgorithm::PS256, "k1", &key);
    let jwks = jwks_of(vec![rsa_jwk("k1", RSA_N)]);
    verify_id_token_against_jwks(&token, &["PS256".into()], &jwks)
        .expect("a correctly signed PS256 token must verify");
}

#[test]
fn es256_verifies_end_to_end_against_a_matching_jwks() {
    let key = EncodingKey::from_ec_der(&der(EC_DER));
    let token = sign(JwtAlgorithm::ES256, "k1", &key);
    let jwks = jwks_of(vec![ec_jwk("k1")]);
    verify_id_token_against_jwks(&token, &["ES256".into()], &jwks)
        .expect("a correctly signed ES256 token must verify");
}

#[test]
fn eddsa_verifies_end_to_end_against_a_matching_jwks() {
    let key = EncodingKey::from_ed_der(&der(ED_DER));
    let token = sign(JwtAlgorithm::EdDSA, "k1", &key);
    let jwks = jwks_of(vec![okp_jwk("k1")]);
    let claims = verify_id_token_against_jwks(&token, &["EdDSA".into()], &jwks)
        .expect("a correctly signed EdDSA token must verify");
    assert_eq!(claims.sub(), Some("upstream-user"));
}

// ---- the signature itself ----------------------------------------------------

#[test]
fn a_signature_altered_by_one_byte_is_refused() {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let mut token = sign(JwtAlgorithm::RS256, "k1", &key);
    // Flip one base64url character in the signature segment -- a byte-level
    // change, not a structural one, so stage 2's parse still accepts it.
    let last = token.pop().expect("token has a final character");
    let flipped = if last == 'A' { 'B' } else { 'A' };
    token.push(flipped);
    let jwks = jwks_of(vec![rsa_jwk("k1", RSA_N)]);
    assert!(matches!(
        verify_id_token_against_jwks(&token, &["RS256".into()], &jwks),
        Err(VerificationError::SignatureInvalid)
    ));
}

/// The signing key really is in the set -- just not under the `kid` the
/// header names. `select_key` finds *a* key of the right family under that
/// `kid`, so selection itself succeeds; `jsonwebtoken::decode`'s actual
/// cryptographic check is what catches that it is the wrong one. Proves the
/// `kid` lookup is not mistaken for proof of authenticity.
#[test]
fn a_key_present_under_the_wrong_kid_is_refused_by_the_signature_check_not_by_selection() {
    let signing_key = EncodingKey::from_rsa_der(&der(RSA_DER)); // RSA_N's key
    let token = sign(JwtAlgorithm::RS256, "wrong-slot", &signing_key);
    // The JWKS has a key under "wrong-slot", but it is RSA2_N's key, not the
    // one that actually signed the token.
    let jwks = jwks_of(vec![rsa_jwk("wrong-slot", RSA2_N)]);
    assert!(matches!(
        verify_id_token_against_jwks(&token, &["RS256".into()], &jwks),
        Err(VerificationError::SignatureInvalid)
    ));
}

#[test]
fn a_kid_absent_from_the_set_is_refused_at_selection() {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let token = sign(JwtAlgorithm::RS256, "k1", &key);
    let jwks = jwks_of(vec![rsa_jwk("other", RSA2_N)]);
    assert!(matches!(
        verify_id_token_against_jwks(&token, &["RS256".into()], &jwks),
        Err(VerificationError::KeySelection(
            KeySelectionError::KidNotFound
        ))
    ));
}

// ---- the outer gate: id_token_algs, before key selection ----------------------

/// `alg: "HS256"` with a configured set that excludes it. Refused as **not
/// permitted**, proven by passing an *empty* `Jwks`: if the alg check ran
/// after key selection, this would be `KidNotFound` instead, because nothing
/// in an empty set could ever match. Getting `AlgNotPermitted` here is what
/// proves the gate runs first.
#[test]
fn hs256_excluded_from_the_configured_set_is_refused_as_not_permitted() {
    let token = token_with_header(r#"{"alg":"HS256","kid":"k1"}"#);
    let jwks = jwks_of(vec![]);
    assert!(matches!(
        verify_id_token_against_jwks(&token, &["RS256".into()], &jwks),
        Err(VerificationError::AlgNotPermitted)
    ));
}

/// The same proof with a *real, matching* key present: if the gate did not
/// run first, `select_key` would happily find this RSA key for `RS256` and
/// `jsonwebtoken` would happily verify a correctly signed token. The
/// configured set excludes `RS256` anyway, and the refusal must still be
/// `AlgNotPermitted`, not a successful verification.
#[test]
fn a_disallowed_alg_is_refused_even_when_a_valid_matching_key_exists() {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let token = sign(JwtAlgorithm::RS256, "k1", &key);
    let jwks = jwks_of(vec![rsa_jwk("k1", RSA_N)]);
    assert!(matches!(
        verify_id_token_against_jwks(&token, &["ES256".into()], &jwks),
        Err(VerificationError::AlgNotPermitted)
    ));
}

#[test]
fn an_allowed_alg_among_several_configured_is_accepted() {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let token = sign(JwtAlgorithm::RS256, "k1", &key);
    let jwks = jwks_of(vec![rsa_jwk("k1", RSA_N)]);
    verify_id_token_against_jwks(&token, &["ES256".into(), "RS256".into()], &jwks)
        .expect("RS256 is in the configured set");
}

// ---- structural refusals pass through, wrapped, not re-described -------------

#[test]
fn a_structurally_invalid_token_is_refused_through_the_wrapped_stage_2_error() {
    assert!(matches!(
        verify_id_token_against_jwks("not-a-jws", &["RS256".into()], &jwks_of(vec![])),
        Err(VerificationError::Structure(
            CompactJwsError::WrongSegmentCount { found: 1 }
        ))
    ));
}

// ---- the provider-level refusal, which only the async wrapper can make --------

#[tokio::test]
async fn no_jwks_uri_is_refused_before_any_network_access() {
    // An unroutable address: if this refusal required a network attempt, the
    // call would hang or error from the attempt itself, not return cleanly.
    let client = reqwest::Client::new();
    let result = verify_id_token(&client, "irrelevant", &["RS256".into()], None).await;
    assert!(matches!(result, Err(VerificationError::NoJwksUri)));
}

// ---- RFC 096-A stage 6b: the time claims ------------------------------------
//
// `exp` and `nbf`'s window arithmetic is `jsonwebtoken`'s own
// (`validation_for`'s doc comment has the full reasoning, including the
// one-second boundary finding); what these tests exercise is everything
// stage 6b actually added: the canonical-shape pre-check
// (`VerificationError::MalformedTimeClaim`), the two renamed error
// variants (`Expired`, `NotYetValid`), and `nbf`'s own window now that it
// is switched on. `iat`, which has no library support at all, is tested in
// `time_claims/tests.rs`.

fn sign_raw_payload(payload_json: &str) -> String {
    let header_json = r#"{"alg":"RS256","kid":"k1"}"#;
    let encoded_header = Base64UrlUnpadded::encode_string(header_json.as_bytes());
    let encoded_payload = Base64UrlUnpadded::encode_string(payload_json.as_bytes());
    let message = format!("{encoded_header}.{encoded_payload}");
    let key = EncodingKey::from_rsa_der(&der(RSA_DER));
    let signature = jsonwebtoken::crypto::sign(message.as_bytes(), &key, JwtAlgorithm::RS256)
        .expect("signing with a freshly generated key must succeed");
    format!("{message}.{signature}")
}

fn verify_raw(payload_json: &str) -> Result<super::VerifiedIdTokenClaims, VerificationError> {
    let token = sign_raw_payload(payload_json);
    verify_id_token_against_jwks(
        &token,
        &["RS256".into()],
        &jwks_of(vec![rsa_jwk("k1", RSA_N)]),
    )
}

// ---- exp: the library's window, re-verified rather than cited --------------

/// RFC 096 `:660`'s own wording is the strict `now < exp + 60s`. The
/// library's actual accept condition, read from `validation.rs:291` and
/// confirmed here rather than assumed, is `now <= exp + 60s` -- one second
/// more permissive than the RFC's literal boundary. Both forms agree at
/// `now == exp + 61` (refused either way); they disagree only at the exact
/// second `now == exp + 60`, which this library accepts and the RFC's own
/// sentence, read literally, would not. Not patched here -- the dispatch is
/// explicit that `exp`'s window is not this stage's to change -- stated as
/// the finding the dispatch asked this stage to either confirm or correct.
#[test]
fn exp_boundary_now_equals_exp_plus_60_is_accepted_by_the_library_one_second_more_than_the_rfcs_own_wording()
 {
    // The accept case races the real clock: `now` is read here, then read
    // again inside `jsonwebtoken::decode` during `verify_raw`. A second
    // ticking over in that gap turns the exact boundary into one second
    // past it, which the library correctly rejects -- an environment
    // timing flake (caught by a gate run during this stage, not assumed),
    // not a defect in the boundary claim. Resampling `now` fresh on
    // failure, rather than widening the margin, keeps the assertion about
    // the real exact boundary while closing the race; five attempts makes
    // a false failure astronomically unlikely.
    let mut last_result = None;
    for _ in 0..5 {
        let now = jsonwebtoken::get_current_timestamp();
        let exp_at_60 = now - 60;
        let p60 = format!(r#"{{"exp":{exp_at_60}}}"#);
        let result = verify_raw(&p60);
        let accepted = result.is_ok();
        last_result = Some(result);
        if accepted {
            break;
        }
    }
    assert!(
        matches!(last_result, Some(Ok(_))),
        "the library accepts the exact boundary, one second past the RFC's strict <: {last_result:?}"
    );

    let now = jsonwebtoken::get_current_timestamp();
    let exp_at_61 = now - 61;
    let p61 = format!(r#"{{"exp":{exp_at_61}}}"#);
    assert!(matches!(verify_raw(&p61), Err(VerificationError::Expired)));
}

#[test]
fn an_expired_token_is_refused_as_expired_not_as_a_signature_failure() {
    let now = jsonwebtoken::get_current_timestamp();
    let long_expired = now - 10_000;
    let p = format!(r#"{{"exp":{long_expired}}}"#);
    assert!(matches!(verify_raw(&p), Err(VerificationError::Expired)));
}

// ---- nbf: switched on by stage 6b -------------------------------------------

#[test]
fn an_nbf_within_the_60_second_skew_is_accepted() {
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now + 10_000;
    let nbf = now - 10;
    let p = format!(r#"{{"exp":{exp},"nbf":{nbf}}}"#);
    assert!(verify_raw(&p).is_ok());
}

#[test]
fn an_nbf_in_the_future_beyond_the_skew_is_refused_as_not_yet_valid_not_as_a_signature_failure() {
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now + 10_000;
    let nbf = now + 10_000;
    let p = format!(r#"{{"exp":{exp},"nbf":{nbf}}}"#);
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::NotYetValid)
    ));
}

// ---- MalformedTimeClaim: the shape check jsonwebtoken does not make --------
//
// `jsonwebtoken`'s own `numeric_type` deserializer rounds a float into an
// accepted integer instead of refusing it; these prove the pre-check this
// stage added catches what the library, left alone, would silently accept.

#[test]
fn a_float_exp_is_refused_despite_the_librarys_own_leniency() {
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now as f64 + 10_000.5;
    let p = format!(r#"{{"exp":{exp}}}"#);
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

#[test]
fn a_whole_number_float_exp_is_also_refused() {
    // `serde_json` tracks the literal's own syntax: `1700000000.0` is
    // `is_f64`, not `is_u64`, even though its value is a whole number.
    // `{:.1}` is deliberate, not cosmetic: Rust's plain `{}` Display for a
    // whole-number `f64` prints no decimal point at all (`1700000000`, not
    // `1700000000.0`), which would silently turn this into the canonical-
    // integer case this test exists to distinguish from -- confirmed by
    // running it before trusting the format string.
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now as f64 + 10_000.0;
    let p = format!(r#"{{"exp":{exp:.1}}}"#);
    assert!(p.contains('.'), "the literal must actually be a float: {p}");
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

/// RFC 096-A stage 9: `validation-matrix.md:152-153`'s corpus requires
/// test claims to include "string/floating/exponential NumericDates" --
/// 6b covered string and floating, not the exponential *literal syntax*
/// itself (`1.7e9`, no decimal point, distinct from
/// `a_whole_number_float_exp_is_also_refused`'s `1700000000.0`). Confirmed
/// empirically first, not assumed: `serde_json::Value`'s own classifier
/// marks *any* exponential-syntax number `is_f64`/`as_u64() == None`
/// regardless of whether it has a fractional part, so this reaches
/// `numeric_date`'s identical `NotAnInteger` path as a plain float -- this
/// test closes a corpus-coverage gap, not a production-code one.
#[test]
fn an_exponential_literal_exp_is_refused() {
    let p = r#"{"exp":1.7e9}"#;
    assert!(matches!(
        verify_raw(p),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

#[test]
fn a_string_exp_is_refused_by_name_not_as_a_missing_claim() {
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now + 10_000;
    let p = format!(r#"{{"exp":"{exp}"}}"#);
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

#[test]
fn a_negative_exp_is_refused() {
    assert!(matches!(
        verify_raw(r#"{"exp":-5}"#),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

#[test]
fn an_exp_past_u64_is_refused() {
    let p = format!(r#"{{"exp":{}}}"#, u64::MAX);
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

/// Distinct from `an_exp_past_u64_is_refused`: `u64::MAX` is rejected
/// earlier, by `i64::try_from` failing, never reaching
/// `chrono::DateTime::from_timestamp`'s own range check at all. `i64::MAX`
/// is representable as `i64` (so `try_from` succeeds) but is still far
/// past `chrono`'s own representable range -- this is the only value in
/// the suite that actually exercises that second check. Confirmed by
/// mutation: disabling the `from_timestamp(..).is_none()` branch leaves
/// every other test in this file passing and only this one fails.
#[test]
fn an_exp_within_i64_but_past_chronos_range_is_refused() {
    let p = format!(r#"{{"exp":{}}}"#, i64::MAX);
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::MalformedTimeClaim("exp"))
    ));
}

#[test]
fn a_malformed_nbf_is_refused_by_its_own_name() {
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now + 10_000;
    let p = format!(r#"{{"exp":{exp},"nbf":"{now}"}}"#);
    assert!(matches!(
        verify_raw(&p),
        Err(VerificationError::MalformedTimeClaim("nbf"))
    ));
}

#[test]
fn an_absent_nbf_is_not_an_error() {
    let now = jsonwebtoken::get_current_timestamp();
    let exp = now + 10_000;
    let p = format!(r#"{{"exp":{exp}}}"#);
    assert!(verify_raw(&p).is_ok());
}

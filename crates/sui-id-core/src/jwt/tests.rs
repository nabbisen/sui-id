use super::*;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct TestClaims {
    sub: String,
    iat: u64,
}

#[test]
fn sign_then_verify_recovers_claims() {
    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let vk = sk.verifying_key();
    let claims = TestClaims {
        sub: "user-123".into(),
        iat: 1_700_000_000,
    };
    let token = sign("k1", &sk, &claims).expect("sign");
    let decoded: Decoded<TestClaims> =
        verify(&token, |kid| if kid == "k1" { Some(vk) } else { None }).expect("verify");
    assert_eq!(decoded.claims, claims);
    assert_eq!(decoded.kid, "k1");
}

#[test]
fn tampered_payload_fails_verification() {
    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let vk = sk.verifying_key();
    let token = sign(
        "k1",
        &sk,
        &TestClaims {
            sub: "u".into(),
            iat: 0,
        },
    )
    .expect("sign");
    let parts: Vec<&str> = token.split('.').collect();
    // Replace the payload with a different (well-formed b64url) value but
    // keep the original signature: the signature will no longer match.
    let new_payload = "eyJzdWIiOiJ4Iiwib3RoZXIiOjF9";
    let bad = format!("{}.{}.{}", parts[0], new_payload, parts[2]);
    let r: Result<Decoded<TestClaims>, _> = verify(&bad, |_| Some(vk));
    assert!(r.is_err());
}

#[test]
fn unknown_kid_is_rejected() {
    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let token = sign(
        "k1",
        &sk,
        &TestClaims {
            sub: "x".into(),
            iat: 0,
        },
    )
    .expect("sign");
    let r: Result<Decoded<TestClaims>, _> = verify(&token, |_| None);
    assert!(matches!(r, Err(CoreError::Jwt)));
}

#[test]
fn malformed_token_is_rejected() {
    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let vk = sk.verifying_key();
    let r: Result<Decoded<TestClaims>, _> = verify("not.a.jwt.at.all", |_| Some(vk));
    assert!(matches!(r, Err(CoreError::Jwt)));
}

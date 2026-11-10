//! RFC 143 stage 5 part B/F: exercises, through whatever OpenSSL the
//! binary was actually linked against, the three signature algorithms
//! passkey verification uses -- COSE ES256, RS256, EdDSA
//! (`webauthn_rs_core::interface::COSEKey::verify_signature`). No test
//! in the repository signs a real passkey assertion (no software
//! authenticator is a dev-dependency), so without this, the release
//! workflow's static build would ship with its OpenSSL signature-
//! verification path exercised nowhere. Built and run on every
//! release-workflow matrix target; its `assert!`s failing the process
//! is what fails the job on a wrong result.
#![allow(clippy::expect_used)]
use openssl::{ec, ecdsa, hash::MessageDigest, nid::Nid, pkey::PKey, rsa::Rsa, sign};

fn main() {
    println!("linked: {}", openssl::version::version());
    let msg = b"authenticatorData||sha256(clientDataJSON)";

    // ES256: P-256 ECDSA over SHA-256 (what COSEKey::verify_signature builds).
    let group = ec::EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("P-256 curve");
    let ec_key =
        PKey::from_ec_key(ec::EcKey::generate(&group).expect("EC keygen")).expect("EC PKey");
    let mut s = sign::Signer::new(MessageDigest::sha256(), &ec_key).expect("ES256 signer");
    let sig = s.sign_oneshot_to_vec(msg).expect("ES256 sign");
    let mut v = sign::Verifier::new(MessageDigest::sha256(), &ec_key).expect("ES256 verifier");
    assert!(
        v.verify_oneshot(&sig, msg).expect("ES256 verify"),
        "ES256 good signature must verify"
    );
    let mut bad = sig.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    let mut v = sign::Verifier::new(MessageDigest::sha256(), &ec_key).expect("ES256 verifier");
    assert!(
        !v.verify_oneshot(&bad, msg).unwrap_or(false),
        "ES256 flipped signature must fail"
    );
    let _ = ecdsa::EcdsaSig::from_der(&sig).expect("ES256 DER round-trip");
    println!("ES256: good verifies, flipped fails");

    // RS256: RSA PKCS#1 v1.5 over SHA-256.
    let rsa = PKey::from_rsa(Rsa::generate(2048).expect("RSA keygen")).expect("RSA PKey");
    let mut s = sign::Signer::new(MessageDigest::sha256(), &rsa).expect("RS256 signer");
    let sig = s.sign_oneshot_to_vec(msg).expect("RS256 sign");
    let mut v = sign::Verifier::new(MessageDigest::sha256(), &rsa).expect("RS256 verifier");
    assert!(
        v.verify_oneshot(&sig, msg).expect("RS256 verify"),
        "RS256 good signature must verify"
    );
    println!("RS256: good verifies");

    // EdDSA: Ed25519 (no digest).
    let ed = PKey::generate_ed25519().expect("Ed25519 keygen");
    let mut s = sign::Signer::new_without_digest(&ed).expect("EdDSA signer");
    let sig = s.sign_oneshot_to_vec(msg).expect("EdDSA sign");
    let mut v = sign::Verifier::new_without_digest(&ed).expect("EdDSA verifier");
    assert!(
        v.verify_oneshot(&sig, msg).expect("EdDSA verify"),
        "EdDSA good signature must verify"
    );
    println!("EdDSA: good verifies");
}

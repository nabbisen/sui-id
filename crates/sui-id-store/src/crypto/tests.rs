#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;

#[test]
fn round_trip_preserves_plaintext() {
    let key = MasterKey::generate();
    let pt = b"hello, sui-id";
    let aad = b"client_secret";
    let sealed = seal(&key, pt, aad).expect("seal");
    let opened = open(&key, &sealed, aad).expect("open");
    assert_eq!(opened, pt);
}

#[test]
fn ciphertexts_for_same_plaintext_differ_due_to_random_nonce() {
    let key = MasterKey::generate();
    let pt = b"sensitive";
    let a = seal(&key, pt, b"").expect("seal a");
    let b = seal(&key, pt, b"").expect("seal b");
    assert_ne!(a, b);
}

#[test]
fn aad_mismatch_fails_open() {
    let key = MasterKey::generate();
    let sealed = seal(&key, b"x", b"context-A").expect("seal");
    assert!(open(&key, &sealed, b"context-B").is_err());
}

#[test]
fn truncated_ciphertext_is_rejected() {
    let key = MasterKey::generate();
    let mut sealed = seal(&key, b"x", b"").expect("seal");
    sealed.truncate(sealed.len() - 1);
    assert!(open(&key, &sealed, b"").is_err());
}

#[test]
fn base64_round_trip_recovers_key_material() {
    let k1 = MasterKey::generate();
    let s = k1.to_base64();
    let k2 = MasterKey::from_base64(&s).expect("decode");
    let pt = b"abc";
    let sealed = seal(&k1, pt, b"").expect("seal");
    let opened = open(&k2, &sealed, b"").expect("open");
    assert_eq!(opened, pt);
}

#[test]
fn from_base64_rejects_wrong_length() {
    // "AAAA" decodes to 3 bytes.
    let r = MasterKey::from_base64("AAAA");
    assert!(matches!(r, Err(StoreError::InvalidMasterKeyLength(3))));
}

// ---------- property-based tests (v0.13.0) ----------
//
// The two core invariants of the seal/open pair:
//
//   1. open(seal(p, aad), aad) == p   for any plaintext and any AAD.
//   2. open(seal(p, aad1), aad2) fails when aad1 != aad2.
//
// The crypto we're using (XChaCha20-Poly1305) is constant-time and
// well-tested; we are not trying to find primitive bugs here. We are
// trying to catch any future refactor that changes our wrapper in a
// way that breaks these properties — e.g. a parameter-order swap, a
// truncation bug at some byte boundary, or an empty-input edge case.

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        // Default of 256 cases per property is fine; the seal/open
        // path is constant-time and a few thousand iterations across
        // all properties stays under a second.
        cases: 256,
        ..ProptestConfig::default()
    })]

    #[test]
    fn round_trip_for_arbitrary_plaintext_and_aad(
        plaintext in proptest::collection::vec(any::<u8>(), 0..2048),
        aad in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        let key = MasterKey::generate();
        let sealed = seal(&key, &plaintext, &aad).expect("seal");
        let opened = open(&key, &sealed, &aad).expect("open");
        prop_assert_eq!(opened, plaintext);
    }

    #[test]
    fn open_with_wrong_aad_fails(
        plaintext in proptest::collection::vec(any::<u8>(), 1..512),
        aad1 in proptest::collection::vec(any::<u8>(), 1..64),
        aad2 in proptest::collection::vec(any::<u8>(), 1..64),
    ) {
        // proptest doesn't have a direct "two distinct values" so
        // we filter cases where they collide — over the full input
        // space this is rare.
        prop_assume!(aad1 != aad2);
        let key = MasterKey::generate();
        let sealed = seal(&key, &plaintext, &aad1).expect("seal");
        prop_assert!(open(&key, &sealed, &aad2).is_err());
    }

    #[test]
    fn open_with_wrong_key_fails(
        plaintext in proptest::collection::vec(any::<u8>(), 1..512),
        aad in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let key1 = MasterKey::generate();
        let key2 = MasterKey::generate();
        let sealed = seal(&key1, &plaintext, &aad).expect("seal");
        prop_assert!(open(&key2, &sealed, &aad).is_err());
    }

    #[test]
    fn ciphertext_strictly_grows_by_nonce_plus_tag(
        plaintext in proptest::collection::vec(any::<u8>(), 0..1024),
    ) {
        // Each ciphertext is (24-byte nonce || ciphertext-of-pt-len ||
        // 16-byte Poly1305 tag). A regression in the framing length
        // would surface here.
        let key = MasterKey::generate();
        let sealed = seal(&key, &plaintext, b"aad").expect("seal");
        prop_assert_eq!(sealed.len(), plaintext.len() + 24 + 16);
    }
}

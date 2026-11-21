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

// ── RFC 100 §2: the master-key fingerprint ──────────────────────────────

#[test]
fn two_different_keys_give_two_different_fingerprints() {
    let a = fingerprint(&MasterKey::generate());
    let b = fingerprint(&MasterKey::generate());
    assert_ne!(a, b);
}

#[test]
fn the_same_key_gives_the_same_fingerprint_every_time() {
    let key = MasterKey::from_bytes([7u8; 32]);
    assert_eq!(fingerprint(&key), fingerprint(&key));
}

/// A vector computed independently (Python's `hmac`/`hashlib`, not this
/// crate), against the key bytes `0..=31` and this module's own domain
/// string: `hmac.new(bytes(range(32)), b"sui-id/master-key-fingerprint/v1",
/// hashlib.sha256).digest()[:8].hex()` → `2c0ce5111113ca77`.
#[test]
fn a_known_key_gives_a_known_fingerprint() {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = i as u8;
    }
    let key = MasterKey::from_bytes(bytes);
    assert_eq!(fingerprint(&key), "2c0ce5111113ca77");
}

#[test]
fn fingerprint_is_sixteen_lowercase_hex_characters() {
    let fp = fingerprint(&MasterKey::generate());
    assert_eq!(fp.len(), 16);
    assert!(
        fp.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

// ---------- RFC 100 stage 3: the rotation journal's tags ----------

/// Vectors computed independently (Python's `hmac`/`hashlib`, not this
/// crate), against the key bytes `0..=31`, this module's own domains, a
/// `0x00` separator, and the body `b'{"x":1}'`:
/// `hmac.new(bytes(range(32)), domain + b"\x00" + body, hashlib.sha256).hexdigest()`.
#[test]
fn known_vectors_for_both_journal_tags() {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = i as u8;
    }
    let key = MasterKey::from_bytes(bytes);
    let body = br#"{"x":1}"#;
    assert_eq!(
        journal_tag_old(&key, body),
        "386b8d9759c62b568bc19d27f3fbae03279c5dc18dfc7d659096dd7c9f469572"
    );
    assert_eq!(
        journal_tag_new(&key, body),
        "c3f35c11df295706d530804510afcda1cb7101078f6fb7b791e39d80f8e55d86"
    );
}

#[test]
fn verify_reports_old_or_new_depending_on_the_candidate_key() {
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();
    let body = b"the published body bytes";
    let tag_old = journal_tag_old(&old_key, body);
    let tag_new = journal_tag_new(&new_key, body);

    assert_eq!(
        verify_journal_tags(&old_key, body, &tag_old, &tag_new),
        Some(KeyRole::Old)
    );
    assert_eq!(
        verify_journal_tags(&new_key, body, &tag_old, &tag_new),
        Some(KeyRole::New)
    );
}

#[test]
fn verify_reports_neither_for_an_unrelated_key() {
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();
    let other_key = MasterKey::generate();
    let body = b"the published body bytes";
    let tag_old = journal_tag_old(&old_key, body);
    let tag_new = journal_tag_new(&new_key, body);

    assert_eq!(
        verify_journal_tags(&other_key, body, &tag_old, &tag_new),
        None
    );
}

#[test]
fn verify_reports_neither_when_the_body_changed_by_one_byte() {
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();
    let body = b"the published body bytes".to_vec();
    let tag_old = journal_tag_old(&old_key, &body);
    let tag_new = journal_tag_new(&new_key, &body);

    let mut tampered = body.clone();
    tampered[0] ^= 1;
    assert_eq!(
        verify_journal_tags(&old_key, &tampered, &tag_old, &tag_new),
        None
    );
    assert_eq!(
        verify_journal_tags(&new_key, &tampered, &tag_old, &tag_new),
        None
    );
}

#[test]
fn old_and_new_tags_differ_even_for_the_same_key_and_body() {
    // Domain separation: if the old and new keys happened to be equal
    // (never true in a real rotation, but nothing here assumes otherwise),
    // the two tags still would not collide.
    let key = MasterKey::generate();
    let body = b"body";
    assert_ne!(journal_tag_old(&key, body), journal_tag_new(&key, body));
}

/// One tag, one spelling: the stored tag is exactly what `hex_encode`
/// wrote. A `+`-prefixed pair (which `u8::from_str_radix` would accept)
/// or an uppercase spelling of the *same* bytes does not verify.
#[test]
fn verify_accepts_only_the_lowercase_spelling_of_a_tag() {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = i as u8;
    }
    let key = MasterKey::from_bytes(bytes);
    let body = br#"{"x":1}"#;
    let tag_old = journal_tag_old(&key, body);
    let tag_new = journal_tag_new(&key, body);
    assert_eq!(
        verify_journal_tags(&key, body, &tag_old, &tag_new),
        Some(KeyRole::Old)
    );
    // Rewrite one pair below 0x10 (`0N`) as `+N`: the same byte to
    // `u8::from_str_radix`, a different spelling on disk.
    let pos = (0..tag_old.len())
        .step_by(2)
        .find(|&i| tag_old.as_bytes()[i] == b'0')
        .expect("the known vector has a pair below 0x10");
    let mut plus = tag_old.clone().into_bytes();
    plus[pos] = b'+';
    let plus = String::from_utf8(plus).expect("ascii");
    assert_eq!(verify_journal_tags(&key, body, &plus, &tag_new), None);
    assert_eq!(
        verify_journal_tags(&key, body, &tag_old.to_uppercase(), &tag_new),
        None
    );
}

#[test]
fn verify_rejects_a_malformed_stored_tag_without_erroring() {
    let key = MasterKey::generate();
    assert_eq!(
        verify_journal_tags(&key, b"body", "not-hex", "also-not-hex"),
        None
    );
}

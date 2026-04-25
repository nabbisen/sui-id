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

use super::*;
use sui_id_store::crypto::MasterKey;

#[tokio::test]
async fn reseal_one_round_trip() {
    let old = MasterKey::generate();
    let new = MasterKey::generate();
    let aad = b"test-aad";
    let plaintext = b"hello world".to_vec();
    let sealed = seal(&old, &plaintext, aad).expect("seal");
    let resealed = reseal_one(&old, &new, &sealed, aad).await.expect("reseal");
    // The re-sealed ciphertext must decrypt under the NEW key,
    // and must NOT decrypt under the OLD key.
    let opened_new = open(&new, &resealed, aad).expect("open with new");
    assert_eq!(opened_new, plaintext);
    assert!(open(&old, &resealed, aad).is_err());
}

#[tokio::test]
async fn reseal_one_fails_with_wrong_old_key() {
    let real_old = MasterKey::generate();
    let wrong_old = MasterKey::generate();
    let new = MasterKey::generate();
    let aad = b"test-aad";
    let sealed = seal(&real_old, b"data", aad).expect("seal");
    // Wrong "old" key: open should fail, error propagates.
    assert!(reseal_one(&wrong_old, &new, &sealed, aad).await.is_err());
}

#[tokio::test]
async fn reseal_one_with_wrong_aad_fails() {
    let old = MasterKey::generate();
    let new = MasterKey::generate();
    let sealed = seal(&old, b"data", b"correct-aad").expect("seal");
    assert!(reseal_one(&old, &new, &sealed, b"wrong-aad").await.is_err());
}

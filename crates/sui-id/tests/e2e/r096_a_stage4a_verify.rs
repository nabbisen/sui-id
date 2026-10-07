//! RFC 096-A stage 4a: `verify_id_token`'s full async chain -- fetch the JWKS over
//! real TLS, then verify a real RS256 signature against it -- driven the same way
//! as the stage-3a JWKS fetch tests: a hostile-provider-shaped fixture reached
//! through [`super::tls_mock::serve_raw_https`] and the production federation
//! client, not a unit-level constructed value.
//!
//! The RSA key here is a throwaway, generated once for this fixture and committed
//! as a PKCS#1 DER literal (the format `jsonwebtoken`'s `aws-lc-rs` backend expects
//! for RSA; see `crates/sui-id/src/http/id_token/tests.rs` for the DER-format finding
//! this follows). It signs nothing outside this file.

use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};

use super::tls_mock::{AfterWrite, FixtureResolver, federation_test_client, serve_raw_https};
use sui_id::id_token::verify_id_token;

const RSA_DER_B64: &str = "MIIEowIBAAKCAQEAqBlsoxL8V2qUYn7z0r6hVMRVNuUMCUmQPbBz/yAb14YjmZ60qc1Dx0GVo7f803msqmAmwfQbdcf01X98iTGnCXUHyAbm0oiMhyIR0noYSPkFAXQqNMgyh9J10DLZgmGIA9biG9Ggjwiw8mV1+UzqOKPwINtnyZVkDLYPcIwnPgC7zUjch8r6AxU+aib7O7UFVnmVJB/KEaSv/sKXlr3YCvX5ltzrzQhiM0XudXSWlgqDEGFbMn5m8IGaKoUh1ws8JLX06gQ3ahlSPgjdH36HLuJwLqDBFffkCVhVfMYBsp39jMttqFheG4Brm2VBjLljFJ+5fXSJfSpweatBf3thKwIDAQABAoIBAA2BJWIgd1dKf6s+CbaHjAx0TWhAlGv2lfjuwRLg8HurEhcYUelGTUinsy7Y7B3NK5rwaIyyYLZgnwG74TRgYcv3s+1U6JxHTgrZKNdg6ayLHOTWAUIGR1D0DnTwcNrxnOy6CaY0fBKhnx2KGyUxEawFN5hcKAVg1A3VXn4rNEfdw+PT71+Le/ca1kLYmHX4b7KBvwfXIBzjAQTp9vdacOPtFX30ADFgvl0+tZrofQy1X1K6Dkx5ypG7AsTXALggBm7ia8/tTcWhIyeiZFgv+rPmajQJhK5D01JGpea1kRED57TCSkseA0k4p/a5VfcME2Lk4S2ACaFokxuEQsEedVkCgYEAz+f3CxVXzNf8NNbpqlTqihNKF2VfA83TJgBRYbdUe5BaaCdu1KoDRajJiRzZsBm1MYbwpOht95TxTxO8CyvS1CexCgUeYqPEPbKrVcppc90bmir0PhRAFU2vJsI12KdA0esz+lg5fwHhWmhSHg0eTBPHrizfg+eL0F0TmRQlm5UCgYEAzvwh3bmwg4rxfTdM6Lk8eEyYyE3kORAs24nkywhVEtiMbRBAO1euhshhICyYw1JZ4zTd0h/a5h5FPmfZ4ontD+NOAzzA/VRdTtq9dnsKh5U2CB9nCxB5iqfr4nh2NOFOc4MnXzY8/4UtnurCAIUZJe6Y7ZZxX/PwMJDt5uGf2b8CgYEAqutyp4P2WIs/5ljAZK3G25icErvd7wypB9a/EOxc4fB8wp+Yd/EFG6F1felf9WxND9h1wbzrmtyxvWbl8vEmNBAlda1bm5Ay5t4aCT+Mjho6dPXXMaoIPPtOgTisd96YZXtNkgQx0H5FO8QDrnzuaXDhegmd/5y9zqCWWgS8HjkCgYAiXyY0jrhsL6+Ibp176/7Jr1aTtLOYckIwtsZinOCbv5AaMF+qOxZFVZMjZ6R6kvtQSqAnW5jbK92tzksVXngaclGrIfSeXNsd1B8wRKBsAXA2ixhucu7sApSeSAjBIUUI05e/LN+WQwRfZnaO5YtWjDejBJ+RIo4ZUoffFkqYOwKBgASHkA2pUP0n0XXa142o8gTtDvvw761KFQJ42YAHGS6DXFnYF4/9xKtDvHgDsyDf5iavE6QA9Yxez+owBqv20cqMvcvk+kvUYRrTw3/jiyQqcvkYUba2dpF5wxY76m9h7UW/kLXdj5KafJ2ILV5mCu5j7j02eHt1StRx7yGlXfKD";
const RSA_N: &str = "qBlsoxL8V2qUYn7z0r6hVMRVNuUMCUmQPbBz_yAb14YjmZ60qc1Dx0GVo7f803msqmAmwfQbdcf01X98iTGnCXUHyAbm0oiMhyIR0noYSPkFAXQqNMgyh9J10DLZgmGIA9biG9Ggjwiw8mV1-UzqOKPwINtnyZVkDLYPcIwnPgC7zUjch8r6AxU-aib7O7UFVnmVJB_KEaSv_sKXlr3YCvX5ltzrzQhiM0XudXSWlgqDEGFbMn5m8IGaKoUh1ws8JLX06gQ3ahlSPgjdH36HLuJwLqDBFffkCVhVfMYBsp39jMttqFheG4Brm2VBjLljFJ-5fXSJfSpweatBf3thKw";
const RSA_E: &str = "AQAB";

fn der(b64_std: &str) -> Vec<u8> {
    Base64::decode_vec(b64_std).expect("valid base64")
}

fn honest(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn jwks_body() -> String {
    serde_json::json!({
        "keys": [{"kty": "RSA", "kid": "e2e-rsa", "n": RSA_N, "e": RSA_E}]
    })
    .to_string()
}

fn signed_token() -> String {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER_B64));
    let mut header = JwtHeader::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("e2e-rsa".to_string());
    let claims = serde_json::json!({
        "sub": "upstream-user",
        "aud": "this-rp-client-id",
        "exp": 9_999_999_999i64,
    });
    encode(&header, &claims, &key).expect("sign RS256")
}

/// The full async chain, over real TLS: fetch the JWKS from the fixture, then
/// verify a real RS256 signature against the key it served. Everything below
/// `verify_id_token` (`fetch_jwks`, `parse_compact_jws`, `select_key`, the
/// `jsonwebtoken::decode` call) runs exactly as production would call it; only
/// the resolver and trust root differ, per [`federation_test_client`].
#[tokio::test]
async fn verify_id_token_succeeds_end_to_end_over_real_tls() {
    let body = jwks_body();
    let (base, addr) = serve_raw_https(move |_| honest(&body), AfterWrite::Close).await;
    let client = federation_test_client(FixtureResolver { addr });
    let token = signed_token();

    let claims = verify_id_token(
        &client,
        &token,
        &["RS256".to_string()],
        Some(&format!("{base}/jwks")),
    )
    .await
    .expect("a real signature over a real TLS-fetched JWKS verifies");

    assert_eq!(claims.sub(), Some("upstream-user"));
}

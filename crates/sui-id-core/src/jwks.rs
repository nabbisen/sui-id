//! JWKS document construction.
//!
//! Only EdDSA / OKP / Ed25519 keys are emitted, since that is the single
//! algorithm sui-id signs with. The shape follows RFC 7517 / RFC 8037.

use base64ct::{Base64UrlUnpadded, Encoding};
use serde::Serialize;
use sui_id_store::{Database, models::SigningKeyRow, repos::signing_keys};

#[derive(Debug, Serialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Serialize)]
pub struct Jwk {
    pub kty: &'static str,
    pub crv: &'static str,
    pub kid: String,
    #[serde(rename = "use")]
    pub use_: &'static str,
    pub alg: &'static str,
    pub x: String,
}

fn b64u(b: &[u8]) -> String {
    let mut out = vec![0u8; b.len() * 2 + 4];
    let n = Base64UrlUnpadded::encode(b, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).expect("base64url is ascii")
}

fn to_jwk(row: &SigningKeyRow) -> Jwk {
    Jwk {
        kty: "OKP",
        crv: "Ed25519",
        kid: row.id.to_string(),
        use_: "sig",
        alg: "EdDSA",
        x: b64u(&row.public_key),
    }
}

/// Build the JWKS document published at `/.well-known/jwks.json`.
pub async fn build(db: &Database) -> Result<Jwks, sui_id_store::StoreError> {
    let rows = signing_keys::list_published(db).await?;
    Ok(Jwks {
        keys: rows.iter().map(to_jwk).collect(),
    })
}

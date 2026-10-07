// RFC 096 :648 (stage 4a): claims are exposed only once signature
// verification has completed. `VerifiedIdTokenClaims`'s only field is
// private and there is no public constructor; the only way to have one is
// `id_token::verify_id_token` or `verify_id_token_against_jwks`'s success
// path. This fixture tries the direct route from outside the crate, the
// same proof shape as `validated_discovery_cannot_be_constructed_directly.rs`.

use sui_id::id_token::VerifiedIdTokenClaims;

fn attempt() -> VerifiedIdTokenClaims {
    VerifiedIdTokenClaims {
        payload: serde_json::json!({"sub": "not-verified"}),
    }
}

fn main() {}

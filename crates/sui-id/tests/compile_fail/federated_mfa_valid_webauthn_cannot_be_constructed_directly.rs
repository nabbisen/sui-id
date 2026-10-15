// RFC 096-B1 stage 11, RFC 096 :757-762: `ValidWebAuthn`'s fields are
// private and there is no public constructor; the only way to hold one
// is `federated_mfa::verify_webauthn`'s own return value. This fixture
// tries the direct route from outside the crate -- the same proof
// shape as `federated_mfa_valid_totp_cannot_be_constructed_directly.rs`,
// for the fourth and last candidate family member.

use sui_id_core::federated_mfa::ValidWebAuthn;
use sui_id_shared::ids::{FederationWebauthnCeremonyId, WebauthnCredentialId};

fn attempt() -> ValidWebAuthn {
    ValidWebAuthn {
        ceremony_id: FederationWebauthnCeremonyId::new(),
        credential_id: WebauthnCredentialId::new(),
        expected_sealed: Vec::new(),
        new_sealed: Vec::new(),
    }
}

fn main() {}

// RFC 096-B1 stage 10a, RFC 096 :759 ("Only the verifier selects the
// closed rejection reason; handlers can neither construct a candidate
// nor supply a row-touching reason."). `BoundRejected`'s field is
// private and there is no public constructor -- a handler cannot even
// hand in a reason of its own choosing, let alone fabricate the whole
// candidate.

use sui_id_core::federated_mfa::{BoundRejected, RejectedReason};

fn attempt() -> BoundRejected {
    BoundRejected {
        reason: RejectedReason::Totp,
    }
}

fn main() {}

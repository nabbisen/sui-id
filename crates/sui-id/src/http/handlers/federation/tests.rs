use super::*;

/// The census (dispatch item 1), each value checked against the
/// bucket this package's review package states for it -- not assumed
/// from the bucket's own doc comment, which could drift from
/// `classify`'s real `match` without this failing.
#[test]
fn every_known_value_classifies_as_declared() {
    let retry = [
        "upstream",
        "state_mismatch",
        "attempt_not_found",
        "replay",
        "expired",
        "clock_regression",
        "tamper_detected",
        "superseded",
        "binding_mismatch",
        "attempt_claim_failed",
        "token_exchange",
        "token_parse",
        "missing_id_token",
        "signature_invalid",
        "nonce_mismatch",
        "identity_claims_invalid",
        "time_claims_invalid",
        "optional_claims_invalid",
        "userinfo",
        "no_id_token",
        "no_sub",
    ];
    for v in retry {
        assert_eq!(FedErrorBucket::classify(v), FedErrorBucket::Retry, "{v}");
    }
    let contact_admin = [
        "disabled",
        "config_drift",
        "discovery_origin",
        "signin_failed",
    ];
    for v in contact_admin {
        assert_eq!(
            FedErrorBucket::classify(v),
            FedErrorBucket::ContactAdmin,
            "{v}"
        );
    }
    let link_required = ["email_collision", "link_required"];
    for v in link_required {
        assert_eq!(
            FedErrorBucket::classify(v),
            FedErrorBucket::LinkRequired,
            "{v}"
        );
    }
    assert_eq!(
        FedErrorBucket::classify("unverified_email"),
        FedErrorBucket::UnverifiedEmail
    );
}

/// RFC 096 `:699-701`/`:707-708`'s own oracle concern, as a direct
/// equality — not merely that both are *some* bucket, but the *same*
/// one, so the rendered message cannot distinguish them.
#[test]
fn email_collision_and_link_required_render_the_same_message() {
    assert_eq!(
        FedErrorBucket::classify("email_collision"),
        FedErrorBucket::classify("link_required")
    );
}

/// Dispatch item 3: an unrecognised value gets the generic message,
/// never its own treatment and never one of the other three.
#[test]
fn an_unrecognised_value_is_the_generic_message() {
    assert_eq!(
        FedErrorBucket::classify("not-a-real-fed-error-value"),
        FedErrorBucket::Retry
    );
    assert_eq!(FedErrorBucket::classify(""), FedErrorBucket::Retry);
}

/// `fed_error_message` never reflects its input: the returned
/// `Flash::text` is always one of the four fixed, localized strings,
/// never a copy of (or containing) the caller-supplied value, even a
/// markup-shaped one.
#[test]
fn the_returned_flash_never_contains_the_input_value() {
    let t = sui_id_i18n::Locale::En.strings();
    let payload = "<script>alert(document.cookie)</script>";
    let flash = fed_error_message(payload, t);
    assert!(!flash.text.contains(payload));
    assert!(!flash.text.contains("<script"));
}

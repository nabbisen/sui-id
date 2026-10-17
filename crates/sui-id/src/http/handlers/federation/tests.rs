use super::*;
use base64ct::Encoding;

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
        "token_endpoint_auth_method",
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

fn decode_form_urlencoded(s: &str) -> String {
    percent_encoding::percent_decode_str(&s.replace('+', " "))
        .decode_utf8()
        .expect("valid utf8")
        .into_owned()
}

/// RFC 096-B1 stage 12c, RFC 6749 `:2.3.1`: `client_id`/`client_secret`
/// must be `application/x-www-form-urlencoded` *before* the colon join
/// and the base64 step -- not a plain HTTP Basic auth encode of the raw
/// bytes. Both `client_id` and `secret` here carry a `:` themselves (the
/// exact separator character), a `&`, a `=`, and a space -- a value
/// skipping the pre-encoding step would either corrupt on decode or,
/// worse, make the credential ambiguous at the colon split. This is the
/// test the module doc names: nothing here would catch a wrong
/// implementation if either value only ever held unreserved characters.
#[test]
fn basic_auth_header_form_urlencodes_before_base64_with_reserved_characters() {
    let client_id = "client:with/reserved chars";
    let secret = "secret&with=reserved%chars:too";

    let header = basic_auth_header(client_id, secret);
    assert!(
        header.starts_with("Basic "),
        "must be an RFC 7617 Basic auth value: {header}"
    );

    let b64 = &header["Basic ".len()..];
    let decoded_bytes = base64ct::Base64::decode_vec(b64).expect("valid standard base64");
    let decoded = String::from_utf8(decoded_bytes).expect("utf8 credentials");

    // Every `:` in either original value must have been percent-encoded
    // (`%3A`) before the join -- only the one separator this function adds
    // itself may remain literal. Skipping the pre-encoding step would
    // leave `client_id`'s and `secret`'s own `:` characters as extra,
    // ambiguous separators.
    assert_eq!(
        decoded.matches(':').count(),
        1,
        "exactly one literal `:` (the separator); every other `:` in \
         client_id/secret must be percent-encoded. Got: {decoded:?}"
    );

    let (user, pass) = decoded.split_once(':').expect("one separator");
    assert_eq!(decode_form_urlencoded(user), client_id);
    assert_eq!(decode_form_urlencoded(pass), secret);
}

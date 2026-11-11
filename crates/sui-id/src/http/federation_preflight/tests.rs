use super::*;

/// RFC 096-B2 S3 dispatch §B, "Errors": "No upstream body, URL query or
/// key material in any variant." Every variant is a bare unit (no
/// embedded data at all), so this is true by construction -- this test
/// pins the operator-facing `Display` text instead, and that every
/// variant's text names a plain, bounded reason.
#[test]
fn every_variant_has_a_bounded_operator_facing_message() {
    // Each message names the reason and the next step (the C17 decision,
    // §3 item 6): the second substring is the next step.
    let cases = [
        (
            PreflightError::NotFound,
            "no longer exists",
            "reload the provider list",
        ),
        (PreflightError::Storage, "could not be read", "try again"),
        (
            PreflightError::NotDisabled,
            "already enabled",
            "disable it first",
        ),
        (
            PreflightError::NotInConfiguration,
            "no [[federation_providers]] block",
            "add one and restart",
        ),
        (
            PreflightError::PolicyNotRecorded,
            "has not been recorded",
            "restart sui-id",
        ),
        (
            PreflightError::Transport,
            "could not be reached",
            "check this host's network",
        ),
        (
            PreflightError::Discovery,
            "discovery document was refused",
            "see the server log",
        ),
        (
            PreflightError::Jwks,
            "signing keys were refused",
            "see the server log",
        ),
        (
            PreflightError::AuthMethodUnsupported,
            "does not accept this provider's token-auth method",
            "check client_secret_env",
        ),
        (
            PreflightError::NoCommonAlgorithm,
            "none of this provider's id_token_algs",
            "add one it supports",
        ),
        (
            PreflightError::ChangedDuringPreflight,
            "changed while the check was running",
            "run the check again",
        ),
    ];
    for (err, reason, next_step) in cases {
        let text = err.to_string();
        assert!(text.contains(reason), "{err:?}: {text:?} lacks {reason:?}");
        assert!(
            text.contains(next_step),
            "{err:?}: {text:?} lacks {next_step:?}"
        );
    }
}

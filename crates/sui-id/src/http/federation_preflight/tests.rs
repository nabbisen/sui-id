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

/// RFC 096-B2 S4 §D: `check_and_enable`'s own closed-enum refusals, each
/// pinned the same way -- reason and next step, same as S3's own fold-in
/// above. `ChangedSinceCheck`/`CheckTooOld`/`AlreadyEnabled` are C17's
/// own; the rest are S3's `PreflightError` variants carried through
/// unchanged ([`every_variant_has_a_bounded_operator_facing_message`]
/// already pins their text, so this test only pins the three new ones).
#[test]
fn check_and_enable_errors_have_a_bounded_operator_facing_message() {
    let cases = [
        (
            CheckAndEnableError::ChangedSinceCheck,
            "changed since the check",
            "run it again",
        ),
        (
            CheckAndEnableError::CheckTooOld,
            "more than 10 minutes old",
            "run it again",
        ),
        (
            CheckAndEnableError::AlreadyEnabled,
            "already enabled",
            "disable it first",
        ),
        (
            CheckAndEnableError::StepUpRequired,
            "identity confirmation expired",
            "confirm it again",
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

/// RFC 096-B2 S4 §D: the store's `EnableFromPreflightError` folds into
/// `CheckAndEnableError` one-to-one. `StepUpRequired` keeps its own
/// variant (S4 review): folded into `Storage`, its message would tell an
/// administrator whose confirmation lapsed to "try again, see the log".
#[test]
fn enable_from_preflight_error_folds_into_check_and_enable_error() {
    use sui_id_store::commands::EnableFromPreflightError as E;
    let cases = [
        (E::ChangedSinceCheck, CheckAndEnableError::ChangedSinceCheck),
        (E::CheckTooOld, CheckAndEnableError::CheckTooOld),
        (E::AlreadyEnabled, CheckAndEnableError::AlreadyEnabled),
        (E::NotFound, CheckAndEnableError::NotFound),
        (E::StepUpRequired, CheckAndEnableError::StepUpRequired),
        (E::Storage, CheckAndEnableError::Storage),
    ];
    for (store_err, expected) in cases {
        assert_eq!(
            CheckAndEnableError::from(store_err),
            expected,
            "{store_err:?} must fold into {expected:?}"
        );
    }
}

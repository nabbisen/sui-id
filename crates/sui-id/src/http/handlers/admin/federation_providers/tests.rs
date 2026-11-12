//! RFC 096-B2 S5 §B/§D: every [`CheckAndEnableError`]/[`PreflightError`]/
//! [`DisableFederationProviderError`] variant translates to a non-empty
//! key in **both** `Locale::ALL` members, and that text actually reaches
//! the rendered confirm page — the same "pin the text, through the
//! renderer" shape S3's `every_variant_has_a_bounded_operator_facing_
//! message` used for the untranslated `Display` text this builds on.

use super::*;
use sui_id_i18n::Locale;

fn enable_confirm_html(flash: Option<Flash>) -> String {
    render_confirm_enable_federation_provider(
        ConfirmEnableFederationProviderData {
            provider_id: "ignored".into(),
            display_name: "Up".into(),
            csrf_token: "tok".into(),
            flash,
        },
        false,
        Locale::En,
    )
}

fn disable_confirm_html(flash: Option<Flash>) -> String {
    render_confirm_disable_federation_provider(
        ConfirmDisableFederationProviderData {
            provider_id: "ignored".into(),
            display_name: "Up".into(),
            csrf_token: "tok".into(),
            flash,
        },
        false,
        Locale::En,
    )
}

#[test]
fn every_check_and_enable_error_renders_a_nonempty_message_in_every_locale() {
    let variants = [
        CheckAndEnableError::NotFound,
        CheckAndEnableError::Storage,
        CheckAndEnableError::NotDisabled,
        CheckAndEnableError::NotInConfiguration,
        CheckAndEnableError::PolicyNotRecorded,
        CheckAndEnableError::Transport,
        CheckAndEnableError::Discovery,
        CheckAndEnableError::Jwks,
        CheckAndEnableError::AuthMethodUnsupported,
        CheckAndEnableError::NoCommonAlgorithm,
        CheckAndEnableError::ChangedDuringPreflight,
        CheckAndEnableError::ChangedSinceCheck,
        CheckAndEnableError::CheckTooOld,
        CheckAndEnableError::AlreadyEnabled,
        CheckAndEnableError::StepUpRequired,
    ];
    for locale in Locale::ALL {
        for variant in variants {
            let text = check_and_enable_error_text(variant, locale.strings());
            assert!(
                !text.is_empty(),
                "{variant:?} has no {locale:?} translation"
            );
            let html = enable_confirm_html(Some(Flash {
                kind: FlashKind::Error,
                text: text.into(),
            }));
            assert!(
                html.contains(text),
                "{variant:?}'s {locale:?} text did not reach the rendered page"
            );
        }
    }
}

#[test]
fn every_preflight_error_renders_a_nonempty_message_in_every_locale() {
    let variants = [
        PreflightError::NotFound,
        PreflightError::Storage,
        PreflightError::NotDisabled,
        PreflightError::NotInConfiguration,
        PreflightError::PolicyNotRecorded,
        PreflightError::Transport,
        PreflightError::Discovery,
        PreflightError::Jwks,
        PreflightError::AuthMethodUnsupported,
        PreflightError::NoCommonAlgorithm,
        PreflightError::ChangedDuringPreflight,
    ];
    for locale in Locale::ALL {
        for variant in variants {
            let text = preflight_error_text(variant, locale.strings());
            assert!(
                !text.is_empty(),
                "{variant:?} has no {locale:?} translation"
            );
            let html = enable_confirm_html(Some(Flash {
                kind: FlashKind::Error,
                text: text.into(),
            }));
            assert!(
                html.contains(text),
                "{variant:?}'s {locale:?} text did not reach the rendered page"
            );
        }
    }
}

#[test]
fn every_disable_error_renders_a_nonempty_message_in_every_locale() {
    let variants = [
        DisableFederationProviderError::NotFound,
        DisableFederationProviderError::StepUpRequired,
        DisableFederationProviderError::Storage,
    ];
    for locale in Locale::ALL {
        for variant in variants {
            let text = disable_error_text(variant, locale.strings());
            assert!(
                !text.is_empty(),
                "{variant:?} has no {locale:?} translation"
            );
            let html = disable_confirm_html(Some(Flash {
                kind: FlashKind::Error,
                text: text.into(),
            }));
            assert!(
                html.contains(text),
                "{variant:?}'s {locale:?} text did not reach the rendered page"
            );
        }
    }
}

/// RFC 096-B2 S5 §D: "No trust field is editable" — no `<input>`/
/// `<select>` other than the CSRF token inside either confirm
/// page's own `<form>`s. Scoped to `<form>...</form>` rather than
/// the whole document: the shared `<Shell>` chrome around every
/// admin page has its own unrelated `<input type="checkbox">` (the
/// mobile-nav toggle), which is not this page's content.
#[test]
fn no_confirm_page_form_has_an_input_or_select_other_than_csrf_and_confirmed() {
    let allowed = ["_csrf", "_confirmed"];
    for html in [enable_confirm_html(None), disable_confirm_html(None)] {
        let mut rest = html.as_str();
        while let Some(start) = rest.find("<form") {
            let after_start = &rest[start..];
            let form_end = after_start.find("</form>").expect("form closes");
            let form = &after_start[..form_end];
            for tag in ["input", "select"] {
                for cap in form.match_indices(&format!("<{tag}")) {
                    let tag_rest = &form[cap.0..];
                    let end = tag_rest.find('>').expect("tag closes");
                    let tag_text = &tag_rest[..end];
                    if tag == "input" {
                        let name = tag_text
                            .split("name=\"")
                            .nth(1)
                            .and_then(|s| s.split('"').next())
                            .unwrap_or_else(|| panic!("<input> with no name: {tag_text}"));
                        assert!(
                            allowed.contains(&name),
                            "unexpected <input name=\"{name}\"> on a federation-provider \
                             confirm page -- RFC 096 `:380` forbids any trust-field input"
                        );
                    } else {
                        panic!("<select> on a federation-provider confirm page: {tag_text}");
                    }
                }
            }
            rest = &after_start[form_end + "</form>".len()..];
        }
    }
}

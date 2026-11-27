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
            username: "admin".into(),
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
            username: "admin".into(),
        },
        false,
        Locale::En,
    )
}

/// `0.93.0` O6: `CheckAndEnableError` is no longer `Copy` (its
/// `Transport`/`Discovery`/`Jwks` payloads are not), so the sample set
/// is built fresh each time, not hoisted and iterated twice. A
/// representative payload stands in for each typed variant; the typed
/// causes themselves are each pinned separately, below.
fn sample_check_and_enable_errors() -> Vec<CheckAndEnableError> {
    vec![
        CheckAndEnableError::NotFound,
        CheckAndEnableError::Storage,
        CheckAndEnableError::NotDisabled,
        CheckAndEnableError::NotInConfiguration,
        CheckAndEnableError::PolicyNotRecorded,
        CheckAndEnableError::Transport(TransportCause::Unreachable),
        CheckAndEnableError::Discovery(DiscoveryError::NotJson),
        CheckAndEnableError::NoJwksUri,
        CheckAndEnableError::Jwks(JwksError::NotJson),
        CheckAndEnableError::NoUsableKey,
        CheckAndEnableError::AuthMethodUnsupported,
        CheckAndEnableError::NoCommonAlgorithm,
        CheckAndEnableError::ChangedDuringPreflight,
        CheckAndEnableError::ChangedSinceCheck,
        CheckAndEnableError::CheckTooOld,
        CheckAndEnableError::AlreadyEnabled,
        CheckAndEnableError::StepUpRequired,
    ]
}

fn sample_preflight_errors() -> Vec<PreflightError> {
    vec![
        PreflightError::NotFound,
        PreflightError::Storage,
        PreflightError::NotDisabled,
        PreflightError::NotInConfiguration,
        PreflightError::PolicyNotRecorded,
        PreflightError::Transport(TransportCause::Unreachable),
        PreflightError::Discovery(DiscoveryError::NotJson),
        PreflightError::NoJwksUri,
        PreflightError::Jwks(JwksError::NotJson),
        PreflightError::NoUsableKey,
        PreflightError::AuthMethodUnsupported,
        PreflightError::NoCommonAlgorithm,
        PreflightError::ChangedDuringPreflight,
    ]
}

#[test]
fn every_check_and_enable_error_renders_a_nonempty_message_in_every_locale() {
    for locale in Locale::ALL {
        for variant in sample_check_and_enable_errors() {
            let debug = format!("{variant:?}");
            let text = check_and_enable_error_text(variant, locale.strings());
            assert!(!text.is_empty(), "{debug} has no {locale:?} translation");
            let html = enable_confirm_html(Some(Flash {
                kind: FlashKind::Error,
                text: text.clone(),
            }));
            assert!(
                html.contains(&text),
                "{debug}'s {locale:?} text did not reach the rendered page"
            );
        }
    }
}

#[test]
fn every_preflight_error_renders_a_nonempty_message_in_every_locale() {
    for locale in Locale::ALL {
        for variant in sample_preflight_errors() {
            let debug = format!("{variant:?}");
            let text = preflight_error_text(variant, locale.strings());
            assert!(!text.is_empty(), "{debug} has no {locale:?} translation");
            let html = enable_confirm_html(Some(Flash {
                kind: FlashKind::Error,
                text: text.clone(),
            }));
            assert!(
                html.contains(&text),
                "{debug}'s {locale:?} text did not reach the rendered page"
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

/// S7 review: delete's refusals, which S6 had no test for. Each variant
/// gives a message in every locale, and the still-enabled one is about
/// deleting: it used to reuse the check flow's "disable it first to check
/// it again", which is the wrong next step for a delete.
#[test]
fn every_delete_error_has_its_own_message_in_every_locale() {
    let variants = [
        DeleteProviderError::NotFound,
        DeleteProviderError::StillEnabled,
        DeleteProviderError::StillConfigured,
        DeleteProviderError::StepUpRequired,
        DeleteProviderError::Storage,
    ];
    for locale in Locale::ALL {
        let t = locale.strings();
        for v in variants {
            assert!(
                !delete_provider_error_text(v, t).is_empty(),
                "{v:?} has no {locale:?} message"
            );
        }
        assert_ne!(
            delete_provider_error_text(DeleteProviderError::StillEnabled, t),
            t.fed_err_not_disabled,
            "{locale:?}: delete must not reuse the check flow's message"
        );
    }
    let en = Locale::En.strings();
    let text = delete_provider_error_text(DeleteProviderError::StillEnabled, en);
    assert!(text.contains("then delete it"), "{text}");
    assert!(!text.contains("check it again"), "{text}");
}

// ---------- `0.93.0` O6 §3: every typed cause reaches a sentence ----------

fn every_discovery_cause() -> Vec<DiscoveryError> {
    vec![
        DiscoveryError::InvalidIssuer("https://bad issuer".to_owned()),
        DiscoveryError::NotAbsolute {
            field: "token_endpoint",
            value: "not a url".to_owned(),
        },
        DiscoveryError::NotHttps {
            field: "token_endpoint",
            value: "http://example.com".to_owned(),
        },
        DiscoveryError::HasUserinfo {
            field: "token_endpoint",
        },
        DiscoveryError::HasQuery {
            field: "token_endpoint",
        },
        DiscoveryError::HasFragment {
            field: "token_endpoint",
        },
        DiscoveryError::Noncanonical {
            field: "token_endpoint",
        },
        DiscoveryError::OriginNotAllowed {
            field: "token_endpoint",
            origin: "https://oauth2.googleapis.com".to_owned(),
        },
        DiscoveryError::IssuerMismatch,
        DiscoveryError::ResponseTypesMissingCode,
        DiscoveryError::GrantTypesMissingAuthorizationCode,
        DiscoveryError::CodeChallengeMethodsMissingS256,
        DiscoveryError::AuthorizationResponseIssNotTrue,
        DiscoveryError::SubjectTypesMissingPublic,
        DiscoveryError::TooDeep { limit: 8 },
        DiscoveryError::DuplicateMember("issuer".to_owned()),
        DiscoveryError::NotJson,
    ]
}

fn every_jwks_cause() -> Vec<JwksError> {
    vec![
        JwksError::NotJson,
        JwksError::NotObject,
        JwksError::NoKeys,
        JwksError::TooDeep { limit: 16 },
        JwksError::DuplicateMember("keys".to_owned()),
        JwksError::TooManyKeys { limit: 32 },
        JwksError::KeyNotObject { index: 0 },
        JwksError::DuplicateKid("abc".to_owned()),
    ]
}

fn every_transport_cause() -> Vec<TransportCause> {
    vec![
        TransportCause::NoCaCertificates,
        TransportCause::DnsAnswerRejected {
            host: "www.googleapis.com".to_owned(),
            count: 40,
        },
        TransportCause::DnsDenied {
            host: "www.example.com".to_owned(),
            class: crate::resolver::DeniedPrefixClass::PrivateNetwork,
        },
        TransportCause::Tls,
        TransportCause::Timeout,
        TransportCause::Unreachable,
    ]
}

/// Dispatch §3's own exhaustive-match requirement, proven directly: a
/// new variant of any of the three cause enums fails to compile against
/// `discovery_cause_text`/`jwks_cause_text`/`transport_cause_text`
/// (each a non-wildcard `match`) until it is handled -- these tests
/// just confirm every *existing* one reaches a non-empty sentence, in
/// every locale, with no `Bounds`-recursion case ever reached twice.
#[test]
fn every_discovery_cause_reaches_a_nonempty_sentence_in_every_locale() {
    for locale in Locale::ALL {
        for cause in every_discovery_cause() {
            let text = discovery_cause_text(&cause, locale.strings());
            assert!(!text.is_empty(), "{cause:?} has no {locale:?} sentence");
        }
        // `Bounds`'s own inner value is never constructed directly here
        // (it is a `response_bounds::BoundsError`, exercised through
        // `classify_discovery_bounds`'s own tests instead); confirm its
        // arm alone still reaches a sentence via a representative value.
        let text = discovery_cause_text(
            &DiscoveryError::Bounds(crate::response_bounds::BoundsError::MissingContentType),
            locale.strings(),
        );
        assert!(
            !text.is_empty(),
            "Discovery::Bounds has no {locale:?} sentence"
        );
    }
}

#[test]
fn every_jwks_cause_reaches_a_nonempty_sentence_in_every_locale() {
    for locale in Locale::ALL {
        for cause in every_jwks_cause() {
            let text = jwks_cause_text(&cause, locale.strings());
            assert!(!text.is_empty(), "{cause:?} has no {locale:?} sentence");
        }
        let text = jwks_cause_text(
            &JwksError::Bounds(crate::response_bounds::BoundsError::MissingContentType),
            locale.strings(),
        );
        assert!(!text.is_empty(), "Jwks::Bounds has no {locale:?} sentence");
    }
}

#[test]
fn every_transport_cause_reaches_a_nonempty_sentence_in_every_locale() {
    for locale in Locale::ALL {
        for cause in every_transport_cause() {
            let text = transport_cause_text(&cause, locale.strings());
            assert!(!text.is_empty(), "{cause:?} has no {locale:?} sentence");
        }
    }
}

/// RFC 096's "browser responses ... do not echo ..." rule, applied here
/// too: no rendered cause sentence may contain anything shaped like a
/// URL (`://` followed by `/`, `?`, or `#`), in any locale.
#[test]
fn no_rendered_cause_sentence_contains_a_url() {
    fn assert_no_url(text: &str, label: &str) {
        let has_url = text.match_indices("://").any(|(i, _)| {
            text[i + 3..]
                .chars()
                .next()
                .is_some_and(|c| c == '/' || c == '?' || c == '#')
        });
        assert!(!has_url, "{label}: {text:?} looks like it contains a URL");
    }
    for locale in Locale::ALL {
        let t = locale.strings();
        for cause in every_discovery_cause() {
            assert_no_url(&discovery_cause_text(&cause, t), "discovery");
        }
        for cause in every_jwks_cause() {
            assert_no_url(&jwks_cause_text(&cause, t), "jwks");
        }
        for cause in every_transport_cause() {
            assert_no_url(&transport_cause_text(&cause, t), "transport");
        }
    }
}

/// Dispatch §3's own test: "an `InvalidIssuer` carrying hostile text
/// renders none of it."
#[test]
fn invalid_issuer_renders_none_of_its_hostile_text() {
    let hostile = "<script>alert(document.cookie)</script>";
    for locale in Locale::ALL {
        let text = discovery_cause_text(
            &DiscoveryError::InvalidIssuer(hostile.to_owned()),
            locale.strings(),
        );
        assert!(
            !text.contains(hostile) && !text.contains("<script>"),
            "{locale:?}: {text:?} contains the hostile issuer text"
        );
    }
}

/// A `NotAbsolute`/`NotHttps`'s raw `value` -- upstream-supplied,
/// unbounded, possibly hostile -- must never reach the page either,
/// only the (sui-id-chosen, closed-set) field name.
#[test]
fn not_absolute_and_not_https_render_none_of_their_raw_value() {
    let hostile = "javascript:alert(1)//attacker-controlled";
    for locale in Locale::ALL {
        let t = locale.strings();
        let text = discovery_cause_text(
            &DiscoveryError::NotAbsolute {
                field: "token_endpoint",
                value: hostile.to_owned(),
            },
            t,
        );
        assert!(
            !text.contains(hostile),
            "{locale:?}: {text:?} leaks NotAbsolute's value"
        );
        let text = discovery_cause_text(
            &DiscoveryError::NotHttps {
                field: "token_endpoint",
                value: hostile.to_owned(),
            },
            t,
        );
        assert!(
            !text.contains(hostile),
            "{locale:?}: {text:?} leaks NotHttps's value"
        );
    }
}

/// `bounded` truncates past 255 bytes and marks the elision, rather
/// than ever rendering an unbounded upstream-influenced host or origin.
#[test]
fn bounded_truncates_past_255_bytes_at_a_char_boundary() {
    let long = "a".repeat(400);
    let out = bounded(&long);
    // 255 content bytes plus the ellipsis mark, which is itself a
    // 3-byte UTF-8 character (U+2026) -- not 256, which would assume
    // a 1-byte marker.
    assert!(
        out.len() <= 255 + '…'.len_utf8(),
        "bounded output is not actually bounded: {} bytes",
        out.len()
    );
    assert!(out.ends_with('…'), "{out:?} does not mark the elision");

    let short = "short-host.example";
    assert_eq!(
        bounded(short),
        short,
        "a short string is returned unchanged"
    );
}

// ---------- review fold-ins (2026-11-22) ----------

/// The cap in the sentence is the resolver's own constant, in every
/// locale: a hard-coded "32" drifts the day the cap changes.
#[test]
fn the_dns_cap_sentence_states_the_resolvers_own_cap() {
    let cap = crate::resolver::MAX_ANSWERS;
    for locale in Locale::ALL {
        let text = transport_cause_text(
            &TransportCause::DnsAnswerRejected {
                host: "www.example.com".to_owned(),
                count: cap + 1,
            },
            locale.strings(),
        );
        assert!(text.contains(&cap.to_string()), "{locale:?}: {text}");
        assert!(text.contains(&(cap + 1).to_string()), "{locale:?}: {text}");
    }
}

/// `InvalidIssuer` is raised for the configured issuer *and* for a
/// configured `allowed_origins` that does not parse
/// (`discovery.rs`), so its sentence names both.
#[test]
fn invalid_issuer_names_both_settings_it_can_come_from() {
    let text = discovery_cause_text(
        &DiscoveryError::InvalidIssuer("x".to_owned()),
        Locale::En.strings(),
    );
    assert!(
        text.contains("issuer") && text.contains("allowed_origins"),
        "{text}"
    );
}

/// Every `BoundsError` the pipeline can raise reaches a specific
/// sentence, never "check the server log" alone, and never the
/// upstream's own content-type text.
#[test]
fn every_bounds_cause_is_specific_and_echoes_no_upstream_text() {
    use crate::response_bounds::BoundsError;
    let hostile = "<script>alert(1)</script>";
    let causes = [
        BoundsError::WrongStatus(reqwest::StatusCode::IM_A_TEAPOT),
        BoundsError::MissingContentType,
        BoundsError::WrongContentType(hostile.to_owned()),
        BoundsError::TooLarge { limit: 1024 },
        BoundsError::TooManyMembers { limit: 7 },
        BoundsError::ArrayTooLong { limit: 7 },
        BoundsError::StringTooLong { limit: 7 },
    ];
    for locale in Locale::ALL {
        for cause in &causes {
            let text = bounds_cause_text(cause, locale.strings());
            assert!(!text.is_empty(), "{cause:?} {locale:?}");
            assert!(!text.contains(hostile), "{cause:?} {locale:?}: {text}");
            assert!(!text.contains("server log"), "{cause:?} {locale:?}: {text}");
        }
    }
    let en = Locale::En.strings();
    assert!(bounds_cause_text(&causes[0], en).contains("418"));
    assert!(bounds_cause_text(&causes[3], en).contains("1024"));
}

/// Discovery's and JWKS's own `Bounds` reach the specific sentence: the
/// wiring, not only the function.
#[test]
fn discovery_and_jwks_bounds_reach_the_specific_sentence() {
    use crate::response_bounds::BoundsError;
    let en = Locale::En.strings();
    let d = discovery_cause_text(
        &DiscoveryError::Bounds(BoundsError::WrongStatus(reqwest::StatusCode::IM_A_TEAPOT)),
        en,
    );
    let j = jwks_cause_text(
        &JwksError::Bounds(BoundsError::WrongStatus(reqwest::StatusCode::IM_A_TEAPOT)),
        en,
    );
    assert!(d.contains("418"), "{d}");
    assert!(j.contains("418"), "{j}");
}

/// RFC 149 §4 / Q3 §4, test 9 (part 1): every [`DeniedPrefixClass`]
/// variant has a non-empty label in every shipped locale.
#[test]
fn every_dns_class_has_a_nonempty_label_in_every_locale() {
    use crate::resolver::DeniedPrefixClass;
    const CLASSES: [DeniedPrefixClass; 7] = [
        DeniedPrefixClass::PrivateNetwork,
        DeniedPrefixClass::Loopback,
        DeniedPrefixClass::LinkLocal,
        DeniedPrefixClass::Documentation,
        DeniedPrefixClass::BroadcastOrUnspecified,
        DeniedPrefixClass::TranslationOrTunnel,
        DeniedPrefixClass::ReservedForSpecialUse,
    ];
    for locale in Locale::ALL {
        for class in CLASSES {
            let label = dns_class_label(class, locale.strings());
            assert!(!label.is_empty(), "{locale:?}/{class:?}: empty label");
        }
    }
}

/// RFC 149 §4 / Q3 §4, test 9 (part 2): the Japanese page for a denied
/// host contains no English registry row name — only the class's own
/// localized label.
#[test]
fn the_japanese_denied_page_names_no_registry_row() {
    use crate::resolver::DeniedPrefixClass;
    let text = transport_cause_text(
        &TransportCause::DnsDenied {
            host: "evil.example".into(),
            class: DeniedPrefixClass::Loopback,
        },
        Locale::Ja.strings(),
    );
    // None of the 52 rows' own English registry-name text (an RFC
    // citation, "Global=True", the row's own English noun) ever
    // appears in the Japanese rendering — only the class's label.
    assert!(!text.contains("RFC"), "{text:?}");
    assert!(!text.contains("Loopback"), "{text:?}");
    assert!(!text.contains("Global=True"), "{text:?}");
}

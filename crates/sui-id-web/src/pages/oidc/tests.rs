//! RFC 095 M3 stage 5: the browser-presentation rule (`:269-270`) is about
//! what the server *emits*, not what a browser subsequently requests, so
//! this is a rendered-HTML assertion against `render_consent`'s own output
//! string, not a browser driver. See the stage 5 dispatch and
//! `metadata-validation.md` §*Browser presentation*.

use super::*;
use sui_id_store::models::RegistrationSource;

const LANG: sui_id_i18n::Locale = sui_id_i18n::Locale::En;

/// A host distinctive enough that a substring match on it means what it
/// says — it cannot appear in the page by any other route.
const LOGO_HOST: &str = "https://logo.rp-evidence-marker.test/logo.png";

fn consent_data(registered_via: RegistrationSource, logo_uri: Option<&str>) -> ConsentData {
    ConsentData {
        client_name: "a client".into(),
        requested_scopes: vec!["openid".into()],
        csrf_token: "csrf-token".into(),
        logo_uri: logo_uri.map(str::to_string),
        homepage_uri: Some("https://rp.test/".into()),
        registered_via,
    }
}

#[test]
fn a_dynamic_clients_logo_uri_never_appears_in_the_output() {
    let html = render_consent(
        consent_data(RegistrationSource::Dynamic, Some(LOGO_HOST)),
        LANG,
    );
    assert!(
        !html.contains(LOGO_HOST),
        "RFC 095 `:269-270`: a dynamically registered client's logo_uri \
         must never be emitted into the page, not merely left unfetched"
    );
}

#[test]
fn a_dynamic_clients_logo_wrapper_and_caption_are_absent() {
    let html = render_consent(
        consent_data(RegistrationSource::Dynamic, Some(LOGO_HOST)),
        LANG,
    );
    assert!(
        !html.contains("consent-app-logo"),
        "no logo wrapper div should render when no image will"
    );
    assert!(
        !html.contains("App logo provided by the application"),
        "the caption must not assert a logo was provided when none is shown"
    );
}

/// Without this, test 1 would pass if the logo were dropped for *every*
/// client — a different bug (the feature is simply gone) that would look
/// identical from `a_dynamic_clients_logo_uri_never_appears_in_the_output`
/// alone. This confirms the admin path is untouched.
#[test]
fn an_admin_clients_logo_uri_still_appears() {
    let html = render_consent(
        consent_data(RegistrationSource::Admin, Some(LOGO_HOST)),
        LANG,
    );
    assert!(
        html.contains(LOGO_HOST),
        "an admin-registered client's logo must still render — \
         only the dynamic path is gated"
    );
}

/// RFC 095's matrix allows a user-activated link (not an automatic
/// request) on conditions; stage 5 changes none of them, but nothing
/// asserted them before now.
#[test]
fn the_homepage_link_is_user_activated_and_leaks_no_referrer_cross_origin() {
    let html = render_consent(consent_data(RegistrationSource::Admin, None), LANG);
    assert!(
        html.contains(r#"rel="noopener noreferrer""#),
        "the homepage link must carry rel=\"noopener noreferrer\""
    );
    // AuthShell emits `same-origin`, not the matrix's literal `no-referrer`.
    // Both send no Referer cross-origin; `same-origin` is what the rest of
    // the UI uses. Do not "fix" this to match the matrix's wording.
    assert!(
        html.contains(r#"name="referrer""#) && html.contains(r#"content="same-origin""#),
        "the page must carry a same-origin referrer policy, which sends no \
         Referer on this cross-origin navigation"
    );
}

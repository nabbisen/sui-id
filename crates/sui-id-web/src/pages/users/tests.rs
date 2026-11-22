use super::*;

/// `0.93.0` O6 §4 item 4: the sessions consequence, with its link
/// to disable the account, is part of the impact area — not a
/// `muted` footnote after the form.
#[test]
fn unlink_sessions_note_joins_the_impact() {
    let html = render_confirm_unlink_federation(
        ConfirmUnlinkFederationData {
            user_id: "u1".into(),
            username: "alice".into(),
            provider_id: "p1".into(),
            provider_display_name: "Example IdP".into(),
            csrf_token: "csrf".into(),
            flash: None,
            admin_username: "admin".into(),
            methods: sui_id_store::repos::users::SignInMethods {
                password: true,
                ..Default::default()
            },
        },
        false,
        sui_id_i18n::Locale::En,
    );
    assert!(html.contains("Sessions already open"));
    assert!(html.contains("Go to this user's page"));
    // The sentence and its link sit inside a `confirm-impact`
    // paragraph together, not a `muted` footnote after the form.
    let impact_open = html.find("<p class=\"confirm-impact\">").unwrap();
    let sessions_pos = html.find("Sessions already open").unwrap();
    let link_pos = html.find("Go to this user's page").unwrap();
    let para_close = html[sessions_pos..].find("</p>").unwrap() + sessions_pos;
    assert!(impact_open < sessions_pos);
    assert!(sessions_pos < link_pos);
    assert!(link_pos < para_close);
    assert!(!html[impact_open..para_close].contains("muted"));
}

// ── `0.93.0` O7 §2/§3: the unlink page states its consequence truthfully ──

fn unlink_html(
    methods: sui_id_store::repos::users::SignInMethods,
    lang: sui_id_i18n::Locale,
) -> String {
    render_confirm_unlink_federation(
        ConfirmUnlinkFederationData {
            user_id: "u1".into(),
            username: "alice".into(),
            provider_id: "p1".into(),
            provider_display_name: "Example IdP".into(),
            csrf_token: "csrf".into(),
            flash: None,
            admin_username: "admin".into(),
            methods,
        },
        false,
        lang,
    )
}

#[test]
fn password_only_names_exactly_what_remains() {
    let html = unlink_html(
        sui_id_store::repos::users::SignInMethods {
            password: true,
            ..Default::default()
        },
        sui_id_i18n::Locale::En,
    );
    assert!(html.contains("they can still sign in with their password"));
    assert!(!html.contains("passkey"));
    assert!(!html.contains("has no other way to sign in"));
}

#[test]
fn passkey_only_names_exactly_what_remains() {
    let html = unlink_html(
        sui_id_store::repos::users::SignInMethods {
            passkeys: true,
            ..Default::default()
        },
        sui_id_i18n::Locale::En,
    );
    assert!(html.contains("they can still sign in with a passkey"));
    assert!(!html.contains("has no other way to sign in"));
}

#[test]
fn another_link_only_names_exactly_what_remains() {
    let html = unlink_html(
        sui_id_store::repos::users::SignInMethods {
            other_links: true,
            ..Default::default()
        },
        sui_id_i18n::Locale::En,
    );
    assert!(html.contains("they can still sign in with another federation provider"));
    assert!(!html.contains("has no other way to sign in"));
}

#[test]
fn directory_only_names_exactly_what_remains() {
    let html = unlink_html(
        sui_id_store::repos::users::SignInMethods {
            directory: true,
            ..Default::default()
        },
        sui_id_i18n::Locale::En,
    );
    assert!(html.contains("they can still sign in with their organization's directory"));
    assert!(!html.contains("has no other way to sign in"));
}

/// §3: none remains — the stark sentence and the link to the user's
/// page, inside `confirm-impact`; the old "their other sign-in methods
/// remain" never appears.
#[test]
fn no_methods_remain_renders_the_stark_sentence_and_the_offer() {
    let html = unlink_html(Default::default(), sui_id_i18n::Locale::En);
    assert!(html.contains("<p class=\"confirm-impact\">"));
    assert!(html.contains("alice has no other way to sign in"));
    assert!(html.contains("After this, they cannot sign in at all"));
    assert!(!html.contains("their other sign-in methods remain"));
    assert!(!html.contains("they can still sign in with"));
    // The three follow-up actions, each a link to an existing page.
    assert!(html.contains("<a href=\"/admin/users/u1/disable-confirm\">"));
    assert!(html.contains("Disable this user"));
    assert!(html.contains("<a href=\"/admin/users/u1/delete-confirm\">"));
    assert!(html.contains("Delete this user"));
    assert!(html.contains("<a href=\"/admin/users/u1/recovery-link-confirm\">"));
    assert!(html.contains("Issue a recovery link"));
}

/// A method remaining suppresses the offer entirely — nothing happens
/// automatically, and the offer is only for the no-other-way-in case.
#[test]
fn a_remaining_method_suppresses_the_offer() {
    let html = unlink_html(
        sui_id_store::repos::users::SignInMethods {
            password: true,
            ..Default::default()
        },
        sui_id_i18n::Locale::En,
    );
    assert!(!html.contains("disable-confirm"));
    assert!(!html.contains("delete-confirm"));
    assert!(!html.contains("recovery-link-confirm"));
}

/// The zero case in all three locales, with no English leftover in
/// `ja`/`zh-Hans`.
#[test]
fn no_methods_remain_has_no_english_in_other_locales() {
    for lang in [sui_id_i18n::Locale::Ja, sui_id_i18n::Locale::ZhHans] {
        let html = unlink_html(Default::default(), lang);
        assert!(
            !html.contains("has no other way to sign in")
                && !html.contains("What you can do instead")
                && !html.contains("Disable this user")
                && !html.contains("Delete this user")
                && !html.contains("Issue a recovery link"),
            "{lang:?}: {html}"
        );
    }
}

/// The recovery-link offer is made while the user is still linked, when
/// a recovery link is refused (RFC 103 T10): the page says it applies
/// after unlinking, and does not call the offers alternatives
/// ("instead") to an action one of them depends on.
#[test]
fn the_recovery_offer_says_it_applies_after_unlinking() {
    let html = unlink_html(Default::default(), sui_id_i18n::Locale::En);
    assert!(html.contains("after unlinking"), "{html}");
    assert!(html.contains(">What you can do next<"), "{html}");
    assert!(!html.contains("What you can do instead"), "{html}");
}

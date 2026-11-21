use super::*;

fn delete_confirm(linked_users_count: usize, lang: sui_id_i18n::Locale) -> String {
    render_confirm_delete_federation_provider(
        ConfirmDeleteFederationProviderData {
            provider_id: "p1".into(),
            display_name: "Example IdP".into(),
            linked_users_count,
            csrf_token: "csrf".into(),
            flash: None,
            username: "admin".into(),
        },
        false,
        lang,
    )
}

/// `0.93.0` O6 §4 item 2: the count is its own prominent element,
/// not interpolated into a sentence built in Rust.
#[test]
fn delete_provider_count_is_its_own_element() {
    let html = delete_confirm(5, sui_id_i18n::Locale::En);
    assert!(html.contains("<span class=\"confirm-impact__count\">5</span>"));
    assert!(
        html.contains("user will no longer be able to sign in through this provider")
            || html.contains("users will no longer be able to sign in through this provider")
    );
}

/// §4 item 2: zero gets its own wording, never "0 users ...".
#[test]
fn delete_provider_zero_linked_users_gets_its_own_wording() {
    let html = delete_confirm(0, sui_id_i18n::Locale::En);
    assert!(!html.contains("0 user"));
    assert!(
        html.contains("<p class=\"confirm-impact\">No user signs in through this provider.</p>")
    );
    assert!(!html.contains("<span class=\"confirm-impact__count\">"));
}

/// The count sentence comes entirely from the locale — no English
/// leftovers in `ja`/`zh-Hans`, at any count including zero.
#[test]
fn delete_provider_count_has_no_english_in_other_locales() {
    for lang in [sui_id_i18n::Locale::Ja, sui_id_i18n::Locale::ZhHans] {
        for count in [0, 1, 5] {
            let html = delete_confirm(count, lang);
            assert!(
                !html.contains("will no longer be able to sign in")
                    && !html.contains("No user signs in"),
                "{lang:?} ({count}): {html}"
            );
        }
    }
}

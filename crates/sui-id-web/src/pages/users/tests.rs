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

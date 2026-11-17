//! RFC 144 (Refresh, §4; N2c): `Shell`'s navigation is one of exactly
//! three states — `Chrome::Console { username }`, which renders the
//! console links and the account menu; `Chrome::Account { username }`,
//! which renders the account menu alone; or `Chrome::Bare`, which
//! renders no navigation at all. There is no state where a page shows
//! navigation without a name to put on it, and no state where a page
//! offers a console link to someone the server would then refuse.

use super::*;
use leptos::reactive::owner::Owner;

fn render_shell(nav: Chrome) -> String {
    let owner = Owner::new();
    owner.with(|| {
        view! {
            <Shell title="t".to_string() nav=nav current=None
                   lang=sui_id_i18n::Locale::En csrf_token="tok".to_string()>
                "body"
            </Shell>
        }
        .into_view()
        .to_html()
    })
}

#[test]
fn console_renders_the_account_menu_with_the_given_name() {
    let html = render_shell(Chrome::Console {
        username: "alice".to_string(),
    });
    assert!(
        html.contains(r#"class="user-menu""#),
        "Chrome::Console must render the account-menu widget"
    );
    assert!(
        html.contains("alice"),
        "the account menu must show the name Chrome::Console was given"
    );
}

#[test]
fn account_renders_the_account_menu_with_no_console_links() {
    let html = render_shell(Chrome::Account {
        username: "alice".to_string(),
    });
    assert!(
        html.contains(r#"class="user-menu""#),
        "Chrome::Account must render the account-menu widget"
    );
    assert!(
        html.contains("alice"),
        "the account menu must show the name Chrome::Account was given"
    );
    assert!(
        !html.contains(r#"href="/admin"#),
        "Chrome::Account must render none of the seven console links \
         (every one of them starts with /admin)"
    );
}

#[test]
fn for_person_chooses_console_for_admin_and_auditor_account_for_user() {
    use sui_id_store::models::Role;
    assert!(matches!(
        Chrome::for_person("alice".to_string(), Role::Admin),
        Chrome::Console { .. }
    ));
    assert!(matches!(
        Chrome::for_person("alice".to_string(), Role::Auditor),
        Chrome::Console { .. }
    ));
    assert!(matches!(
        Chrome::for_person("alice".to_string(), Role::User),
        Chrome::Account { .. }
    ));
}

#[test]
fn bare_renders_no_navigation_at_all() {
    let html = render_shell(Chrome::Bare);
    assert!(
        !html.contains("<nav"),
        "Chrome::Bare must render no <nav> element"
    );
    assert!(
        !html.contains(r#"class="user-menu""#),
        "Chrome::Bare must render no account menu"
    );
}

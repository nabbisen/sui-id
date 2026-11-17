//! RFC 144 (Refresh, §4): `Shell`'s navigation is one of exactly two
//! states — `Chrome::Console { username }`, which always renders the
//! account menu, or `Chrome::Bare`, which renders no navigation at
//! all. There is no third state where a page shows the console chrome
//! without a name to put on it.

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

//! RFC 144 (Refresh, §4; N2c), RFC 149 §1: `Shell`'s navigation is one of
//! exactly four states — `Chrome::Console { username, item }`, which
//! renders the sidebar with `item` marked and the account menu;
//! `Chrome::ConsolePersonal { username }`, the same sidebar on the
//! person's own pages, with no item marked; `Chrome::Account { username }`,
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
            <Shell title="t".to_string() nav=nav
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
        item: NavItem::Dashboard,
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
        Chrome::ConsolePersonal { .. }
    ));
    assert!(matches!(
        Chrome::for_person("alice".to_string(), Role::Auditor),
        Chrome::ConsolePersonal { .. }
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

/// RFC 149 §1: the console's chrome marks exactly the item its variant
/// names; on the person's own pages it shows the same sidebar with none
/// marked (they belong to the account menu, not to a group).
#[test]
fn console_marks_its_item_and_personal_pages_mark_none() {
    // The page's own markup only: the stylesheet in <head> names the
    // same attribute in its selectors.
    let marked = |html: &str| {
        let body = html.find("<body").map_or(html, |i| &html[i..]);
        body.matches(r#"aria-current="page""#).count()
    };
    let console = render_shell(Chrome::Console {
        username: "alice".to_string(),
        item: NavItem::AuditLog,
    });
    // Two responsive copies of one list: one mark in each.
    assert_eq!(marked(&console), 2, "one mark per sidebar copy");
    assert!(console.contains(r#"href="/admin/audit" aria-current="page""#));
    let personal = render_shell(Chrome::ConsolePersonal {
        username: "alice".to_string(),
    });
    assert!(
        personal.contains(r#"class="app-sidebar__nav""#),
        "the same sidebar"
    );
    assert_eq!(
        marked(&personal),
        0,
        "no item is current on a personal page"
    );
}

/// RFC 144 §4: every focusable thing shows focus. A rule may drop the
/// outline only where it draws a replacement (the inputs' accent
/// border and `box-shadow` ring); the account menu's items once
/// dropped it with nothing in its place but the hover tint.
#[test]
fn no_rule_removes_the_focus_outline_without_a_replacement() {
    let css = crate::components::components_css();
    for (at, _) in css.match_indices("outline: none") {
        let open = css[..at].rfind('{').expect("inside a rule");
        let close = at + css[at..].find('}').expect("rule closes");
        let selector_start = css[..open].rfind('}').map_or(0, |i| i + 1);
        let rule = &css[selector_start..close];
        assert!(
            rule.contains("box-shadow"),
            "a rule drops the focus outline with no replacement ring: {}",
            rule.trim()
        );
    }
}

/// RFC 149 §3: the theme selector says what it selects. Its buttons
/// (*Light*, *Auto*, *Dark*) sit in a group named *Theme*, which is
/// also shown, so *Auto* is never read alone.
#[test]
fn the_theme_selector_is_named() {
    let html = render_shell(Chrome::Account {
        username: "alice".to_string(),
    });
    let group = r#"role="group" aria-label="Theme""#;
    let at = html
        .find(group)
        .expect("the theme buttons are a named group");
    let label = html
        .find(r#"class="user-menu__theme-label""#)
        .expect("the group's name is visible");
    assert!(label < at, "the visible name comes before the buttons");
    let close = at + html[at..].find("</div>").expect("group closes");
    assert_eq!(
        html[at..close].matches("data-theme-value=").count(),
        3,
        "all three buttons are inside the named group"
    );
}

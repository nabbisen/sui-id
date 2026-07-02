//! Page renderers for the "setup" screen domain (RFC 065).

use leptos::prelude::*;

mod admin;
mod done;
mod hibp;
mod lang;
mod welcome;

pub use admin::*;
pub use done::*;
pub use hibp::*;
pub use lang::*;
pub use welcome::*;

pub(super) fn setup_step_indicator(active: usize, lang: sui_id_i18n::Locale) -> impl IntoView {
    // Five labelled dots showing which step the operator is on.
    // Steps are: Welcome(0), Admin(1), Language(2), HIBP(3), Done(4).
    let t = lang.strings();
    let labels = [
        t.setup_step_welcome,
        t.setup_step_admin,
        t.setup_step_lang,
        t.setup_step_hibp,
        t.setup_step_done,
    ];
    let dots: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active;
            let aria = if is_active { Some("step") } else { None };
            let badge = if i < active {
                view! { <span class="badge badge--ok">{format!("{}", i + 1)}</span> }.into_any()
            } else if is_active {
                view! { <span class="badge badge--accent">{format!("{}", i + 1)}</span> }.into_any()
            } else {
                view! { <span class="badge">{format!("{}", i + 1)}</span> }.into_any()
            };
            // RFC-MI-040: use CSS classes instead of inline style= attributes.
            // .setup-step__label--current / --done / --upcoming defined in
            // components/setup.rs (via StepState::label_class).
            let label_cls = if is_active {
                "setup-step__label--current"
            } else if i < active {
                "setup-step__label--done"
            } else {
                "setup-step__label--upcoming"
            };
            view! {
                <span class="row gap1-center" aria-current=aria>
                    {badge}
                    <span class=label_cls>{*label}</span>
                </span>
            }
        })
        .collect();
    view! {
        // RFC-MI-040: .setup-steps replaces the inline style= on this nav.
        <nav class="setup-steps" aria-label=t.setup_steps_aria>
            {dots}
        </nav>
    }
}

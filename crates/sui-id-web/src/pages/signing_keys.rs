//! Page renderers for the "signing_keys" screen domain (RFC 065).

use super::common::*;
use crate::components::empty_state;
use crate::layout::Shell;
use leptos::prelude::*;

fn signing_key_row_view(
    can_write: bool,
    k: sui_id_shared::api::SigningKeySummary,
    _csrf: String,
    t: &'static sui_id_i18n::Strings,
) -> impl IntoView {
    let id_str = k.id.to_string();
    let id_for_display = id_str.clone();
    let id_for_confirm = id_str.clone();
    let status_view = if k.is_active {
        crate::components::status_badge(t, crate::components::StatusKind::InUse).into_any()
    } else {
        crate::components::status_badge(t, crate::components::StatusKind::Retired).into_any()
    };
    let rotated = k
        .rotated_at
        .map(fmt_time)
        .unwrap_or_else(|| t.empty_dash.to_string());
    // RFC 030/058: dangerous action goes through confirm screen;
    // the older `let delete_url` form-action var was trimmed in the
    // RFC 065 split.
    let actions = if k.is_active {
        view! { <td><span class="muted">{t.signing_keys_in_use_badge}</span></td> }.into_any()
    } else if can_write {
        view! {
            <td>
                <a href=format!("/admin/signing-keys/{}/delete-confirm", id_for_confirm) class="button danger">{t.button_delete}</a>
            </td>
        }
        .into_any()
    } else {
        view! { <td></td> }.into_any()
    };
    view! {
        <tr>
            <td><span class="code">{id_for_display}</span></td>
            <td>{k.algorithm}</td>
            <td>{status_view}</td>
            <td class="muted">{fmt_time(k.created_at)}</td>
            <td class="muted">{rotated}</td>
            {actions}
        </tr>
    }
}

pub fn render_signing_keys(
    can_write: bool,
    keys: Vec<sui_id_shared::api::SigningKeySummary>,
    flash: Option<Flash>,
    csrf_token: String,
    _dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let key_count = keys.len();
        let rows: Vec<_> = keys
            .into_iter()
            .map(|k| signing_key_row_view(can_write, k, csrf_for_rows.clone(), t))
            .collect();
        view! {
            <Shell
                title=t.signing_keys_title.to_string()
                show_nav=true
                current=Some("signing-keys".to_string()) lang=lang
                csrf_token=csrf_token.clone()
            >
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.signing_keys_title}</h1>
                        <p class="page-header__lede">
                            {t.signing_keys_lede}
                            " "
                            {(t.signing_keys_count_caption)(key_count)}
                        </p>
                    </div>
                    // RFC 071: auditors cannot rotate keys.
                    {can_write.then(|| view! {
                        <div class="page-header__actions">
                            <a href="#rotate-key" class="button button--icon"
                               aria-label=t.signing_keys_rotate_section>
                                "↻"
                            </a>
                        </div>
                    })}
                </header>
                {flash_banner(flash)}

                <section>
                    <h2>{t.signing_keys_table_section}</h2>
                    <div class="table-wrap">
                        {if rows.is_empty() {
                            // No CTA — signing keys are issued via the rotate form below.
                            empty_state(t.empty_signing_keys, None).into_any()
                        } else {
                            view! {
                                <table>
                                    <thead>
                                        <tr>
                                            <th>{t.signing_keys_th_key_id}</th>
                                            <th>{t.signing_keys_th_algorithm}</th>
                                            <th>{t.signing_keys_th_status}</th>
                                            <th>{t.signing_keys_th_created}</th>
                                            <th>{t.signing_keys_th_retired}</th>
                                            <th></th>
                                        </tr>
                                    </thead>
                                    <tbody>{rows}</tbody>
                                </table>
                            }.into_any()
                        }}
                    </div>
                </section>

                // RFC 071: auditors cannot rotate keys.
                // Section is always visible below the table; the header ↻ button
                // scrolls to it via the #rotate-key anchor.
                {can_write.then(|| view! {
                    <section id="rotate-key">
                        <div class="card">
                            <h3 class="card__title">{t.signing_keys_rotate_section}</h3>
                            <p class="muted">
                                {t.signing_keys_rotate_explanation_1}
                                " "
                                {t.signing_keys_rotate_explanation_2}
                                " "
                                {t.signing_keys_rotate_explanation_3}
                            </p>
                            <div class="card__footer">
                                <form method="post" action="/admin/signing-keys/rotate">
                                    <input type="hidden" name="_csrf" value=csrf_for_form />
                                    <button type="submit">{t.signing_keys_rotate_button}</button>
                                </form>
                            </div>
                        </div>
                    </section>
                })}
            </Shell>
        }
    })
}

//! Page-level components and their public render entry points.
//!
//! Each `render_xxx` function constructs a Leptos view, drives it through
//! the SSR renderer, and returns a complete HTML document. The doctype is
//! prepended manually because `view!{}` only renders the tree it is given.

use crate::layout::Shell;
use chrono::{DateTime, Utc};
use leptos::prelude::*;
use leptos::reactive::owner::Owner;
use sui_id_shared::api::{AuditLogEntryDto, ClientSummary, UserSummary};

const DOCTYPE: &str = "<!DOCTYPE html>";

/// Severity of a flash banner displayed at the top of a page.
#[derive(Debug, Clone, Copy)]
pub enum FlashKind {
    Info,
    Warn,
    Error,
}

impl FlashKind {
    fn class(self) -> &'static str {
        match self {
            Self::Info => "flash info",
            Self::Warn => "flash warn",
            Self::Error => "flash error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Flash {
    pub kind: FlashKind,
    pub text: String,
}

fn flash_banner(flash: Option<Flash>) -> Option<impl IntoView> {
    flash.map(|f| view! { <div class=f.kind.class() role="status">{f.text}</div> })
}

fn fmt_time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// Run a closure inside a fresh reactive Owner and prepend the HTML doctype.
fn render<F, V>(f: F) -> String
where
    F: FnOnce() -> V,
    V: IntoView + 'static,
{
    let owner = Owner::new();
    let body = owner.with(|| f().into_view().to_html());
    let mut out = String::with_capacity(DOCTYPE.len() + body.len());
    out.push_str(DOCTYPE);
    out.push_str(&body);
    out
}

// ---------- setup ----------

pub fn render_setup(flash: Option<Flash>) -> String {
    render(move || {
        view! {
            <Shell title="Setup".to_string() show_nav=false current=None>
                <h2>"Welcome to sui-id."</h2>
                <p class="muted">
                    "This server has not been initialized yet. Create the first administrator below. "
                    "The setup token was printed once on this server's standard error at startup; \
                     paste it here to confirm you control the host."
                </p>
                {flash_banner(flash)}
                <form method="post" action="/setup">
                    <label for="token">"Setup token"</label>
                    <input id="token" name="setup_token" type="password" required=true autocomplete="off" />

                    <label for="username">"Administrator username"</label>
                    <input id="username" name="username" type="text" required=true autocomplete="username" />

                    <label for="display">"Display name (optional)"</label>
                    <input id="display" name="display_name" type="text" autocomplete="name" />

                    <label for="password">"Password (12 characters or more)"</label>
                    <input id="password" name="password" type="password" required=true minlength="12" autocomplete="new-password" />

                    <button type="submit">"Create administrator"</button>
                </form>
            </Shell>
        }
    })
}

// ---------- login ----------

pub fn render_login(flash: Option<Flash>, next: Option<String>) -> String {
    render(move || {
        let next_value = next.clone().unwrap_or_default();
        view! {
            <Shell title="Sign in".to_string() show_nav=false current=None>
                <h2>"Sign in"</h2>
                {flash_banner(flash)}
                <form method="post" action="/admin/login">
                    <input type="hidden" name="next" value=next_value />
                    <label for="username">"Username"</label>
                    <input id="username" name="username" type="text" required=true autocomplete="username" />
                    <label for="password">"Password"</label>
                    <input id="password" name="password" type="password" required=true autocomplete="current-password" />
                    <button type="submit">"Sign in"</button>
                </form>
            </Shell>
        }
    })
}

// ---------- dashboard ----------

pub struct DashboardData {
    pub admin_username: String,
    pub user_count: usize,
    pub client_count: usize,
    pub issuer: String,
}

pub fn render_dashboard(data: DashboardData, flash: Option<Flash>) -> String {
    render(move || {
        let DashboardData { admin_username, user_count, client_count, issuer } = data;
        view! {
            <Shell title="Dashboard".to_string() show_nav=true current=Some("dashboard".to_string())>
                <h2>{format!("Hello, {admin_username}.")}</h2>
                {flash_banner(flash)}
                <p class="muted">"sui-id is running. Service overview below."</p>
                <table>
                    <tbody>
                        <tr><th>"Issuer"</th><td><span class="code">{issuer}</span></td></tr>
                        <tr><th>"Users"</th><td>{user_count.to_string()}</td></tr>
                        <tr><th>"Clients"</th><td>{client_count.to_string()}</td></tr>
                        <tr><th>"OIDC Discovery"</th><td><a href="/.well-known/openid-configuration">"/.well-known/openid-configuration"</a></td></tr>
                        <tr><th>"JWKS"</th><td><a href="/.well-known/jwks.json">"/.well-known/jwks.json"</a></td></tr>
                    </tbody>
                </table>
            </Shell>
        }
    })
}

// ---------- users ----------

fn user_row_view(u: UserSummary, current_user: String) -> impl IntoView {
    let status = if u.is_deleted {
        "deleted"
    } else if u.is_disabled {
        "disabled"
    } else if u.is_admin {
        "admin"
    } else {
        "active"
    };
    let display = u.display_name.clone().unwrap_or_default();
    let id_str = u.id.to_string();
    let is_self = u.username == current_user;
    let is_disabled = u.is_disabled;
    let is_deleted = u.is_deleted;
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    let action_target = if is_disabled { "false" } else { "true" };
    let disabled_url = format!("/admin/users/{id_str}/disabled");
    let delete_url = format!("/admin/users/{id_str}/delete");

    let actions = if is_self {
        view! { <td class="muted">"(you)"</td> }.into_any()
    } else if is_deleted {
        view! { <td class="muted">"-"</td> }.into_any()
    } else {
        view! {
            <td>
                <form method="post" action=disabled_url style="display:inline">
                    <input type="hidden" name="disabled" value=action_target />
                    <button type="submit" class="secondary">{action_label}</button>
                </form>
                " "
                <form method="post" action=delete_url style="display:inline"
                      onsubmit="return confirm('Permanently delete this user?');">
                    <button type="submit" class="danger">"Delete"</button>
                </form>
            </td>
        }
        .into_any()
    };

    view! {
        <tr>
            <td><span class="code">{u.username}</span></td>
            <td>{display}</td>
            <td>{status}</td>
            <td>{fmt_time(u.created_at)}</td>
            {actions}
        </tr>
    }
}

pub fn render_users(users: Vec<UserSummary>, flash: Option<Flash>, current_user: String) -> String {
    render(move || {
        let rows: Vec<_> = users
            .into_iter()
            .map(|u| user_row_view(u, current_user.clone()))
            .collect();
        view! {
            <Shell title="Users".to_string() show_nav=true current=Some("users".to_string())>
                <h2>"Users"</h2>
                {flash_banner(flash)}
                <h3>"Add a user"</h3>
                <form method="post" action="/admin/users">
                    <label for="u-name">"Username"</label>
                    <input id="u-name" name="username" type="text" required=true autocomplete="off" />
                    <label for="u-disp">"Display name (optional)"</label>
                    <input id="u-disp" name="display_name" type="text" autocomplete="off" />
                    <label for="u-pw">"Password (12 chars or more)"</label>
                    <input id="u-pw" name="password" type="password" required=true minlength="12" autocomplete="new-password" />
                    <label>
                        <input name="is_admin" type="checkbox" value="true" />
                        " Grant administrator privileges"
                    </label>
                    <button type="submit">"Create user"</button>
                </form>

                <h3>"All users"</h3>
                <table>
                    <thead>
                        <tr><th>"Username"</th><th>"Display"</th><th>"Status"</th><th>"Created"</th><th></th></tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </Shell>
        }
    })
}

// ---------- clients ----------

fn client_row_view(c: ClientSummary) -> impl IntoView {
    let status = if c.is_deleted {
        "deleted"
    } else if c.is_disabled {
        "disabled"
    } else {
        "active"
    };
    let kind = if c.confidential { "confidential" } else { "public" };
    let id_str = c.id.to_string();
    let is_disabled = c.is_disabled;
    let is_deleted = c.is_deleted;
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    let action_target = if is_disabled { "false" } else { "true" };
    let disabled_url = format!("/admin/clients/{id_str}/disabled");
    let delete_url = format!("/admin/clients/{id_str}/delete");

    let actions = if is_deleted {
        view! { <td class="muted">"-"</td> }.into_any()
    } else {
        view! {
            <td>
                <form method="post" action=disabled_url style="display:inline">
                    <input type="hidden" name="disabled" value=action_target />
                    <button type="submit" class="secondary">{action_label}</button>
                </form>
                " "
                <form method="post" action=delete_url style="display:inline"
                      onsubmit="return confirm('Permanently delete this client and revoke its tokens?');">
                    <button type="submit" class="danger">"Delete"</button>
                </form>
            </td>
        }
        .into_any()
    };

    view! {
        <tr>
            <td>{c.name}</td>
            <td><span class="code">{c.id.to_string()}</span></td>
            <td>{kind}</td>
            <td>{status}</td>
            {actions}
        </tr>
    }
}

pub fn render_clients(
    clients: Vec<ClientSummary>,
    flash: Option<Flash>,
    new_secret: Option<(String, String)>,
) -> String {
    render(move || {
        let secret_block = new_secret.map(|(cid, sec)| {
            view! {
                <div class="flash warn" role="status">
                    <strong>"Save this client secret now - it will not be shown again."</strong>
                    <div>"Client id: "<span class="code">{cid}</span></div>
                    <div>"Client secret: "<span class="code">{sec}</span></div>
                </div>
            }
        });
        let rows: Vec<_> = clients.into_iter().map(client_row_view).collect();
        view! {
            <Shell title="Clients".to_string() show_nav=true current=Some("clients".to_string())>
                <h2>"Clients"</h2>
                {flash_banner(flash)}
                {secret_block}
                <h3>"Register a client"</h3>
                <form method="post" action="/admin/clients">
                    <label for="c-name">"Application name"</label>
                    <input id="c-name" name="name" type="text" required=true />
                    <label for="c-uris">"Redirect URIs (one per line; https or http loopback)"</label>
                    <textarea id="c-uris" name="redirect_uris" required=true rows="3"></textarea>
                    <label>
                        <input name="confidential" type="checkbox" value="true" checked=true />
                        " Confidential client (will receive a client secret)"
                    </label>
                    <button type="submit">"Register"</button>
                </form>

                <h3>"Registered clients"</h3>
                <table>
                    <thead>
                        <tr><th>"Name"</th><th>"Client id"</th><th>"Type"</th><th>"Status"</th><th></th></tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </Shell>
        }
    })
}

// ---------- audit ----------

fn audit_row_view(e: AuditLogEntryDto) -> impl IntoView {
    view! {
        <tr>
            <td>{fmt_time(e.at)}</td>
            <td><span class="code">{e.actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into())}</span></td>
            <td>{e.action}</td>
            <td><span class="code">{e.target.unwrap_or_default()}</span></td>
            <td>{e.result}</td>
        </tr>
    }
}

pub fn render_audit(entries: Vec<AuditLogEntryDto>, flash: Option<Flash>) -> String {
    render(move || {
        let rows: Vec<_> = entries.into_iter().map(audit_row_view).collect();
        view! {
            <Shell title="Audit".to_string() show_nav=true current=Some("audit".to_string())>
                <h2>"Audit log"</h2>
                {flash_banner(flash)}
                <p class="muted">"Most recent administrative actions, newest first."</p>
                <table>
                    <thead>
                        <tr><th>"When"</th><th>"Actor"</th><th>"Action"</th><th>"Target"</th><th>"Result"</th></tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </Shell>
        }
    })
}

// ---------- signing keys ----------

fn signing_key_row_view(k: sui_id_shared::api::SigningKeySummary) -> impl IntoView {
    let id_str = k.id.to_string();
    let id_for_url = id_str.clone();
    let id_for_display = id_str.clone();
    let status = if k.is_active { "active" } else { "retired" };
    let rotated = k
        .rotated_at
        .map(fmt_time)
        .unwrap_or_else(|| "-".to_string());
    let delete_url = format!("/admin/signing-keys/{id_for_url}/delete");
    let actions = if k.is_active {
        view! { <td class="muted">"(in use)"</td> }.into_any()
    } else {
        view! {
            <td>
                <form method="post" action=delete_url style="display:inline"
                      onsubmit="return confirm('Permanently delete this retired key? Tokens still in flight that were signed with it will fail to verify.');">
                    <button type="submit" class="danger">"Delete"</button>
                </form>
            </td>
        }
        .into_any()
    };
    view! {
        <tr>
            <td><span class="code">{id_for_display}</span></td>
            <td>{k.algorithm}</td>
            <td>{status}</td>
            <td>{fmt_time(k.created_at)}</td>
            <td>{rotated}</td>
            {actions}
        </tr>
    }
}

pub fn render_signing_keys(
    keys: Vec<sui_id_shared::api::SigningKeySummary>,
    flash: Option<Flash>,
) -> String {
    render(move || {
        let rows: Vec<_> = keys.into_iter().map(signing_key_row_view).collect();
        view! {
            <Shell
                title="Signing keys".to_string()
                show_nav=true
                current=Some("signing-keys".to_string())
            >
                <h2>"Signing keys"</h2>
                {flash_banner(flash)}
                <p class="muted">
                    "sui-id signs JWTs with one active Ed25519 key. Rotating publishes a fresh key as the new \
                     signing key, demotes the previous one to retired status, and keeps it in JWKS so that \
                     tokens already issued can still be verified during their remaining lifetime. Once those \
                     tokens have expired, you can safely delete the retired key from this page."
                </p>
                <form method="post" action="/admin/signing-keys/rotate">
                    <button type="submit">"Rotate signing key"</button>
                </form>

                <h3>"All keys"</h3>
                <table>
                    <thead>
                        <tr>
                            <th>"Key id"</th>
                            <th>"Algorithm"</th>
                            <th>"Status"</th>
                            <th>"Created"</th>
                            <th>"Retired"</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </Shell>
        }
    })
}

// ---------- error ----------

pub fn render_error(title: String, message: String, request_id: String) -> String {
    render(move || {
        let title2 = title.clone();
        view! {
            <Shell title=title.clone() show_nav=false current=None>
                <h2>{title2}</h2>
                <div class="flash error" role="alert">{message}</div>
                <p class="muted">
                    "If you contact your administrator, please mention this id: "
                    <span class="code">{request_id}</span>
                </p>
                <p><a href="/" class="button secondary">"Back to start"</a></p>
            </Shell>
        }
    })
}

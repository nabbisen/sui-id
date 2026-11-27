//! RFC 149 Q4 §2: parsing the app form's raw POST body. Repeated
//! fields (`redirect_uri_0`, `redirect_uri_1`, ...) and "which button
//! was pressed" are both things axum's typed `Form<T>` extractor
//! cannot express — `url::form_urlencoded::parse` (already in the
//! tree, via the `url` crate axum itself depends on) reads the raw
//! bytes instead. No new crate enters `Cargo.lock`.
//!
//! §5 test 1 ("Add another"/"Remove" save nothing and record nothing)
//! is why this file separates *parsing* from *saving* at all: the
//! handler decides whether to call a command only after it knows
//! which button was pressed.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormAction {
    #[default]
    Save,
    AddRedirectUri,
    RemoveRedirectUri(usize),
    AddPostLogoutUri,
    RemovePostLogoutUri(usize),
}

#[derive(Debug, Default)]
pub struct ParsedAppForm {
    pub csrf: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub post_logout_uris: Vec<String>,
    /// Every scope `openid` doesn't already cover, in
    /// `sui_id_core::discovery::STANDARD_SCOPES`'s own order --
    /// [`parse`] seeds this from that list, so a `scope_<name>`
    /// field is only ever recognized for a name discovery itself
    /// advertises (RFC 149 Q4 §1).
    pub standard_scopes_checked: Vec<(&'static str, bool)>,
    pub additional_scopes: String,
    pub consent_policy: String,
    pub homepage_uri: String,
    pub privacy_policy_uri: String,
    pub tos_uri: String,
    /// `Some(_)` only if the field was present at all — the create
    /// form's own checkbox; absent on the edit form, which never
    /// submits it (kind is immutable after creation).
    pub confidential: Option<String>,
    pub action: FormAction,
}

/// Parse the raw POST body. `redirect_uri_<n>`/`post_logout_uri_<n>`
/// are collected by their numeric suffix into a `BTreeMap` first, so
/// a gap (from an earlier removal) or an out-of-order submission
/// still reconstructs in the right order — never assumed contiguous.
pub fn parse(body: &[u8]) -> ParsedAppForm {
    let mut redirect_uris: BTreeMap<usize, String> = BTreeMap::new();
    let mut post_logout_uris: BTreeMap<usize, String> = BTreeMap::new();
    let mut out = ParsedAppForm {
        standard_scopes_checked: sui_id_core::discovery::STANDARD_SCOPES
            .iter()
            .skip(1) // openid: never a submitted choice
            .map(|&s| (s, false))
            .collect(),
        ..Default::default()
    };
    for (k, v) in url::form_urlencoded::parse(body) {
        match k.as_ref() {
            "_csrf" => out.csrf = v.into_owned(),
            "name" => out.name = v.into_owned(),
            "additional_scopes" => out.additional_scopes = v.into_owned(),
            "consent_policy" => out.consent_policy = v.into_owned(),
            "homepage_uri" => out.homepage_uri = v.into_owned(),
            "privacy_policy_uri" => out.privacy_policy_uri = v.into_owned(),
            "tos_uri" => out.tos_uri = v.into_owned(),
            "confidential" => out.confidential = Some(v.into_owned()),
            other if other.starts_with("scope_") => {
                let name = &other["scope_".len()..];
                if let Some(entry) = out
                    .standard_scopes_checked
                    .iter_mut()
                    .find(|(n, _)| *n == name)
                {
                    entry.1 = true;
                }
            }
            "add_redirect_uri" => out.action = FormAction::AddRedirectUri,
            "add_post_logout_uri" => out.action = FormAction::AddPostLogoutUri,
            "remove_redirect_uri" => {
                if let Ok(i) = v.parse() {
                    out.action = FormAction::RemoveRedirectUri(i);
                }
            }
            "remove_post_logout_uri" => {
                if let Ok(i) = v.parse() {
                    out.action = FormAction::RemovePostLogoutUri(i);
                }
            }
            other if other.starts_with("redirect_uri_") => {
                if let Ok(i) = other["redirect_uri_".len()..].parse::<usize>() {
                    redirect_uris.insert(i, v.into_owned());
                }
            }
            other if other.starts_with("post_logout_uri_") => {
                if let Ok(i) = other["post_logout_uri_".len()..].parse::<usize>() {
                    post_logout_uris.insert(i, v.into_owned());
                }
            }
            _ => {}
        }
    }
    out.redirect_uris = redirect_uris.into_values().collect();
    out.post_logout_uris = post_logout_uris.into_values().collect();
    if out.redirect_uris.is_empty() {
        out.redirect_uris.push(String::new());
    }
    out
}

/// Apply the parsed action to the two URI lists in place. A removal
/// that would empty the redirect-URI list is a no-op instead — there
/// is always at least one row to show, the same floor the command
/// layer enforces at save time; the post-logout list has no such
/// floor (it is optional, so an empty list is a valid end state).
pub fn apply_uri_action(
    action: FormAction,
    redirect_uris: &mut Vec<String>,
    post_logout_uris: &mut Vec<String>,
) {
    match action {
        FormAction::AddRedirectUri => redirect_uris.push(String::new()),
        FormAction::RemoveRedirectUri(i) if redirect_uris.len() > 1 && i < redirect_uris.len() => {
            redirect_uris.remove(i);
        }
        FormAction::AddPostLogoutUri => post_logout_uris.push(String::new()),
        FormAction::RemovePostLogoutUri(i) if i < post_logout_uris.len() => {
            post_logout_uris.remove(i);
        }
        FormAction::Save
        | FormAction::RemoveRedirectUri(_)
        | FormAction::RemovePostLogoutUri(_) => {}
    }
}

// ---------- validation (RFC 149 Q4 §1, §2) ----------

/// Everything a save needs, once every field has passed its own
/// check. `allowed_scopes` carries the checked standard scopes plus
/// the validated additional ones — `openid` is not included here;
/// `create_client`/`update_client_form` insert it unconditionally
/// (RFC 149 Q4 §1), so there is exactly one place that decides it,
/// not two.
pub struct ValidatedAppForm {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub post_logout_uris: Vec<String>,
    pub allowed_scopes: String,
    pub consent_policy: sui_id_store::models::ConsentPolicy,
    pub homepage_uri: Option<String>,
    pub privacy_policy_uri: Option<String>,
    pub tos_uri: Option<String>,
}

/// One error per field, by position for the two URI lists — RFC 149
/// Q4 §2: "each URI is checked on its own... all the URIs' errors
/// are shown at once". `None` everywhere means nothing to refuse.
#[derive(Default)]
pub struct AppFormErrors {
    pub name: Option<String>,
    pub redirect_uris: Vec<Option<String>>,
    pub post_logout_uris: Vec<Option<String>>,
    pub additional_scopes: Option<String>,
    pub homepage_uri: Option<String>,
    pub privacy_policy_uri: Option<String>,
    pub tos_uri: Option<String>,
}

impl AppFormErrors {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.redirect_uris.iter().all(Option::is_none)
            && self.post_logout_uris.iter().all(Option::is_none)
            && self.additional_scopes.is_none()
            && self.homepage_uri.is_none()
            && self.privacy_policy_uri.is_none()
            && self.tos_uri.is_none()
    }
}

pub fn redirect_uri_error_text(
    e: sui_id_core::admin::clients::RedirectUriError,
    t: &'static sui_id_i18n::Strings,
) -> String {
    use sui_id_core::admin::clients::RedirectUriError;
    match e {
        RedirectUriError::NotAUrl => t.clients_error_redirect_uri_not_url.to_owned(),
        RedirectUriError::SchemeNotAllowed => t.clients_error_redirect_uri_scheme.to_owned(),
        RedirectUriError::HasFragment => t.clients_error_redirect_uri_fragment.to_owned(),
    }
}

pub fn additional_scope_error_text(
    e: sui_id_core::admin::clients::AdditionalScopeError,
    t: &'static sui_id_i18n::Strings,
) -> String {
    use sui_id_core::admin::clients::AdditionalScopeError;
    match e {
        AdditionalScopeError::InvalidSyntax(tok) => (t.clients_error_additional_scope_syntax)(&tok),
        AdditionalScopeError::IsStandardScope(s) => {
            (t.clients_error_additional_scope_is_standard)(s)
        }
        AdditionalScopeError::NearMiss { typed, standard } => {
            (t.clients_error_additional_scope_near_miss)(&typed, standard)
        }
    }
}

/// Validate every field, never stopping at the first failure — each
/// URI gets its own entry in the returned errors, in position, and
/// every other field is checked too, so a save attempt shows every
/// problem at once.
pub fn validate(
    parsed: &ParsedAppForm,
    t: &'static sui_id_i18n::Strings,
) -> Result<ValidatedAppForm, Box<AppFormErrors>> {
    use sui_id_core::admin::clients as core_clients;
    let mut errors = AppFormErrors::default();

    let name = parsed.name.trim().to_owned();
    if name.is_empty() {
        errors.name = Some(t.clients_error_name_required.to_owned());
    }

    errors.redirect_uris = core_clients::check_redirect_uris(&parsed.redirect_uris)
        .into_iter()
        .map(|r| r.err().map(|e| redirect_uri_error_text(e, t)))
        .collect();
    errors.post_logout_uris = core_clients::check_redirect_uris(&parsed.post_logout_uris)
        .into_iter()
        .map(|r| r.err().map(|e| redirect_uri_error_text(e, t)))
        .collect();

    let additional_scopes =
        match core_clients::validate_additional_scopes(&parsed.additional_scopes) {
            Ok(v) => v,
            Err(e) => {
                errors.additional_scopes = Some(additional_scope_error_text(e, t));
                Vec::new()
            }
        };

    fn app_uri(raw: &str, field: &'static str, error: &mut Option<String>) -> Option<String> {
        match crate::dynamic_registration_validation::validate_application_uri(Some(raw), field) {
            Ok(v) => v,
            Err(e) => {
                *error = Some(e.reason.to_owned());
                None
            }
        }
    }
    let homepage_uri = app_uri(
        &parsed.homepage_uri,
        "homepage_uri",
        &mut errors.homepage_uri,
    );
    let privacy_policy_uri = app_uri(
        &parsed.privacy_policy_uri,
        "privacy_policy_uri",
        &mut errors.privacy_policy_uri,
    );
    let tos_uri = app_uri(&parsed.tos_uri, "tos_uri", &mut errors.tos_uri);

    if !errors.is_empty() {
        return Err(Box::new(errors));
    }

    // Never decided by the form: `openid` is always first, whether
    // or not the submission included it (RFC 149 Q4 §1, test 4) --
    // `create_client`/`update_client_form` would add it anyway for a
    // non-empty value, but putting it here too means this string is
    // never empty, so their own "empty means permit any" legacy path
    // (unrelated to this stage) is never reached from this form.
    let mut scope_parts: Vec<String> = vec!["openid".to_owned()];
    for (name, checked) in &parsed.standard_scopes_checked {
        if *checked {
            scope_parts.push((*name).to_owned());
        }
    }
    scope_parts.extend(additional_scopes);

    Ok(ValidatedAppForm {
        name,
        redirect_uris: parsed.redirect_uris.clone(),
        post_logout_uris: parsed.post_logout_uris.clone(),
        allowed_scopes: scope_parts.join(" "),
        consent_policy: sui_id_store::models::ConsentPolicy::parse(parsed.consent_policy.trim()),
        homepage_uri,
        privacy_policy_uri,
        tos_uri,
    })
}

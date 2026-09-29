//! RFC 120 D6 — every route reachable without an authenticated actor is
//! enumerated, and the list is checked.
//!
//! Two of the routes this RFC fixed escaped because nothing listed the routes
//! that answer without a session. This test derives that list from the router
//! itself and compares it with [`EXPECTED_WITHOUT_ACTOR`], so **adding a route
//! that does not require an actor changes the expected list, which is a
//! reviewed diff** and not an accident.
//!
//! How the list is derived, with no hand-kept route table:
//!
//! 1. Every `.route("<path>", <verb>(<handler>)...)` in `src/http/router.rs` is
//!    read (comments removed). The count of `.route(` occurrences must equal the
//!    count parsed, so a form this parser does not understand fails the test
//!    instead of vanishing from the list.
//! 2. Each handler is found in `src/http/` and its **signature** is read. A
//!    handler requires an actor when its signature takes one of the
//!    authenticated-actor extractors ([`ACTOR_EXTRACTORS`]), which resolve the
//!    session through `session::resolve`.
//! 3. The routes whose handlers do not are the set compared below.
//!
//! And one behavioural check so the classification is not taken on trust: every
//! route classified as requiring an actor is sent an anonymous request and must
//! be turned away (the login redirect, 401 or 403), never answered.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

/// The extractors that resolve a live session to an actor.
const ACTOR_EXTRACTORS: [&str; 5] = [
    "CurrentUser",
    "SessionContext",
    "CurrentAdmin",
    "CurrentAdminOrAuditor",
    "CurrentAdminJson",
];

const VERBS: [&str; 5] = ["get", "post", "put", "delete", "patch"];

/// Every route whose handler does not take an actor extractor, with why that is
/// right. A route is here because it is public by nature, because it
/// authenticates something other than a browser session (a client, a bearer
/// token, a one-time token, a pending sign-in), or because it does its own
/// session check inline. **Read the reason before adding to this list.**
const EXPECTED_WITHOUT_ACTOR: &[(&str, &str, &str)] = &[
    // ---- public by nature
    ("GET", "/", "landing; redirects by initialization state"),
    ("GET", "/healthz", "liveness probe; leaks nothing (RFC 016)"),
    ("GET", "/.well-known/openid-configuration", "OIDC discovery"),
    ("GET", "/.well-known/jwks.json", "public signing keys"),
    ("GET", "/static/{*path}", "embedded static assets"),
    // ---- first-run setup; the admin POST is gated by the setup token
    ("GET", "/setup", "welcome; redirects once initialized"),
    ("GET", "/setup/admin", "form; redirects once initialized"),
    (
        "POST",
        "/setup/admin",
        "gated by the setup token, and refused once initialized",
    ),
    ("GET", "/setup/done", "informational only"),
    // ---- OAuth / OIDC protocol endpoints: they authenticate a client, a
    // bearer token or the user's own session inline, not through an extractor
    (
        "GET",
        "/oauth2/authorize",
        "resolves the session inline and sends an anonymous user to sign in",
    ),
    (
        "GET",
        "/oauth2/logout",
        "RP-initiated logout: an id_token_hint or the session cookie, inline",
    ),
    (
        "POST",
        "/oauth2/token",
        "client authentication and a one-time code or refresh token",
    ),
    (
        "POST",
        "/oauth2/register",
        "RFC 7591: an initial-access token",
    ),
    ("POST", "/oauth2/introspect", "client authentication"),
    ("POST", "/oauth2/revoke", "client authentication"),
    ("GET", "/oauth2/userinfo", "bearer access token"),
    ("POST", "/oauth2/userinfo", "bearer access token"),
    // ---- sign-in and recovery: the caller is by definition not signed in, and
    // two constant redirects that take no input and read nothing
    ("GET", "/admin/login", "sign-in form"),
    (
        "POST",
        "/admin/login",
        "credentials; lockout and rate limit",
    ),
    (
        "GET",
        "/admin/login/mfa",
        "second factor; a pending-sign-in cookie",
    ),
    (
        "POST",
        "/admin/login/mfa",
        "second factor; a pending-sign-in cookie",
    ),
    (
        "POST",
        "/admin/login/webauthn/start",
        "passkey sign-in; a pending ceremony",
    ),
    (
        "POST",
        "/admin/login/webauthn/complete",
        "passkey sign-in; a pending ceremony",
    ),
    (
        "POST",
        "/admin/logout",
        "ends the caller's own session; CSRF",
    ),
    (
        "GET",
        "/admin/profile",
        "permanent redirect to /me/security",
    ),
    (
        "GET",
        "/admin/settings",
        "constant redirect to the first settings tab; reads nothing",
    ),
    (
        "GET",
        "/me/security",
        "constant redirect to the overview; reads nothing",
    ),
    ("GET", "/forgot-password", "recovery form"),
    ("POST", "/forgot-password", "uniform response; rate limit"),
    ("GET", "/reset-password", "a one-time reset token"),
    ("POST", "/reset-password", "a one-time reset token"),
    (
        "GET",
        "/auth/federated/{slug}/start",
        "upstream sign-in start",
    ),
    (
        "GET",
        "/auth/federated/callback",
        "upstream sign-in callback; a signed state cookie",
    ),
    ("GET", "/auth/federated/link", "link-only sign-in"),
    // ---- optional
    (
        "GET",
        "/metrics",
        "a bearer token or an administrator session, inline; mounted only when enabled",
    ),
];

// ---------- reading the router ----------

fn src(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel)
}

/// `text` with `//` comments removed (outside string literals).
fn strip_line_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut in_str = false;
        let mut prev = '\0';
        let mut cut = line.len();
        for (i, c) in line.char_indices() {
            if c == '"' && prev != '\\' {
                in_str = !in_str;
            }
            if !in_str && c == '/' && prev == '/' {
                cut = i - 1;
                break;
            }
            prev = c;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// The text between the `(` at `open` and its matching `)`, and the index after
/// that `)`. String literals are skipped.
fn balanced(text: &str, open: usize) -> (&str, usize) {
    let bytes = text.as_bytes();
    assert_eq!(bytes[open], b'(');
    let mut depth = 0usize;
    let mut in_str = false;
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return (&text[open + 1..i], i + 1);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    panic!("unbalanced parentheses in the router source");
}

/// `(method, path, handler)` for every verb of every `.route(` in the router.
fn routes() -> Vec<(String, String, String)> {
    let raw = std::fs::read_to_string(src("http/router.rs")).expect("read router.rs");
    let text = strip_line_comments(&raw);
    let mut found = Vec::new();
    let mut parsed_routes = 0usize;
    let mut at = 0usize;
    while let Some(i) = text[at..].find(".route(") {
        let open = at + i + ".route".len();
        let (args, next) = balanced(&text, open);
        at = next;
        parsed_routes += 1;
        let args = args.trim_start();
        assert!(
            args.starts_with('"'),
            "route path must be a string literal: {args:.60}"
        );
        let end = 1 + args[1..].find('"').expect("closing quote");
        let path = args[1..end].to_owned();
        let expr = &args[end + 1..];
        let mut verbs = 0usize;
        for verb in VERBS {
            let pat = format!("{verb}(");
            let mut from = 0usize;
            while let Some(j) = expr[from..].find(&pat) {
                let pos = from + j;
                from = pos + pat.len();
                let before = expr[..pos].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let (handler, _) = balanced(expr, pos + verb.len());
                found.push((verb.to_uppercase(), path.clone(), handler.trim().to_owned()));
                verbs += 1;
            }
        }
        assert!(verbs > 0, "no method found for route {path}");
    }
    let occurrences = text.matches(".route(").count();
    assert_eq!(
        parsed_routes, occurrences,
        "every `.route(` must be parsed exactly once"
    );
    assert!(
        parsed_routes > 50,
        "the router parse found {parsed_routes} routes"
    );
    found
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("read dir") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The parameter list of the handler named by `path` (`admin::login_get`,
/// `crate::handlers::step_up::get`, ...).
fn signature(handler_path: &str, files: &BTreeMap<PathBuf, String>) -> String {
    let segs: Vec<&str> = handler_path.split("::").collect();
    let name = *segs.last().expect("handler name");
    let hint = if segs.len() >= 2 {
        Some(segs[segs.len() - 2])
    } else {
        None
    };
    let needle = format!("pub async fn {name}(");
    let mut hits: Vec<(&PathBuf, usize)> = Vec::new();
    for (p, text) in files {
        let mut from = 0usize;
        while let Some(j) = text[from..].find(&needle) {
            let pos = from + j;
            from = pos + needle.len();
            // `fn get(` must not match inside `fn forget(`.
            hits.push((p, pos));
        }
    }
    if hits.len() > 1 {
        let hint = hint.unwrap_or("");
        hits.retain(|(p, _)| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let parent = p
                .parent()
                .and_then(|d| d.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("");
            stem == hint || parent == hint
        });
    }
    assert_eq!(
        hits.len(),
        1,
        "handler `{handler_path}` must resolve to exactly one function, found {hits:?}"
    );
    let (p, pos) = hits[0];
    let text = &files[p];
    let (params, _) = balanced(text, pos + needle.len() - 1);
    params.to_owned()
}

fn requires_actor(params: &str) -> bool {
    ACTOR_EXTRACTORS.iter().any(|e| params.contains(e))
}

/// `(method, path)` pairs.
type RouteSet = BTreeSet<(String, String)>;

fn classified() -> (RouteSet, RouteSet) {
    let mut paths = Vec::new();
    rust_files(&src("http"), &mut paths);
    let files: BTreeMap<PathBuf, String> = paths
        .into_iter()
        .map(|p| {
            let t = std::fs::read_to_string(&p).expect("read source");
            (p, strip_line_comments(&t))
        })
        .collect();
    let mut with = BTreeSet::new();
    let mut without = BTreeSet::new();
    for (method, path, handler) in routes() {
        let params = signature(&handler, &files);
        let key = (method, path);
        if requires_actor(&params) {
            with.insert(key);
        } else {
            without.insert(key);
        }
    }
    (with, without)
}

// ---------- the tests ----------

#[test]
fn the_routes_that_answer_without_an_actor_are_exactly_the_expected_set() {
    let (_, without) = classified();
    let expected: BTreeSet<(String, String)> = EXPECTED_WITHOUT_ACTOR
        .iter()
        .map(|(m, p, _)| ((*m).to_owned(), (*p).to_owned()))
        .collect();
    let now_public: Vec<_> = without.difference(&expected).collect();
    let now_guarded: Vec<_> = expected.difference(&without).collect();
    assert!(
        now_public.is_empty(),
        "these routes answer without an authenticated actor and are not in \
         EXPECTED_WITHOUT_ACTOR (a route that acts for a user, or changes state, \
         must take an actor extractor; a genuinely public one needs a reviewed \
         entry with its reason): {now_public:#?}"
    );
    assert!(
        now_guarded.is_empty(),
        "these expected-public routes now require an actor or no longer exist; \
         remove them from EXPECTED_WITHOUT_ACTOR: {now_guarded:#?}"
    );
}

#[test]
fn every_expected_route_has_a_reason() {
    for (m, p, why) in EXPECTED_WITHOUT_ACTOR {
        assert!(why.len() > 8, "{m} {p} needs a reason");
    }
}

fn fill(path: &str) -> String {
    let id = "00000000-0000-4000-8000-000000000001";
    let mut out = String::new();
    let mut rest = path;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let j = rest[i..].find('}').expect("closing brace") + i;
        let name = &rest[i + 1..j];
        out.push_str(if name.starts_with('*') || name == "slug" {
            "x"
        } else {
            id
        });
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

#[tokio::test]
async fn every_route_classified_as_requiring_an_actor_turns_an_anonymous_caller_away() {
    let (with, _) = classified();
    assert!(with.len() > 40, "only {} guarded routes found", with.len());
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let mut failures = Vec::new();
    for (method, path) in with {
        let m = Method::from_bytes(method.as_bytes()).expect("method");
        let mut req = Request::builder().method(m.clone()).uri(fill(&path));
        if m == Method::POST {
            req = req.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        }
        let resp = build_router(state.clone())
            .oneshot(req.body(Body::empty()).expect("req"))
            .await
            .expect("anonymous request");
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let turned_away = (resp.status().is_redirection() && loc == "/admin/login")
            || resp.status() == StatusCode::UNAUTHORIZED
            || resp.status() == StatusCode::FORBIDDEN;
        if !turned_away {
            failures.push(format!("{method} {path} -> {} {loc}", resp.status()));
        }
    }
    assert!(
        failures.is_empty(),
        "routes that require an actor answered an anonymous caller: {failures:#?}"
    );
}

//! RFC 122 D4 — the set of routes carrying the one-time-secret
//! `Cache-Control: no-store` router layer is enumerated, and checked in
//! both directions.
//!
//! Unlike RFC 120's route test ([`super::r120_routes`]), "does this handler
//! show a one-time secret" has no syntactic marker on a handler's
//! signature — nothing distinguishes it from any other response-building
//! function. So the list below is hand-maintained, not derived from
//! handler shape. What *is* derived and checked is narrower: for each
//! `.route("<path>", <expr>)` in `src/http/router.rs`, whether `<expr>`
//! attaches `SetResponseHeaderLayer::overriding(CACHE_CONTROL, "no-store")`
//! directly to that route (the pattern RFC 103 established at
//! `/reset-password` and this RFC reuses). Comparing the derived set with
//! [`EXPECTED_NO_STORE_ROUTES`] in both directions means a route quietly
//! gaining or losing the layer is a diff someone must approve, not a
//! silent change — the same property RFC 120's test gives handler-actor
//! coverage, applied to a narrower, hand-kept list whose own completeness
//! this test cannot itself prove (RFC 122 D4).
//!
//! One surface is deliberately absent from both the router-layer list and
//! this test: `/admin/users/{id}/recovery-link` sets the header itself, in
//! the handler (`admin/users.rs`), predating this RFC and outside its
//! touches list; it has its own test
//! (`r103_stage4::recovery_link_response_is_no_store`, not shown here — see
//! that file).

use std::collections::BTreeSet;

use super::r120_routes::{balanced, parse_doc_table, repo_root, routes, src, strip_line_comments};

/// Every route this RFC (or its RFC 103 precedent) attaches the router-layer
/// `no-store` header to, with why. **Read the reason before removing an
/// entry** — the header stops being sent the moment its `.layer(...)` call
/// is removed from `router.rs`, silently, unless this list catches it.
///
/// RFC 127 D2: this fact also has a home in
/// `docs/src/reference/security-surfaces.md`'s second table, in this exact
/// order — [`the_documented_table_matches_expected_no_store_routes`] asserts
/// the two match.
const EXPECTED_NO_STORE_ROUTES: &[(&str, &str)] = &[
    (
        "/reset-password",
        "RFC 103: a one-time reset token held in a resubmitted form field",
    ),
    (
        "/admin/clients",
        "RFC 122: POST shows a freshly generated client secret once",
    ),
    (
        "/admin/clients/{id}/rotate-secret",
        "RFC 122: renders the rotated client secret directly, once",
    ),
    (
        "/me/security/mfa/enroll/start",
        "RFC 122: shows the TOTP secret and QR code once",
    ),
    (
        "/me/security/mfa/enroll/confirm",
        "RFC 122: shows the fresh recovery codes once",
    ),
    (
        "/me/security/mfa/recovery-codes/regenerate",
        "RFC 122: shows the regenerated recovery codes once",
    ),
    (
        "/oauth2/register",
        "RFC 122: RFC 7591 response carries a generated client_secret",
    ),
];

/// Every distinct path with a `.route(` call whose expression attaches the
/// `no-store` `SetResponseHeaderLayer` directly (not via an outer
/// `Router::layer`, which `routes()`'s per-verb parse would not see either).
fn paths_carrying_no_store_layer() -> BTreeSet<String> {
    let raw = std::fs::read_to_string(src("http/router.rs")).expect("read router.rs");
    let text = strip_line_comments(&raw);
    let mut found = BTreeSet::new();
    let mut at = 0usize;
    while let Some(i) = text[at..].find(".route(") {
        let open = at + i + ".route".len();
        let (args, next) = balanced(&text, open);
        at = next;
        let args = args.trim_start();
        assert!(
            args.starts_with('"'),
            "route path must be a string literal: {args:.60}"
        );
        let end = 1 + args[1..].find('"').expect("closing quote");
        let path = args[1..end].to_owned();
        let expr = &args[end + 1..];
        let carries_no_store = expr.contains("SetResponseHeaderLayer::overriding")
            && expr.contains("CACHE_CONTROL")
            && expr.contains("HeaderValue::from_static(\"no-store\")");
        if carries_no_store {
            found.insert(path);
        }
    }
    found
}

#[test]
fn the_routes_carrying_the_no_store_layer_are_exactly_the_expected_set() {
    // routes() is called for its side effect only: it asserts every
    // `.route(` in the file parses, so a form this file's own simpler
    // parse silently skipped would still be caught by the sibling test.
    let _ = routes();

    let found = paths_carrying_no_store_layer();
    let expected: BTreeSet<String> = EXPECTED_NO_STORE_ROUTES
        .iter()
        .map(|(p, _)| (*p).to_owned())
        .collect();
    let now_carrying: Vec<_> = found.difference(&expected).collect();
    let no_longer_carrying: Vec<_> = expected.difference(&found).collect();
    assert!(
        now_carrying.is_empty(),
        "these routes now carry the no-store router layer and are not in \
         EXPECTED_NO_STORE_ROUTES (a route added for a one-time secret needs \
         a reviewed entry with its reason): {now_carrying:#?}"
    );
    assert!(
        no_longer_carrying.is_empty(),
        "these expected routes no longer carry the no-store router layer, or \
         no longer exist; remove them from EXPECTED_NO_STORE_ROUTES only if \
         the surface they guarded is gone or now protected another way: \
         {no_longer_carrying:#?}"
    );
}

#[test]
fn every_expected_route_has_a_reason() {
    for (path, why) in EXPECTED_NO_STORE_ROUTES {
        assert!(why.len() > 8, "{path} needs a reason");
    }
}

/// RFC 127 D7: `docs/src/reference/security-surfaces.md`'s second table must
/// list exactly `EXPECTED_NO_STORE_ROUTES`, in the same order and with the
/// same reasons — the document is the checked artefact, not a cross-cited
/// description of one.
#[test]
fn the_documented_table_matches_expected_no_store_routes() {
    let doc = repo_root().join("docs/src/reference/security-surfaces.md");
    let rows = parse_doc_table(&doc, "Surfaces that show a secret once");
    let documented: Vec<(String, String)> = rows
        .into_iter()
        .map(|c| {
            assert_eq!(
                c.len(),
                2,
                "expected 2 columns (Route, What it shows), got {c:?}"
            );
            (c[0].clone(), c[1].clone())
        })
        .collect();
    let expected: Vec<(String, String)> = EXPECTED_NO_STORE_ROUTES
        .iter()
        .map(|(p, w)| ((*p).to_owned(), (*w).to_owned()))
        .collect();
    assert_eq!(
        documented, expected,
        "docs/src/reference/security-surfaces.md's second table must match \
         EXPECTED_NO_STORE_ROUTES exactly, in the same order (RFC 127 D7)"
    );
}

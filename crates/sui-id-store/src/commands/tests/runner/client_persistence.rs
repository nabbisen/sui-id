//! `clients::create` round-trips every field it is given — the persistence
//! bug (2026-10-02): the `INSERT` silently dropped `consent_policy` and the
//! four application-identity URIs, falling back to the table's defaults
//! regardless of what the caller passed. Affects every caller of `create`,
//! not only RFC 094 C15's dynamic-registration path.

use super::*;
use crate::models::ConsentPolicy;

#[tokio::test]
async fn create_persists_consent_policy_and_application_identity_uris() {
    let db = fresh_db();
    let mut row = a_client();
    row.consent_policy = ConsentPolicy::Always;
    row.logo_uri = Some("https://rp.test/logo.png".to_owned());
    row.homepage_uri = Some("https://rp.test/".to_owned());
    row.privacy_policy_uri = Some("https://rp.test/privacy".to_owned());
    row.tos_uri = Some("https://rp.test/tos".to_owned());
    let id = row.id;

    repos::clients::create(&db, &row).await.expect("create");

    let stored = repos::clients::get(&db, id).await.expect("read back");
    assert_eq!(stored.consent_policy, ConsentPolicy::Always);
    assert_eq!(stored.logo_uri.as_deref(), Some("https://rp.test/logo.png"));
    assert_eq!(stored.homepage_uri.as_deref(), Some("https://rp.test/"));
    assert_eq!(
        stored.privacy_policy_uri.as_deref(),
        Some("https://rp.test/privacy")
    );
    assert_eq!(stored.tos_uri.as_deref(), Some("https://rp.test/tos"));
}

/// `registered_via` is deliberately excluded from this round-trip: it is
/// the one field `create` still does not persist, by design (C11's own
/// dedicated, separately-audited writer is the only path to a non-default
/// value, and this dispatch's own "what this does not decide" section
/// forbids routing creation through the C09/C10/C11 setters). Pinning that
/// here means a future attempt to "fix" it by adding the column to this
/// `INSERT` fails a test that says why, rather than silently changing
/// behaviour C15 already depends on.
#[tokio::test]
async fn create_leaves_registered_via_at_the_table_default() {
    let db = fresh_db();
    let mut row = a_client();
    row.registered_via = crate::models::RegistrationSource::Dynamic;
    let id = row.id;

    repos::clients::create(&db, &row).await.expect("create");

    let stored = repos::clients::get(&db, id).await.expect("read back");
    assert_eq!(
        stored.registered_via,
        crate::models::RegistrationSource::Admin,
        "registered_via is not written by create; only set_registered_via (C11) sets it"
    );
}

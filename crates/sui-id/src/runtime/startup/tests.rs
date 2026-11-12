use super::*;
use crate::config::FederationProviderConfig;
use chrono::Utc;
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id_shared::ids::{FederationProviderId, UserId};
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{
    FederationLinkRow, FederationProviderRow, ProvisionMode, Role, UserRow, UserSource,
    ValidationStatus,
};
use sui_id_store::{Database, repos::federation_link, repos::federation_provider};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn seeded_row(slug: &str) -> FederationProviderRow {
    let now = Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: "https://accounts.google.com".into(),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        // RFC 096-B2 S2a: every one of this fixture's 14 callers goes
        // through `federation_provider::create`, which still writes
        // `enabled` verbatim -- `true` would now fail migration 0051's
        // own `CHECK` (no evidence, no `id_token_algs`, not `validated`).
        // None of this file's tests need `row.enabled` to be `true`
        // specifically; every assertion compares `fetched.enabled`
        // against `row.enabled` itself, or constructs `cfg.enabled` from
        // it, so this holds symmetrically either way.
        enabled: false,
        allowed_origins: "https://accounts.google.com".into(),
        config_version: 0,
        activation_generation: 0,
        validation_status: ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: None,
    }
}

fn matching_cfg(row: &FederationProviderRow) -> FederationProviderConfig {
    FederationProviderConfig {
        slug: row.slug.clone(),
        display_name: row.display_name.clone(),
        issuer: row.issuer.clone(),
        client_id: row.client_id.clone(),
        client_secret_env: String::new(),
        scopes: row.scopes.clone(),
        provision_mode: row.provision_mode.as_str().into(),
        enabled: row.enabled,
        allowed_origins: row
            .allowed_origins
            .split_whitespace()
            .map(String::from)
            .collect(),
        id_token_algs: vec!["RS256".into()],
    }
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn capture() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (captured, guard)
}

fn logged(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned()
}

/// RFC 096-B2 S2b: a config field that is also trust policy now applies
/// on this boot, and disables the provider -- replacing the S2a-era 7a/
/// 7b split (one field reconciled silently, every other only warned
/// about). Seeded through [`register_new_federation_provider`] so the
/// baseline row already has a recorded `id_token_algs` (C16's own write)
/// and is directly comparable -- a raw `seeded_row` has none recorded at
/// all, which would make every field "changed" for the wrong reason (no
/// decoded baseline to compare against).
#[tokio::test]
async fn a_changed_trust_field_disables_the_provider_and_logs_the_changed_fields() {
    let db = fresh_db();
    let cfg = matching_cfg(&seeded_row("google"));
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");

    let mut edited = cfg.clone();
    edited.scopes = "openid email profile".into();
    let (captured, _guard) = capture();
    reconcile_federation_provider(&db, &edited, &stored, None, Utc::now()).await;

    let after = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("still there");
    assert_eq!(after.scopes, "openid email profile", "the field is applied");
    assert!(
        !after.enabled,
        "a trust-field change must disable the provider"
    );
    assert_eq!(after.validation_status, ValidationStatus::RequiresReview);
    assert_eq!(after.config_version, stored.config_version + 1);
    assert_eq!(
        after.activation_generation,
        stored.activation_generation + 1
    );

    let log = logged(&captured);
    assert!(log.contains("google"), "does not name the slug: {log}");
    assert!(
        log.contains("scopes"),
        "does not name the changed field: {log}"
    );
    assert!(
        log.contains("re-enable"),
        "does not say it is now disabled until re-enabled: {log}"
    );
    assert!(
        log.contains("/admin/federation-providers"),
        "does not say where: {log}"
    );
    assert!(
        log.contains("Check and enable"),
        "does not say what to press: {log}"
    );
    assert_eq!(
        audit_count(&db, "federation.provider.policy_updated").await,
        1
    );
}

/// The other half: a config that already matches the stored, decoded
/// policy exactly is a true no-op -- no write, no event, no warning.
#[tokio::test]
async fn a_fully_matching_policy_is_a_no_op() {
    let db = fresh_db();
    let cfg = matching_cfg(&seeded_row("google"));
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");

    let (captured, _guard) = capture();
    reconcile_federation_provider(&db, &cfg, &stored, None, Utc::now()).await;

    let after = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("still there");
    assert_eq!(after.config_version, stored.config_version);
    assert_eq!(after.activation_generation, stored.activation_generation);
    assert_eq!(after.updated_at, stored.updated_at, "nothing was written");
    assert_eq!(
        audit_count(&db, "federation.provider.policy_updated").await,
        0
    );
    let log = logged(&captured);
    assert!(log.is_empty(), "expected no output at all:\n{log}");
}

/// `display_name` is not trust policy (RFC 096 `:434`) -- it applies
/// without disabling the provider, unlike every field in the test
/// above. Seeded enabled (via `create_enabled_for_test`) specifically to
/// prove the point: if this were mistakenly routed through C23, it
/// would come back disabled.
#[tokio::test]
async fn display_name_change_applies_without_disabling_the_provider() {
    let db = fresh_db();
    let mut row = seeded_row("google");
    row.id_token_algs = Some("RS256".into());
    federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.display_name = "A New Label".into();

    reconcile_federation_provider(&db, &cfg, &row, None, Utc::now()).await;

    let after = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(after.display_name, "A New Label");
    assert!(after.enabled, "display_name must not disable the provider");
    assert_eq!(after.config_version, row.config_version);
    assert_eq!(after.activation_generation, row.activation_generation);
    assert_eq!(
        audit_count(&db, "federation.provider.display_name_changed").await,
        1
    );
    assert_eq!(
        audit_count(&db, "federation.provider.policy_updated").await,
        0
    );
}

/// RFC 134's whole point: changing a trust field must not destroy
/// federation links to the provider (the old fix -- delete and
/// recreate -- cascades and does exactly that). C23 disables, it never
/// deletes.
#[tokio::test]
async fn federation_links_survive_a_policy_change() {
    let db = fresh_db();
    let cfg = matching_cfg(&seeded_row("google"));
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    let now = Utc::now();
    let user_id = UserId::new();
    sui_id_store::repos::users::create(
        &db,
        &UserRow {
            id: user_id,
            username: "fed-user".into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .unwrap();
    federation_link::upsert(
        &db,
        FederationLinkRow {
            user_id,
            provider_id: stored.id,
            upstream_sub: "sub-1".into(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
    let mut edited = cfg.clone();
    edited.allowed_origins = vec!["https://oauth2.googleapis.com".into()];
    reconcile_federation_provider(&db, &edited, &stored, None, Utc::now()).await;
    let still_there = federation_link::list_for_user(&db, user_id).await.unwrap();
    assert_eq!(still_there.len(), 1, "the federation link must survive");
}

/// RFC 094 M2b wave 3, C16, through the layer that actually runs it —
/// startup's own create path, not an HTTP route. A config block whose
/// slug is absent from the database is created and audited in one
/// transaction; previously the insert and the (nonexistent) audit
/// append were two uncoordinated steps and nothing emitted the event at
/// all (`git grep federation.provider.created` found no append call
/// anywhere in `crates/` before this round).
#[tokio::test]
async fn register_new_federation_provider_creates_and_audits_it() {
    let db = fresh_db();
    let row = seeded_row("new-provider");
    let cfg = matching_cfg(&row);

    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;

    let stored = federation_provider::get_by_slug(&db, "new-provider")
        .await
        .expect("provider created");
    let note: Option<String> = db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'federation.provider.created' \
                 AND target = ?1 ORDER BY seq DESC LIMIT 1",
                [stored.id.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row for federation.provider.created");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "slug").as_deref(),
        Some("new-provider")
    );
    assert_eq!(
        sui_id_store::registry::note_field(&note, "issuer").as_deref(),
        Some(stored.issuer.as_str())
    );
}

/// A failure registering a provider must not abort startup. Proven
/// structurally, not only by observation: `register_new_federation_
/// provider`'s own return type is `()`, not `Result<(), _>` — there is
/// no error for a caller to propagate with `?` even if someone tried.
/// Observed here too: a duplicate slug (the create's own `Conflict`
/// path) leaves exactly the one original row and does not panic.
#[tokio::test]
async fn register_new_federation_provider_does_not_abort_on_failure() {
    let db = fresh_db();
    let row = seeded_row("dup-provider");
    federation_provider::create(&db, &row, None).await.unwrap();
    let cfg = matching_cfg(&row);

    // Same slug already exists -- the insert hits the UNIQUE constraint.
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;

    let all = federation_provider::list(&db).await.expect("list");
    assert_eq!(all.len(), 1, "the duplicate attempt must not add a row");
}

/// RFC 096-B2 S2b, C23, through the layer that actually runs it — an
/// `allowed_origins` change is one case of the general trust-policy
/// replacement, audited as `federation.provider.policy_updated` with
/// `endpoint_origins` named in its `changed_fields`, not a dedicated
/// event of its own (C24, which this retires, had no `changed_fields`
/// concept at all).
#[tokio::test]
async fn differing_allowed_origins_update_is_audited() {
    let db = fresh_db();
    let cfg = matching_cfg(&seeded_row("google"));
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    let mut edited = cfg.clone();
    edited.allowed_origins = vec![
        "https://accounts.google.com".into(),
        "https://oauth2.googleapis.com".into(),
    ];

    reconcile_federation_provider(&db, &edited, &stored, None, Utc::now()).await;

    let note: Option<String> = db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = \
                 'federation.provider.policy_updated' AND target = ?1 \
                 ORDER BY seq DESC LIMIT 1",
                [stored.id.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row for federation.provider.policy_updated");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "changed_fields").as_deref(),
        Some("endpoint_origins")
    );
}

// ── RFC 096-B2 S1 §D: the per-provider "requires review" startup warning ───

/// `seeded_row` already starts `RequiresReview` (migration 0050's own
/// default), so a freshly created provider warns once, naming it by slug,
/// at `WARN`, and saying sign-in through it is off until an administrator
/// reviews and re-enables it.
#[tokio::test]
async fn requires_review_provider_warns_once_naming_the_slug() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    let provider_id = row.id;
    federation_provider::create(&db, &row, None).await.unwrap();

    warn_providers_requiring_review(&db).await;

    let log = logged(&captured);
    let found = log.lines().find(|l| l.contains("requires review"));
    assert!(
        found.is_some(),
        "no review-required warning line in:\n{log}"
    );
    let line = found.expect("checked above");
    assert!(line.contains("WARN"), "wrong level: {line}");
    assert!(line.contains("google"), "does not name the slug: {line}");
    assert!(
        line.contains("re-enabl"),
        "does not say sign-in is off until re-enabled: {line}"
    );
    assert!(
        line.contains("/admin/federation-providers"),
        "does not say where: {line}"
    );
    assert!(
        line.contains("Check and enable"),
        "does not say what to press: {line}"
    );
    // RFC 096-B2 S2a Part F: architecture.md:306-307's own upgrade
    // report asks for the internal ID, a yes/no for links, and the
    // missing policy fields (just `id_token_algs` after this stage).
    assert!(
        line.contains(&provider_id.to_string()),
        "does not name the internal id: {line}"
    );
    assert!(
        line.contains("has no linked user"),
        "does not say there is no link (this fixture has none): {line}"
    );
    assert!(
        line.contains("id_token_algs"),
        "does not name the missing policy field: {line}"
    );
    let count = log
        .lines()
        .filter(|l| l.contains("requires review"))
        .count();
    assert_eq!(count, 1, "expected exactly one warning line:\n{log}");
}

/// Part F's other required case: a provider with an existing link says
/// so (`has a linked user`), not `has no`.
#[tokio::test]
async fn requires_review_provider_with_an_existing_link_says_so() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    let provider_id = row.id;
    federation_provider::create(&db, &row, None).await.unwrap();
    let now = Utc::now();
    let user_id = UserId::new();
    sui_id_store::repos::users::create(
        &db,
        &UserRow {
            id: user_id,
            username: "fed-user".into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .unwrap();
    federation_link::upsert(
        &db,
        FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: "sub-1".into(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();

    warn_providers_requiring_review(&db).await;

    let log = logged(&captured);
    let line = log
        .lines()
        .find(|l| l.contains("requires review"))
        .expect("warning line");
    assert!(
        line.contains("has a linked user"),
        "does not say there is a link: {line}"
    );
}

/// A provider already `validated` -- not reachable through any writer
/// yet (C17 enable is S4), so this moves the row with a direct `UPDATE`
/// standing in for that future writer -- produces no warning at all.
#[tokio::test]
async fn validated_provider_produces_no_warning() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    db.with_conn(move |c| {
        Ok(c.execute(
            "UPDATE federation_provider SET validation_status = 'validated' WHERE id = ?1",
            [row.id.to_string()],
        )?)
    })
    .await
    .unwrap();

    warn_providers_requiring_review(&db).await;

    let log = logged(&captured);
    assert!(
        !log.contains("requires review"),
        "a validated provider must not warn:\n{log}"
    );
}

/// Two providers both still `requires_review` each get their own line,
/// naming their own slug -- not one warning that only names the first.
#[tokio::test]
async fn two_review_required_providers_each_get_their_own_warning() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    federation_provider::create(&db, &seeded_row("google"), None)
        .await
        .unwrap();
    federation_provider::create(&db, &seeded_row("github"), None)
        .await
        .unwrap();

    warn_providers_requiring_review(&db).await;

    let log = logged(&captured);
    let count = log
        .lines()
        .filter(|l| l.contains("requires review"))
        .count();
    assert_eq!(count, 2, "expected one warning line per provider:\n{log}");
    assert!(log.contains("google"));
    assert!(log.contains("github"));
}

/// No providers at all: the no-op case, asserted directly rather than
/// assumed -- this must neither warn nor error. `capture()` is armed
/// only after `fresh_db()` so the migration runner's own `INFO` lines
/// (`fresh_db` runs every migration, including 0050) are not mistaken
/// for silence.
#[tokio::test]
async fn no_providers_warns_about_nothing() {
    let db = fresh_db();
    let (captured, _guard) = capture();

    warn_providers_requiring_review(&db).await;

    let log = logged(&captured);
    assert!(log.is_empty(), "expected no output at all:\n{log}");
}

// ── RFC 096-B2 S2a Part E: the `enabled` config key no longer enables ──────

#[test]
fn warn_if_enabled_key_is_set_warns_once_naming_the_slug() {
    let (captured, _guard) = capture();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.enabled = true;

    warn_if_enabled_key_is_set(&cfg);

    let log = logged(&captured);
    let first_line = log.lines().next();
    assert!(first_line.is_some(), "no warning line in:\n{log}");
    let line = first_line.expect("checked above");
    assert!(line.contains("WARN"), "wrong level: {line}");
    assert!(line.contains("google"), "does not name the slug: {line}");
    assert!(
        line.contains("no longer enables"),
        "does not say the key no longer enables it: {line}"
    );
    assert!(
        line.contains("/admin/federation-providers"),
        "does not say where: {line}"
    );
}

#[test]
fn warn_if_enabled_key_is_set_is_silent_when_enabled_is_false() {
    let (captured, _guard) = capture();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.enabled = false;

    warn_if_enabled_key_is_set(&cfg);

    let log = logged(&captured);
    assert!(log.is_empty(), "expected no output at all:\n{log}");
}

// ── RFC 096-B2 S2a review fold-ins: reconcile compares by meaning ─────────
//
// C16 stores the validated policy, and reconcile used to compare the raw
// config strings with it. These pin that the two agree, so a boot after
// creation writes nothing and warns about nothing that did not change.

async fn audit_count(db: &Database, action: &'static str) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action = ?1",
            [action],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

/// The churn: a provider created with no `allowed_origins` must not be
/// rewritten, and audited, by C23 on the very next boot -- the stored
/// `""` and the config's empty list both resolve to the same effective
/// origin (the issuer's own), so the decoded comparison sees no change.
#[tokio::test]
async fn a_provider_created_with_default_origins_is_not_rewritten_on_the_next_boot() {
    let db = fresh_db();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.allowed_origins = Vec::new();
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");

    reconcile_federation_provider(&db, &cfg, &stored, None, Utc::now()).await;

    assert_eq!(
        audit_count(&db, "federation.provider.policy_updated").await,
        0,
        "nothing changed, so nothing is written or audited"
    );
    let after = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("still there");
    assert_eq!(after.allowed_origins, stored.allowed_origins);
    assert_eq!(after.config_version, stored.config_version);
}

#[tokio::test]
async fn scopes_spelled_in_another_order_produce_no_change() {
    let db = fresh_db();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.scopes = "email openid".into();
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    assert_eq!(
        stored.scopes, "openid email",
        "C16 stores the canonical order"
    );

    reconcile_federation_provider(&db, &cfg, &stored, None, Utc::now()).await;
    assert_eq!(
        audit_count(&db, "federation.provider.policy_updated").await,
        0,
        "same scopes, different spelling, must not be reported as a change"
    );
}

/// RFC 096-B2 S2b: the ID-token algorithms the callback trusts are now
/// applied from config on the next boot too, like every other trust
/// field -- this inverts the S2a-era behavior (not applied, warned
/// about) this test used to pin.
#[tokio::test]
async fn differing_id_token_algs_are_applied_and_disable_the_provider() {
    let db = fresh_db();
    let cfg = matching_cfg(&seeded_row("google"));
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    assert_eq!(stored.id_token_algs.as_deref(), Some("RS256"));

    let mut edited = cfg.clone();
    edited.id_token_algs = vec!["ES256".into()];
    reconcile_federation_provider(&db, &edited, &stored, None, Utc::now()).await;

    let after = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("still there");
    assert_eq!(after.id_token_algs.as_deref(), Some("ES256"), "now applied");
    assert!(!after.enabled);
    let note: Option<String> = db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'federation.provider.policy_updated' \
                 AND target = ?1 ORDER BY seq DESC LIMIT 1",
                [stored.id.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "changed_fields").as_deref(),
        Some("id_token_algs")
    );
}

#[tokio::test]
async fn the_same_id_token_algs_in_another_order_produce_no_change() {
    let db = fresh_db();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.id_token_algs = vec!["ES256".into(), "RS256".into()];
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    assert_eq!(stored.id_token_algs.as_deref(), Some("RS256 ES256"));

    reconcile_federation_provider(&db, &cfg, &stored, None, Utc::now()).await;
    assert_eq!(
        audit_count(&db, "federation.provider.policy_updated").await,
        0
    );
}

// ── RFC 096-B2 S2b §E (amendment A2): a provider removed from config ───────

#[tokio::test]
async fn a_provider_no_longer_configured_is_disabled_and_warns() {
    let db = fresh_db();
    let mut row = seeded_row("ghost");
    row.id_token_algs = Some("RS256".into());
    federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .unwrap();

    let (captured, _guard) = capture();
    remove_providers_no_longer_configured(&db, &[], Utc::now()).await;

    let after = federation_provider::get(&db, row.id).await.unwrap();
    assert!(!after.enabled, "must be disabled");
    assert_eq!(after.activation_generation, row.activation_generation + 1);
    assert_eq!(
        audit_count(&db, "federation.provider.removed_from_configuration").await,
        1
    );
    let log = logged(&captured);
    let line = log
        .lines()
        .find(|l| l.contains("no matching"))
        .expect("warning line");
    assert!(line.contains("ghost"), "{line}");
    assert!(line.contains(&row.id.to_string()), "{line}");
    assert!(
        line.contains("keeps its existing federation links"),
        "{line}"
    );
}

#[tokio::test]
async fn an_already_disabled_provider_no_longer_configured_still_warns_but_writes_nothing() {
    let db = fresh_db();
    let row = seeded_row("ghost-disabled");
    federation_provider::create(&db, &row, None).await.unwrap();

    let (captured, _guard) = capture();
    remove_providers_no_longer_configured(&db, &[], Utc::now()).await;

    let after = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        after.activation_generation, 0,
        "a generation must never be burned for nothing"
    );
    assert_eq!(
        audit_count(&db, "federation.provider.removed_from_configuration").await,
        0,
        "no event for an already-disabled provider"
    );
    let log = logged(&captured);
    assert!(
        log.lines().any(|l| l.contains("no matching")),
        "still warns every boot: {log}"
    );
}

#[tokio::test]
async fn a_configured_provider_is_never_touched_by_the_removal_pass() {
    let db = fresh_db();
    let mut row = seeded_row("still-here");
    row.id_token_algs = Some("RS256".into());
    federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .unwrap();
    let cfg = matching_cfg(&row);

    let (captured, _guard) = capture();
    remove_providers_no_longer_configured(&db, std::slice::from_ref(&cfg), Utc::now()).await;

    let after = federation_provider::get(&db, row.id).await.unwrap();
    assert!(after.enabled, "a configured provider must not be touched");
    let log = logged(&captured);
    assert!(log.is_empty(), "expected no output at all:\n{log}");
}

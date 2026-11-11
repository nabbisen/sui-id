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

/// 7a: a stored origin set differing from config is updated on boot.
#[tokio::test]
async fn differing_allowed_origins_are_updated() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec![
        "https://accounts.google.com".into(),
        "https://oauth2.googleapis.com".into(),
    ];
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        fetched.allowed_origins,
        "https://accounts.google.com https://oauth2.googleapis.com"
    );
}

/// 7a's other half: a stored origin set that already matches config is
/// **not written** -- asserted on the actual write, not just the end
/// state, since an unconditional update would pass a same-value check
/// alone.
#[tokio::test]
async fn matching_allowed_origins_produce_no_write() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let cfg = matching_cfg(&row);
    let later = row.updated_at + chrono::Duration::hours(1);
    reconcile_federation_provider(&db, &cfg, &row, later).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        fetched.updated_at, row.updated_at,
        "updated_at must not move if nothing was written"
    );
}

/// The update touches only `allowed_origins`: every other column is
/// unchanged across the reconciliation.
#[tokio::test]
async fn update_touches_only_allowed_origins() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec!["https://oauth2.googleapis.com".into()];
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(fetched.allowed_origins, "https://oauth2.googleapis.com");
    assert_eq!(fetched.display_name, row.display_name);
    assert_eq!(fetched.issuer, row.issuer);
    assert_eq!(fetched.client_id, row.client_id);
    assert_eq!(fetched.scopes, row.scopes);
    assert_eq!(fetched.provision_mode, row.provision_mode);
    assert_eq!(fetched.enabled, row.enabled);
    assert_eq!(fetched.created_at, row.created_at);
}

/// RFC 134's whole point: changing the origin set must not destroy
/// federation links to the provider (the old fix -- delete and
/// recreate -- cascades and does exactly that).
#[tokio::test]
async fn federation_links_survive_an_origin_set_change() {
    let db = fresh_db();
    let row = seeded_row("google");
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
            provider_id: row.id,
            upstream_sub: "sub-1".into(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec!["https://oauth2.googleapis.com".into()];
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let still_there = federation_link::list_for_user(&db, user_id).await.unwrap();
    assert_eq!(still_there.len(), 1, "the federation link must survive");
}

/// 7b: a differing `scopes` produces a warning and does not change the
/// stored value.
#[tokio::test]
async fn differing_scopes_produces_a_warning_and_no_write() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.scopes = "openid email profile".into();
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        fetched.scopes, row.scopes,
        "scopes must not be silently reconciled"
    );
    let log = logged(&captured);
    assert!(log.contains("field=scopes") || log.contains("field=\"scopes\""));
    assert!(log.contains("openid email profile"));
}

/// `enabled` is never compared -- it is meant to be a runtime toggle,
/// independent of this file, so a differing `enabled` must produce
/// neither a write nor a warning.
#[tokio::test]
async fn differing_enabled_produces_neither_write_nor_warning() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.enabled = !row.enabled;
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(fetched.enabled, row.enabled);
    let log = logged(&captured);
    assert!(!log.contains("field=enabled") && !log.contains("field=\"enabled\""));
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

/// RFC 094 M2b wave 3, C24, through the layer that actually runs it —
/// `differing_allowed_origins_are_updated` above already proves the
/// value changes; this proves the write is now also audited, which a
/// value-only assertion on an unchanged call site cannot (RFC 134 step
/// 7a's original write had no audit record of any kind).
#[tokio::test]
async fn differing_allowed_origins_update_is_audited() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec![
        "https://accounts.google.com".into(),
        "https://oauth2.googleapis.com".into(),
    ];

    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;

    let note: Option<String> = db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = \
                 'federation.provider.allowed_origins_changed' AND target = ?1 \
                 ORDER BY seq DESC LIMIT 1",
                [row.id.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row for federation.provider.allowed_origins_changed");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "origins_before").as_deref(),
        Some("1"),
        "the seeded row has one allowed origin"
    );
    assert_eq!(
        sui_id_store::registry::note_field(&note, "origins_after").as_deref(),
        Some("2")
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
        line.contains("operator guide"),
        "does not say where: {line}"
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
/// rewritten, and audited, by C24 on the very next boot.
#[tokio::test]
async fn a_provider_created_with_default_origins_is_not_rewritten_on_the_next_boot() {
    let db = fresh_db();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.allowed_origins = Vec::new();
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");

    reconcile_federation_provider(&db, &cfg, &stored, Utc::now()).await;

    assert_eq!(
        audit_count(&db, "federation.provider.allowed_origins_changed").await,
        0,
        "nothing changed, so nothing is written or audited"
    );
    let after = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("still there");
    assert_eq!(after.allowed_origins, stored.allowed_origins);
}

#[tokio::test]
async fn scopes_spelled_in_another_order_produce_no_warning() {
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

    let (captured, _guard) = capture();
    reconcile_federation_provider(&db, &cfg, &stored, Utc::now()).await;
    let log = logged(&captured);
    assert!(
        !log.contains("field=scopes") && !log.contains("field=\"scopes\""),
        "same scopes, different spelling: {log}"
    );
}

/// The callback trusts the stored algorithms (S2a part D), so an edit to
/// them in the config file is not applied, and must not be silent.
#[tokio::test]
async fn differing_id_token_algs_produce_a_warning_and_no_write() {
    let db = fresh_db();
    let cfg = matching_cfg(&seeded_row("google"));
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    assert_eq!(stored.id_token_algs.as_deref(), Some("RS256"));

    let mut edited = cfg.clone();
    edited.id_token_algs = vec!["ES256".into()];
    let (captured, _guard) = capture();
    reconcile_federation_provider(&db, &edited, &stored, Utc::now()).await;

    let after = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("still there");
    assert_eq!(after.id_token_algs.as_deref(), Some("RS256"), "not applied");
    let log = logged(&captured);
    assert!(
        log.contains("field=id_token_algs") || log.contains("field=\"id_token_algs\""),
        "{log}"
    );
    assert!(log.contains("ES256"), "{log}");
}

#[tokio::test]
async fn the_same_id_token_algs_in_another_order_produce_no_warning() {
    let db = fresh_db();
    let mut cfg = matching_cfg(&seeded_row("google"));
    cfg.id_token_algs = vec!["ES256".into(), "RS256".into()];
    register_new_federation_provider(&db, &cfg, None, Utc::now()).await;
    let stored = federation_provider::get_by_slug(&db, "google")
        .await
        .expect("created");
    assert_eq!(stored.id_token_algs.as_deref(), Some("RS256 ES256"));

    let (captured, _guard) = capture();
    reconcile_federation_provider(&db, &cfg, &stored, Utc::now()).await;
    let log = logged(&captured);
    assert!(
        !log.contains("field=id_token_algs") && !log.contains("field=\"id_token_algs\""),
        "{log}"
    );
}

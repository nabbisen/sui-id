//! Shared application state passed to handlers as `State<AppState>`.

use crate::config::Config;
use crate::ipnet::Cidr;
use crate::ratelimit::Limiters;
use std::sync::Arc;
use sui_id_core::cache::Caches;
use sui_id_core::hibp::HibpClient;
use sui_id_core::mail::MailSender;
use sui_id_core::time::{SharedClock, system_clock};
use sui_id_core::tokens::TokenLifetimes;
use sui_id_store::Database;
use sui_id_store::metrics::Metrics;
use sui_id_store::user_source::UserSource;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub clock: SharedClock,
    pub config: Arc<Config>,
    pub setup_token: Arc<String>,
    pub limiters: Arc<Limiters>,
    pub trusted_proxies: Arc<Vec<Cidr>>,
    /// Outbound mail sender. Production code constructs an
    /// `SmtpMailSender`; tests use `InMemoryMailSender`. Cloning
    /// the `AppState` clones the `Arc`; the underlying sender is
    /// shared.
    pub mailer: Arc<dyn MailSender>,
    /// Pwned Passwords (HIBP) breach-check client, used by the
    /// setup wizard to optionally screen the initial admin
    /// password (and, in v0.24.x patches, other password-set
    /// entry points). Production constructs an
    /// `HttpHibpClient`; tests inject `InMemoryHibpClient` from
    /// `sui-id-core`'s `test-support` feature.
    ///
    /// Even when `server_settings.hibp_mode = 'off'` we still
    /// hold a client here — the cost is one Arc clone, and
    /// keeping the field unconditional avoids a mode-checked
    /// match at every dispatch site.
    pub hibp_client: Arc<dyn HibpClient>,
    /// Hot-path caches (RFC 014): redirect-origins and JWKS signing keys.
    /// Rebuilt on startup and after mutations to clients/signing_keys.
    pub caches: Arc<Caches>,
    /// True when the process was started with `--dev`. Used to render the
    /// browser-side dev-mode banner on every page (RFC 032).
    pub is_dev_mode: bool,
    /// RFC 005: configured external user-sources for the auth cascade.
    /// Empty when no `[[user_source]]` blocks are in the config.
    pub user_sources: Vec<Arc<dyn UserSource>>,
    /// RFC 004: shared HTTP client for upstream federation requests
    /// (discovery fetches, token exchanges).
    pub http_client: Arc<reqwest::Client>,

    /// Prometheus metrics registry (RFC 006).
    /// `None` when `metrics_enabled = false` in config — no counters are
    /// incremented and the `/metrics` route is not registered.
    pub metrics: Option<Arc<Metrics>>,
}

impl AppState {
    pub fn new(
        db: Database,
        config: Config,
        setup_token: String,
        mailer: Arc<dyn MailSender>,
        hibp_client: Arc<dyn HibpClient>,
        caches: Arc<Caches>,
    ) -> Self {
        let trusted_proxies: Vec<Cidr> = config
            .server
            .trusted_proxies
            .iter()
            .filter_map(|s| Cidr::parse(s).ok())
            .collect();
        Self {
            db,
            clock: system_clock(),
            config: Arc::new(config),
            setup_token: Arc::new(setup_token),
            limiters: Arc::new(Limiters::default()),
            trusted_proxies: Arc::new(trusted_proxies),
            mailer,
            hibp_client,
            caches,
            is_dev_mode: false,
            metrics: None,
            user_sources: Vec::new(),
            http_client: Arc::new(
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .expect("failed to build federation HTTP client"),
            ),
        }
    }

    pub fn token_lifetimes(&self) -> TokenLifetimes {
        TokenLifetimes {
            access_secs: self.config.tokens.access_lifetime_secs,
            id_secs: self.config.tokens.id_token_lifetime_secs,
            refresh_secs: self.config.tokens.refresh_lifetime_secs,
        }
    }

    pub fn issuer(&self) -> &str {
        self.config.server.issuer.trim_end_matches('/')
    }

    /// Active security level for this process.
    ///
    /// Returns `SecurityLevel::Development` when running with `--dev`
    /// and `SecurityLevel::Standard` otherwise. Use this to obtain
    /// level-appropriate thresholds (e.g. `security_level().password_min_len()`)
    /// rather than branching on `is_dev_mode` directly at call sites.
    /// Convenience accessor for the metrics registry.
    /// Returns `None` when metrics are disabled (`metrics_enabled = false`).
    /// Call sites: `if let Some(m) = self.metric() { m.signin(result); }`
    #[inline]
    pub fn metric(&self) -> Option<&sui_id_store::metrics::Metrics> {
        self.metrics.as_deref()
    }

    pub fn security_level(&self) -> sui_id_core::security::SecurityLevel {
        if self.is_dev_mode {
            sui_id_core::security::SecurityLevel::Development
        } else {
            sui_id_core::security::SecurityLevel::Standard
        }
    }
}

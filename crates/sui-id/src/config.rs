//! Configuration loading.
//!
//! The on-disk config is a single `sui-id.toml`. We deliberately do not
//! support layered include files or environment overrides for everything;
//! this matches the project's "few moving parts" philosophy. Two settings
//! that *must* live outside the file are the master encryption key (per the
//! spec's prohibition on plaintext secrets in config) and an optional setup
//! token override.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub tokens: TokensConfig,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// `host:port` to listen on.
    pub listen_addr: String,
    /// External URL used as the OIDC `issuer`.
    pub issuer: String,
    /// Whether the cookie should be marked `Secure` (set this true behind HTTPS).
    #[serde(default)]
    pub cookie_secure: bool,
    /// CIDR ranges of reverse proxies whose `X-Forwarded-For` header should
    /// be trusted. Empty = always use the socket peer. See `sui-id.example.toml`
    /// for guidance.
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
    /// Enable the Prometheus `/metrics` endpoint (RFC 006).
    /// Default: `false`. When `false` the route is not registered and
    /// returns 404 so that the endpoint's existence is not leaked.
    #[serde(default)]
    pub metrics_enabled: bool,
    /// Optional separate listen address for the `/metrics` endpoint.
    /// Empty string (the default) mounts `/metrics` on the same listener
    /// as the main application. Set to e.g. `"127.0.0.1:9090"` to bind
    /// a private management port (strongly recommended for production).
    #[serde(default)]
    pub metrics_listen_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Path to the SQLite database.
    pub db_path: PathBuf,
    /// Path to a file holding the base64-encoded 32-byte master key.
    /// Used when the `SUI_ID_MASTER_KEY` environment variable is not set.
    /// On first start the file is created with permissions `0600`.
    pub key_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokensConfig {
    pub access_lifetime_secs: i64,
    pub id_token_lifetime_secs: i64,
    pub refresh_lifetime_secs: i64,
}

impl Default for TokensConfig {
    fn default() -> Self {
        Self {
            access_lifetime_secs: 15 * 60,
            id_token_lifetime_secs: 15 * 60,
            refresh_lifetime_secs: 14 * 24 * 60 * 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    /// `fmt` (human-readable) or `json` (one JSON object per line).
    #[serde(default = "default_log_format")]
    pub format: String,
    /// `tracing-subscriber` env-filter expression.
    #[serde(default = "default_log_filter")]
    pub filter: String,
    /// Emit one INFO line per HTTP request (method, path, status, request-id).
    ///
    /// Defaults to `false` in production. Set to `true` here or use
    /// `--dev` (which enables it automatically) to see requests arrive.
    #[serde(default)]
    pub access_log: bool,
    /// If set, write all log output to a daily-rotated file at this path
    /// (in addition to stderr). The path is a directory; log files are
    /// named `sui-id.YYYY-MM-DD.log`.
    ///
    /// When `None` (the default), only stderr is used.
    #[serde(default)]
    pub file: Option<PathBuf>,
}

fn default_log_format() -> String {
    "fmt".into()
}

fn default_log_filter() -> String {
    "info,sui_id=info,sui_id_core=info,sui_id_store=info".into()
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            format: default_log_format(),
            filter: default_log_filter(),
            access_log: false,
            file: None,
        }
    }
}

/// Security-policy knobs. Currently the only setting is the maximum
/// time an account remains locked after repeated failed sign-ins;
/// future settings (password-policy parameters, rate-limit
/// thresholds, …) will land here too.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    /// Cap on the auto-unlock interval used at the top of the
    /// progressive-backoff curve. After enough consecutive failures,
    /// the account is locked for *up to* this long; an admin can
    /// still unlock manually with `sui-id admin unlock-user`.
    ///
    /// The value is read from a small set of allowed durations so
    /// that an operator picking "12h" by hand cannot accidentally
    /// type "1h" or "120h"; see [`MaxLockoutDuration`]. Defaults to
    /// 24 hours.
    #[serde(default)]
    pub max_lockout: MaxLockoutDuration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_lockout: MaxLockoutDuration::default(),
        }
    }
}

/// Allowed maximum-lockout settings. A small, hand-picked set rather
/// than an arbitrary integer: each value is operationally meaningful
/// (an over-business-hours cooldown, a one-business-day cooldown, a
/// weekend cooldown). The cap of 48 hours is deliberate — locking
/// past two days is more likely to lock out a real user
/// (post-vacation, post-weekend) than to deter an attacker, who has
/// long given up by then.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MaxLockoutDuration {
    #[serde(rename = "15min")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "4h")]
    FourHours,
    #[serde(rename = "12h")]
    TwelveHours,
    #[serde(rename = "24h")]
    TwentyFourHours,
    #[serde(rename = "48h")]
    FortyEightHours,
}

impl Default for MaxLockoutDuration {
    fn default() -> Self {
        // 24h matches the default in the canonical operator examples
        // (NIST SP 800-63B suggests "rate limit … for at least one
        // day" for the higher AAL tiers, which is exactly this).
        Self::TwentyFourHours
    }
}

impl MaxLockoutDuration {
    /// The duration as a count of seconds. Used both to compute
    /// `users.locked_until` and to stamp the `Retry-After` HTTP
    /// header on a locked response.
    pub fn as_secs(self) -> i64 {
        match self {
            Self::FifteenMinutes => 15 * 60,
            Self::OneHour => 60 * 60,
            Self::FourHours => 4 * 60 * 60,
            Self::TwelveHours => 12 * 60 * 60,
            Self::TwentyFourHours => 24 * 60 * 60,
            Self::FortyEightHours => 48 * 60 * 60,
        }
    }

    /// Human-readable label for the duration, used by the settings
    /// admin page. Matches the wire form an operator would write
    /// in `sui-id.toml`.
    pub fn label(self) -> &'static str {
        match self {
            Self::FifteenMinutes => "15m",
            Self::OneHour => "1h",
            Self::FourHours => "4h",
            Self::TwelveHours => "12h",
            Self::TwentyFourHours => "24h",
            Self::FortyEightHours => "48h",
        }
    }
}

impl Config {
    /// Read and parse a TOML configuration file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let body = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read config {}: {e}", path.display()))?;
        let cfg: Config = toml::from_str(&body)
            .map_err(|e| anyhow::anyhow!("failed to parse config {}: {e}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Reasonable defaults useful for first-run output and tests.
    pub fn sample() -> Self {
        Self {
            server: ServerConfig { metrics_enabled: bool::default(), metrics_listen_addr: String::default(),
                listen_addr: "127.0.0.1:8801".into(),
                issuer: "http://127.0.0.1:8801".into(),
                cookie_secure: false,
                trusted_proxies: Vec::new(),
            },
            storage: StorageConfig {
                db_path: PathBuf::from("./sui-id.sqlite"),
                key_file: PathBuf::from("./sui-id.key"),
            },
            tokens: TokensConfig::default(),
            log: LogConfig::default(),
            security: SecurityConfig::default(),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.tokens.access_lifetime_secs <= 0 {
            anyhow::bail!("tokens.access_lifetime_secs must be positive");
        }
        if self.tokens.refresh_lifetime_secs <= self.tokens.access_lifetime_secs {
            anyhow::bail!("tokens.refresh_lifetime_secs should exceed access_lifetime_secs");
        }
        if !self.server.issuer.starts_with("http://") && !self.server.issuer.starts_with("https://") {
            anyhow::bail!("server.issuer must be an absolute http(s) URL");
        }
        for cidr in &self.server.trusted_proxies {
            crate::ipnet::Cidr::parse(cidr).map_err(|e| {
                anyhow::anyhow!("invalid CIDR in server.trusted_proxies: {cidr:?} ({e})")
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_validates() {
        Config::sample().validate().expect("sample is valid");
    }

    #[test]
    fn negative_lifetime_is_rejected() {
        let mut c = Config::sample();
        c.tokens.access_lifetime_secs = 0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn refresh_must_exceed_access() {
        let mut c = Config::sample();
        c.tokens.access_lifetime_secs = 100;
        c.tokens.refresh_lifetime_secs = 50;
        assert!(c.validate().is_err());
    }

    #[test]
    fn issuer_must_be_absolute() {
        let mut c = Config::sample();
        c.server.issuer = "/not-absolute".into();
        assert!(c.validate().is_err());
    }
}

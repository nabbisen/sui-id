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
            server: ServerConfig {
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

//! RFC 148 §1 (cycle O2b): the two commented configuration profiles
//! `sui-id config sample [--production]` and `sui-id init [--production]`
//! both write — one function per profile, called by both commands, so
//! they cannot print or write two different things for the same flag.
//!
//! Every value in [`local_trial_toml`] comes from `Config::sample()`
//! itself, not a second, hand-copied literal; `tests.rs`'s
//! `local_trial_toml_round_trips_to_config_sample` parses the rendered
//! text back and asserts equality, so a comment can never hide a value
//! that has drifted from `sample()`'s own default.

use crate::config::{Config, MaxLockoutDuration};

/// `sui-id config sample` / `sui-id init`: the local-trial profile.
pub(crate) fn local_trial_toml() -> String {
    let cfg = Config::sample();
    let max_lockout = match cfg.security.max_lockout {
        MaxLockoutDuration::FifteenMinutes => "15min",
        MaxLockoutDuration::OneHour => "1h",
        MaxLockoutDuration::FourHours => "4h",
        MaxLockoutDuration::TwelveHours => "12h",
        MaxLockoutDuration::TwentyFourHours => "24h",
        MaxLockoutDuration::FortyEightHours => "48h",
    };
    format!(
        r#"# sui-id local-trial configuration (written by `sui-id init`, or
# `sui-id config sample > sui-id.toml`).
#
# Minimal and insecure by design: HTTP, no TLS, a plaintext issuer bound to
# localhost. Fine for trying sui-id out; edit before any other use -- see
# `sui-id init --production` for a starting point meant for that.

[server]

# host:port this process listens on.
listen_addr = {listen_addr:?}

# External URL clients see as the OIDC issuer. Must be reachable by them.
# http:// only makes sense for this local-trial profile.
issuer = {issuer:?}

# Mark the session cookie `Secure`. Set true once served over https://.
cookie_secure = {cookie_secure}

# Trust `X-Forwarded-For` for the rate-limit IP. Empty = trust nobody (use
# the socket peer). See the configuration reference before setting this
# behind a reverse proxy.
trusted_proxies = {trusted_proxies:?}

# Prometheus /metrics endpoint (RFC 006). When false the route does not
# exist at all (404), so its existence is not leaked.
#metrics_enabled = {metrics_enabled}

# Separate listen address for /metrics. Empty mounts it on the main
# listener instead.
#metrics_listen_addr = {metrics_listen_addr:?}

[storage]

# SQLite database file. Created, and migrated, on first start.
db_path = {db_path:?}

# Master encryption key file. Generated with permissions 0600 on first
# start if missing. Back this file up -- losing it makes every encrypted
# column (refresh tokens, signing-key bytes) unreadable.
key_file = {key_file:?}

[tokens]

# Access and ID tokens are short-lived JWTs.
access_lifetime_secs = {access_lifetime_secs}
id_token_lifetime_secs = {id_token_lifetime_secs}

# Refresh tokens last much longer, and rotate on every use.
refresh_lifetime_secs = {refresh_lifetime_secs}

# ----- external user sources (RFC 005) and upstream OIDC federation
# (RFC 004) are both omitted here -- local accounts only. One block per
# source/provider adds either; see the configuration reference.

[log]

# "fmt" (human-readable) here; "json" for production log aggregators.
format = {format:?}

# `tracing-subscriber` env-filter expression.
filter = {filter:?}

# One INFO line per HTTP request. Handy while trying things out.
access_log = {access_log}

[security]

# Maximum auto-unlock interval for account lockout; see the configuration
# reference for the full progressive-backoff curve this caps.
max_lockout = {max_lockout:?}
"#,
        listen_addr = cfg.server.listen_addr,
        issuer = cfg.server.issuer,
        cookie_secure = cfg.server.cookie_secure,
        trusted_proxies = cfg.server.trusted_proxies,
        metrics_enabled = cfg.server.metrics_enabled,
        metrics_listen_addr = cfg.server.metrics_listen_addr,
        db_path = cfg.storage.db_path,
        key_file = cfg.storage.key_file,
        access_lifetime_secs = cfg.tokens.access_lifetime_secs,
        id_token_lifetime_secs = cfg.tokens.id_token_lifetime_secs,
        refresh_lifetime_secs = cfg.tokens.refresh_lifetime_secs,
        format = cfg.log.format,
        filter = cfg.log.filter,
        access_log = cfg.log.access_log,
        max_lockout = max_lockout,
    )
}

/// `sui-id config sample --production` / `sui-id init --production`:
/// `sui-id.example.toml`, already commented and held to `Config::sample()`'s
/// *keys* by RFC 147 §4's own test. Embedded so an installed binary (no
/// source checkout) can still print and write it.
pub(crate) fn production_toml() -> &'static str {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/sui-id.example.toml"))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "templates/tests.rs"]
mod tests;

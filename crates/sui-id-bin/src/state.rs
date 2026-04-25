//! Shared application state passed to handlers as `State<AppState>`.

use crate::config::Config;
use crate::ratelimit::Limiters;
use std::sync::Arc;
use sui_id_core::time::{system_clock, SharedClock};
use sui_id_core::tokens::TokenLifetimes;
use sui_id_store::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub clock: SharedClock,
    pub config: Arc<Config>,
    pub setup_token: Arc<String>,
    pub limiters: Arc<Limiters>,
}

impl AppState {
    pub fn new(db: Database, config: Config, setup_token: String) -> Self {
        Self {
            db,
            clock: system_clock(),
            config: Arc::new(config),
            setup_token: Arc::new(setup_token),
            limiters: Arc::new(Limiters::default()),
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
}

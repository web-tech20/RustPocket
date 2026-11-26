use std::sync::Arc;

use crate::{config::Config, db::Db, limiter::RateLimiter, sandbox::Sandbox};

/// Shared state handed to every request handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<Db>,
    pub sandbox: Arc<Sandbox>,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(config: Config, db: Db, sandbox: Sandbox, limiter: RateLimiter) -> Self {
        Self {
            config: Arc::new(config),
            db: Arc::new(db),
            sandbox: Arc::new(sandbox),
            limiter: Arc::new(limiter),
        }
    }
}

use std::sync::Arc;

use rustpocket_server::{
    api, config::Config, db::Db, limiter::RateLimiter, sandbox::Sandbox, state::AppState,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustpocket_server=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    let db = Db::open(&config.database_path)?;
    let sandbox = Sandbox::new(Arc::new(config.clone()));
    let limiter = RateLimiter::new(config.runs_per_minute, config.runs_per_day);
    let state = Arc::new(AppState::new(config.clone(), db, sandbox, limiter));

    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind(config.bind).await?;

    tracing::info!(
        address = %config.bind,
        sandbox = ?config.sandbox_mode,
        "RustPocket server listening"
    );

    axum::serve(listener, app).await?;
    Ok(())
}

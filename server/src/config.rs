use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

/// Where the untrusted code is executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// Spawn `rustc` directly on the host, with rlimits and a timeout.
    /// Meant for development only.
    Local,
    /// Run every command in a throw-away Docker container
    /// (`--network none`, memory/CPU/PID limits).
    Docker,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_path: PathBuf,
    pub jwt_secret: String,
    pub token_ttl_secs: i64,
    pub sandbox_mode: SandboxMode,
    pub sandbox_image: String,
    /// Root directory for the per-run workspaces. When the server runs in a
    /// container this must be a path shared with the Docker daemon.
    pub work_dir: PathBuf,
    /// Named Docker volume holding `work_dir`. Required when the server itself
    /// runs in a container, because the daemon resolves bind-mount paths on the
    /// host, not inside the server container.
    pub work_volume: Option<String>,
    pub max_code_bytes: usize,
    pub max_output_bytes: usize,
    pub compile_timeout: Duration,
    pub run_timeout: Duration,
    pub format_timeout: Duration,
    pub runs_per_minute: usize,
    pub runs_per_day: usize,
}

impl Config {
    /// Reads the configuration from the environment, falling back to the
    /// development defaults documented in `.env.example`.
    pub fn from_env() -> Self {
        let bind = env::var("RUSTPOCKET_BIND")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 8080)));

        let sandbox_mode = match env::var("RUSTPOCKET_SANDBOX")
            .unwrap_or_else(|_| "local".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "docker" => SandboxMode::Docker,
            _ => SandboxMode::Local,
        };

        let jwt_secret = env::var("RUSTPOCKET_JWT_SECRET").unwrap_or_else(|_| {
            tracing::warn!(
                "RUSTPOCKET_JWT_SECRET is not set, using a development secret. \
                 Set it before exposing the server."
            );
            "rustpocket-dev-secret-change-me".to_string()
        });

        Self {
            bind,
            database_path: env::var("RUSTPOCKET_DATABASE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("rustpocket.db")),
            jwt_secret,
            token_ttl_secs: env_secs("RUSTPOCKET_TOKEN_TTL", 30 * 24 * 3600) as i64,
            sandbox_mode,
            sandbox_image: env::var("RUSTPOCKET_SANDBOX_IMAGE")
                .unwrap_or_else(|_| "rustpocket-sandbox:latest".into()),
            work_dir: env::var("RUSTPOCKET_WORK_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| env::temp_dir()),
            work_volume: env::var("RUSTPOCKET_WORK_VOLUME")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            max_code_bytes: env_usize("RUSTPOCKET_MAX_CODE_BYTES", 64 * 1024),
            max_output_bytes: env_usize("RUSTPOCKET_MAX_OUTPUT_BYTES", 64 * 1024),
            compile_timeout: Duration::from_secs(env_secs("RUSTPOCKET_COMPILE_TIMEOUT", 30)),
            run_timeout: Duration::from_secs(env_secs("RUSTPOCKET_RUN_TIMEOUT", 10)),
            format_timeout: Duration::from_secs(env_secs("RUSTPOCKET_FORMAT_TIMEOUT", 10)),
            runs_per_minute: env_usize("RUSTPOCKET_RUNS_PER_MINUTE", 20),
            runs_per_day: env_usize("RUSTPOCKET_RUNS_PER_DAY", 300),
        }
    }
}

fn env_secs(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

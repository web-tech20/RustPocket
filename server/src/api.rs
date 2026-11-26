use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    auth::{AuthUser, create_token, hash_password, verify_password},
    config::Config,
    db::User,
    error::ApiError,
    sandbox::{BuildMode, CompileResult, RunResult, SandboxError},
    state::AppState,
};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/me", get(me))
        .route("/run", post(run))
        .route("/compile", post(compile))
        .route("/check", post(check))
        .route("/format", post(format))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "rustpocket-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: String,
    email: String,
}

#[derive(Serialize)]
struct AuthResponse {
    token: String,
    user: UserResponse,
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let email = normalize_email(&body.email)?;
    validate_password(&body.password)?;

    let user = User {
        id: uuid::Uuid::new_v4().to_string(),
        email,
        password_hash: hash_password(&body.password)?,
    };
    state.db.insert_user(&user)?;
    let token = create_token(&state.config, &user)?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: UserResponse {
                id: user.id,
                email: user.email,
            },
        }),
    ))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = body.email.trim().to_lowercase();
    let user = state.db.find_user_by_email(&email)?;

    let Some(user) = user else {
        return Err(ApiError::Unauthorized);
    };
    if !verify_password(&body.password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }

    let token = create_token(&state.config, &user)?;
    Ok(Json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            email: user.email,
        },
    }))
}

#[derive(Serialize)]
struct MeResponse {
    email: String,
    quotas: crate::limiter::QuotaStatus,
    limits: LimitsResponse,
}

#[derive(Serialize)]
struct LimitsResponse {
    max_code_bytes: usize,
    run_timeout_secs: u64,
    compile_timeout_secs: u64,
}

async fn me(State(state): State<Arc<AppState>>, user: AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        email: user.email,
        quotas: state.limiter.status(&user.id),
        limits: LimitsResponse {
            max_code_bytes: state.config.max_code_bytes,
            run_timeout_secs: state.config.run_timeout.as_secs(),
            compile_timeout_secs: state.config.compile_timeout.as_secs(),
        },
    })
}

#[derive(Deserialize)]
struct CodeRequest {
    code: String,
}

async fn run(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<CodeRequest>,
) -> Result<Json<RunResult>, ApiError> {
    check_request(&state.config, &state.limiter, &user, &body.code)?;
    state
        .sandbox
        .run(&body.code)
        .await
        .map(Json)
        .map_err(map_sandbox)
}

async fn compile(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<CodeRequest>,
) -> Result<Json<CompileResult>, ApiError> {
    check_request(&state.config, &state.limiter, &user, &body.code)?;
    state
        .sandbox
        .compile(&body.code, BuildMode::Binary)
        .await
        .map(Json)
        .map_err(map_sandbox)
}

async fn check(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<CodeRequest>,
) -> Result<Json<CompileResult>, ApiError> {
    check_request(&state.config, &state.limiter, &user, &body.code)?;
    state
        .sandbox
        .compile(&body.code, BuildMode::Metadata)
        .await
        .map(Json)
        .map_err(map_sandbox)
}

#[derive(Serialize)]
struct FormatResponse {
    code: String,
}

async fn format(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<CodeRequest>,
) -> Result<Json<FormatResponse>, ApiError> {
    check_request(&state.config, &state.limiter, &user, &body.code)?;
    let code = state
        .sandbox
        .format(&body.code)
        .await
        .map_err(map_sandbox)?;
    Ok(Json(FormatResponse { code }))
}

fn check_request(
    config: &Config,
    limiter: &crate::limiter::RateLimiter,
    user: &AuthUser,
    code: &str,
) -> Result<(), ApiError> {
    if code.len() > config.max_code_bytes {
        return Err(ApiError::BadRequest(format!(
            "code trop volumineux : {} octets (maximum {}).",
            code.len(),
            config.max_code_bytes
        )));
    }

    // Quotas are consumed before touching the sandbox.
    limiter
        .consume(&user.id)
        .map(|_| ())
        .map_err(|quota| ApiError::TooManyRequests(quota.message))
}

fn map_sandbox(err: SandboxError) -> ApiError {
    match err {
        SandboxError::Format(message) => ApiError::BadRequest(message),
        SandboxError::Workspace(message) | SandboxError::Unavailable(message) => {
            ApiError::Sandbox(message)
        }
    }
}

async fn not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": {
                "code": "not_found",
                "message": "route inconnue"
            }
        })),
    )
}

fn normalize_email(raw: &str) -> Result<String, ApiError> {
    let email = raw.trim().to_lowercase();
    if email.len() > 254 || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(ApiError::BadRequest("adresse e-mail invalide.".to_string()));
    }
    Ok(email)
}

fn validate_password(password: &str) -> Result<(), ApiError> {
    if password.len() < 8 {
        return Err(ApiError::BadRequest(
            "le mot de passe doit contenir au moins 8 caractères.".to_string(),
        ));
    }
    if password.len() > 128 {
        return Err(ApiError::BadRequest(
            "le mot de passe est trop long (128 caractères maximum).".to_string(),
        ));
    }
    Ok(())
}

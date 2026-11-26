use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use serde::{Deserialize, Serialize};

use crate::{config::Config, db::User, error::ApiError, state::AppState};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub iat: usize,
    pub exp: usize,
}

/// The authenticated caller, extracted from `Authorization: Bearer <jwt>`.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiError::InvalidToken)?;

        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .ok_or(ApiError::InvalidToken)?
            .trim();

        let claims = verify_token(&state.config, token)?;
        Ok(Self {
            id: claims.sub,
            email: claims.email,
        })
    }
}

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, phc::PasswordHash},
    };

    let hash: PasswordHash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("hachage du mot de passe: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    use argon2::{
        Argon2,
        password_hash::{PasswordVerifier, phc::PasswordHash},
    };

    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn create_token(config: &Config, user: &User) -> Result<String, ApiError> {
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

    let now = now_secs();
    let claims = Claims {
        sub: user.id.clone(),
        email: user.email.clone(),
        iat: now,
        exp: now + config.token_ttl_secs as usize,
    };
    let key = EncodingKey::from_secret(config.jwt_secret.as_bytes())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("clé de signature: {e}")))?;
    encode(&Header::new(Algorithm::HS256), &claims, &key)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("signature du jeton: {e}")))
}

pub fn verify_token(config: &Config, token: &str) -> Result<Claims, ApiError> {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let key = DecodingKey::from_secret(config.jwt_secret.as_bytes())
        .map_err(|_| ApiError::InvalidToken)?;
    let data = decode::<Claims>(token, &key, &validation).map_err(|_| ApiError::InvalidToken)?;
    Ok(data.claims)
}

fn now_secs() -> usize {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or_default()
}

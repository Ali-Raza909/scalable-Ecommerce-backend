use argon2::password_hash::{PasswordHash, PasswordVerifier};
use argon2::Argon2;
use axum::{extract::State, Json};
use chrono::Utc;
use sqlx::PgPool;

use crate::auth;
use crate::db;
use crate::error::AppError;
use crate::models::{LoginRequest, LoginResponse};

pub async fn login(
    State(pool): State<PgPool>,
    Json(input): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user = db::get_user_by_email(&pool, &input.email)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| anyhow::anyhow!("Failed to parse password hash: {}", e))?;

    Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized)?;

    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let token = auth::create_token(&user.id.to_string(), &user.email, &jwt_secret)?;
    let expires_at = Utc::now() + chrono::Duration::hours(24);

    Ok(Json(LoginResponse {
        token,
        expires_at,
    }))
}

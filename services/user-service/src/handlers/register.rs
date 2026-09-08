use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use rand::rngs::OsRng;
use sqlx::PgPool;

use crate::db;
use crate::error::AppError;
use crate::models::{RegisterRequest, UserResponse};

pub async fn register(
    State(pool): State<PgPool>,
    Json(input): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    if input.email.is_empty() || input.password.is_empty() || input.full_name.is_empty() {
        return Err(AppError::BadRequest(
            "Email, password, and full name are required".to_string(),
        ));
    }

    if db::get_user_by_email(&pool, &input.email).await?.is_some() {
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(input.password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
        .to_string();

    let user = db::create_user(&pool, &input.email, &password_hash, &input.full_name).await?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

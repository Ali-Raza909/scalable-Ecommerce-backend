use axum::{extract::State, Json};
use sqlx::PgPool;
use serde_json::{json, Value};

pub async fn stripe_webhook(
    State(pool): State<PgPool>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, crate::error::AppError> {
    // TODO: Implement stripe webhook
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
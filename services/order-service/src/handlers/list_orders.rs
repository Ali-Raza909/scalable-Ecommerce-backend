use axum::{extract::State, Json};
use sqlx::PgPool;

pub async fn list_orders(
    State(pool): State<PgPool>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement list orders
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
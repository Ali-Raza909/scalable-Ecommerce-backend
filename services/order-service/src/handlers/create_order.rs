use axum::{extract::State, Json};
use sqlx::PgPool;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    // TODO: Add fields as needed
}

pub async fn create_order(
    State(pool): State<PgPool>,
    Json(input): Json<CreateOrderRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement create order
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
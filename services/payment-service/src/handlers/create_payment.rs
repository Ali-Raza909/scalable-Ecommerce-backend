use axum::{extract::State, Json};
use sqlx::PgPool;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreatePaymentRequest {
    pub order_id: String,
    pub amount_cents: i32,
}

pub async fn create_payment(
    State(pool): State<PgPool>,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement create payment
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
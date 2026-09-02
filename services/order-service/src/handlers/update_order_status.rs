use axum::{extract::{Path, State}, Json};
use sqlx::PgPool;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

pub async fn update_order_status(
    State(pool): State<PgPool>,
    Path(order_id): Path<Uuid>,
    Json(input): Json<UpdateStatusRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement update order status
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
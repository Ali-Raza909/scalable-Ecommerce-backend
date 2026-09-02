use axum::{extract::{Path, State}, Json};
use sqlx::PgPool;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct UpdateStockRequest {
    pub delta: i32,
}

pub async fn update_stock(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
    Json(input): Json<UpdateStockRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement update stock
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
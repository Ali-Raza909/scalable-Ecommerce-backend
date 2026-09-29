use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::{Product, UpdateStockRequest};

pub async fn update_stock(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
    Json(input): Json<UpdateStockRequest>,
) -> Result<Json<Product>, AppError> {
    if input.delta == 0 {
        return Err(AppError::BadRequest("Delta cannot be zero".to_string()));
    }

    let product = db::update_stock(&pool, product_id, input.delta)
        .await?
        .ok_or(AppError::Conflict("Insufficient stock".to_string()))?;

    Ok(Json(product))
}

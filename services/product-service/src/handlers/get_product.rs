use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::Product;

pub async fn get_product(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
) -> Result<Json<Product>, AppError> {
    let product = db::get_product_by_id(&pool, product_id)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(product))
}

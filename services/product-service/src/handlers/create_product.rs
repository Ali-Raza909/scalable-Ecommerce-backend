use axum::{extract::State, Json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::{CreateProductRequest, Product};

pub async fn create_product(
    State(pool): State<PgPool>,
    Json(input): Json<CreateProductRequest>,
) -> Result<Json<Product>, AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("Product name is required".to_string()));
    }
    if input.price_cents < 0 {
        return Err(AppError::BadRequest("Price cannot be negative".to_string()));
    }
    if input.stock_quantity < 0 {
        return Err(AppError::BadRequest(
            "Stock quantity cannot be negative".to_string(),
        ));
    }

    let category_id = Uuid::parse_str(&input.category_id)
        .map_err(|_| AppError::BadRequest("Invalid category id".to_string()))?;

    if !db::ensure_category_exists(&pool, category_id).await? {
        return Err(AppError::BadRequest("Category does not exist".to_string()));
    }

    let product = db::create_product(
        &pool,
        category_id,
        &input.name,
        input.description.as_deref(),
        input.price_cents,
        input.stock_quantity,
    )
    .await?;

    Ok(Json(product))
}

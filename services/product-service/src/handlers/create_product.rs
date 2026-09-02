use axum::{extract::State, Json};
use sqlx::PgPool;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateProductRequest {
    pub category_id: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub stock_quantity: i32,
}

pub async fn create_product(
    State(pool): State<PgPool>,
    Json(input): Json<CreateProductRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement create product
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
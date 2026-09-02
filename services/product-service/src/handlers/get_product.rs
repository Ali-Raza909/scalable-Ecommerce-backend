use axum::{extract::{Path, State}, Json};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_product(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement get product
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
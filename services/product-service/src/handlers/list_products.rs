use axum::{extract::State, Json};
use sqlx::PgPool;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListProductsQuery {
    pub category: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

pub async fn list_products(
    State(pool): State<PgPool>,
    query: axum::extract::Query<ListProductsQuery>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement list products
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
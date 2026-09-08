use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: Uuid,
    pub category_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub stock_quantity: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub category_id: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub stock_quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct ListProductsQuery {
    pub category: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedProducts {
    pub items: Vec<Product>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStockRequest {
    pub delta: i32,
}

use axum::{extract::State, Json};
use redis::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AddItemRequest {
    pub product_id: String,
    pub quantity: i32,
}

pub async fn add_item(
    State(redis_client): State<Client>,
    Json(input): Json<AddItemRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    // TODO: Implement add item
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
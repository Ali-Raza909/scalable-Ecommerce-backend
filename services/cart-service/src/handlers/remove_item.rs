use axum::{extract::{Path, State}, Json};
use redis::Client;
use serde_json::{json, Value};

pub async fn remove_item(
    State(redis_client): State<Client>,
    Path(product_id): Path<String>,
) -> Result<Json<Value>, crate::error::AppError> {
    // TODO: Implement remove item
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
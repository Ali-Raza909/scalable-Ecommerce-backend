use axum::{extract::State, Json};
use redis::Client;
use serde_json::{json, Value};

pub async fn get_cart(
    State(redis_client): State<Client>,
) -> Result<Json<Value>, crate::error::AppError> {
    // TODO: Implement get cart
    Err(crate::error::AppError::BadRequest("Not implemented".to_string()))
}
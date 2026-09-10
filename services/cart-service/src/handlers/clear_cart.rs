use axum::{extract::{Extension, State}, Json};
use common::Claims;
use redis::AsyncCommands;
use redis::Client;
use serde_json::{json, Value};

use crate::auth::cart_key;
use crate::error::AppError;

pub async fn clear_cart(
    State(redis_client): State<Client>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, AppError> {
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| anyhow::anyhow!("Redis connection failed: {}", e))?;

    let deleted: i64 = conn
        .del(cart_key(&claims.sub))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to clear cart: {}", e))?;

    Ok(Json(json!({ "status": "ok", "deleted": deleted })))
}
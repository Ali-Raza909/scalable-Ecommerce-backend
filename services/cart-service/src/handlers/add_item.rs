use axum::{extract::{Extension, State}, Json};
use common::Claims;
use redis::AsyncCommands;
use redis::Client;

use crate::auth::cart_key;
use crate::error::AppError;
use crate::models::{AddItemRequest, Cart, CartItem};

const CART_TTL_SECONDS: i64 = 60 * 60 * 24 * 30;

pub async fn add_item(
    State(redis_client): State<Client>,
    Extension(claims): Extension<Claims>,
    Json(input): Json<AddItemRequest>,
) -> Result<Json<Cart>, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::BadRequest(
            "Quantity must be positive".to_string(),
        ));
    }

    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| anyhow::anyhow!("Redis connection failed: {}", e))?;

    let key = cart_key(&claims.sub);

    conn.hincr::<_, _, _, ()>(&key, &input.product_id, input.quantity)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add item to cart: {}", e))?;
    conn.expire::<_, ()>(&key, CART_TTL_SECONDS)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set cart TTL: {}", e))?;

    let entries: Vec<(String, String)> = conn
        .hgetall(&key)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read cart: {}", e))?;

    let items = entries
        .into_iter()
        .filter_map(|(product_id, qty)| {
            qty.parse::<i32>().ok().map(|quantity| CartItem {
                product_id,
                quantity,
            })
        })
        .collect();

    Ok(Json(Cart { items }))
}
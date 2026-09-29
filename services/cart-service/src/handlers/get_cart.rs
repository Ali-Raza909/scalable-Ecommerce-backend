use axum::{
    extract::{Extension, State},
    Json,
};
use common::Claims;
use redis::AsyncCommands;
use redis::Client;

use crate::auth::cart_key;
use crate::error::AppError;
use crate::models::{Cart, CartItem};

pub async fn get_cart(
    State(redis_client): State<Client>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Cart>, AppError> {
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| anyhow::anyhow!("Redis connection failed: {}", e))?;

    let entries: Vec<(String, String)> = conn
        .hgetall(cart_key(&claims.sub))
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

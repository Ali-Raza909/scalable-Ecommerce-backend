use axum::{routing::{get, post, delete}, Router};
use redis::Client;

use crate::handlers;

pub fn create_router(redis_client: Client) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/cart", get(handlers::get_cart).delete(handlers::clear_cart))
        .route("/cart/items", post(handlers::add_item))
        .route("/cart/items/:product_id", delete(handlers::remove_item))
        .with_state(redis_client)
}
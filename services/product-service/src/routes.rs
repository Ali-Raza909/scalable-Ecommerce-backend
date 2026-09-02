use axum::{routing::{get, post, patch}, Router};
use sqlx::PgPool;

use crate::handlers;

pub fn create_router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/products", get(handlers::list_products).post(handlers::create_product))
        .route("/products/:id", get(handlers::get_product))
        .route("/products/:id/stock", patch(handlers::update_stock))
        .with_state(pool)
}
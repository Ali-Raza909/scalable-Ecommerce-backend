use axum::{routing::{get, post, patch}, Router};
use sqlx::PgPool;

use crate::handlers;

pub fn create_router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/orders", post(handlers::create_order).get(handlers::list_orders))
        .route("/orders/:id", get(handlers::get_order))
        .route("/orders/:id/status", patch(handlers::update_order_status))
        .with_state(pool)
}
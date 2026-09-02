use axum::{routing::{get, post}, Router};
use sqlx::PgPool;

use crate::handlers;

pub fn create_router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/payments", post(handlers::create_payment))
        .route("/webhooks/stripe", post(handlers::stripe_webhook))
        .with_state(pool)
}
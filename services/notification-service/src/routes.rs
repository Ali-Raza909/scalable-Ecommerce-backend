use axum::{routing::{get, post}, Router};

use crate::handlers;

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/notify/email", post(handlers::send_email))
}
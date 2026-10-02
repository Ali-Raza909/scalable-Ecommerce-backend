use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use common::request_id_middleware;

use crate::handlers;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/payments", post(handlers::create_payment))
        .route("/webhooks/safepay", post(handlers::safepay_webhook))
        .route("/payments/success", get(handlers::payment_success))
        .route("/payments/cancel", get(handlers::payment_cancel))
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(state)
}

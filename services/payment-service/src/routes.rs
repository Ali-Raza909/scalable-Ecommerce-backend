use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/payments", post(handlers::create_payment))
        .route("/webhooks/safepay", post(handlers::safepay_webhook))
        .route("/payments/success", get(handlers::payment_success))
        .route("/payments/cancel", get(handlers::payment_cancel))
        .with_state(state)
}

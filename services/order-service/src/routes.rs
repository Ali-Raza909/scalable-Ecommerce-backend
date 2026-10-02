use axum::{
    middleware,
    routing::{get, patch, post},
    Router,
};
use common::request_id_middleware;

use crate::handlers;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let jwt_secret = state.jwt_secret.clone();

    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .route(
            "/orders/:id/payment-confirm",
            post(handlers::payment_confirm),
        )
        .route(
            "/orders/payment-result/:id/:result",
            get(handlers::payment_result),
        );

    let protected_routes = Router::new()
        .route(
            "/orders",
            post(handlers::create_order).get(handlers::list_orders),
        )
        .route("/orders/:id", get(handlers::get_order))
        .route("/orders/:id/status", patch(handlers::update_order_status))
        .layer(middleware::from_fn_with_state(
            jwt_secret,
            crate::auth::auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        // Outermost so every request (including ones rejected by auth below)
        // gets an id to quote.
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(state)
}

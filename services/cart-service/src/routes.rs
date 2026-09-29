use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use redis::Client;

use crate::handlers;

pub fn create_router(redis_client: Client) -> Router {
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let public_routes = Router::new().route("/health", get(handlers::health));

    let protected_routes = Router::new()
        .route(
            "/cart",
            get(handlers::get_cart).delete(handlers::clear_cart),
        )
        .route("/cart/items", post(handlers::add_item))
        .route("/cart/items/:product_id", delete(handlers::remove_item))
        .layer(middleware::from_fn_with_state(
            jwt_secret,
            crate::auth::auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(redis_client)
}

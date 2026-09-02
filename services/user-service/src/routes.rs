use axum::{middleware, routing::{get, post}, Router};
use sqlx::PgPool;

use crate::handlers;

pub fn create_router(pool: PgPool) -> Router {
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .route("/register", post(handlers::register))
        .route("/login", post(handlers::login));

    let protected_routes = Router::new()
        .route("/users/:id", get(handlers::get_user))
        .layer(middleware::from_fn_with_state(
            jwt_secret,
            crate::auth::auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(pool)
}

use sqlx::PgPool;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::clients::ServiceClient;
use crate::state::AppState;

mod auth;
mod clients;
mod db;
mod error;
mod handlers;
mod models;
mod routes;
mod state;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "order_service=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let pool = loop {
        match PgPool::connect(&database_url).await {
            Ok(pool) => break pool,
            Err(e) => {
                tracing::warn!("Database connection failed, retrying in 2s: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    };

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let client = ServiceClient::new(
        std::env::var("USER_SERVICE_URL").expect("USER_SERVICE_URL must be set"),
        std::env::var("PRODUCT_SERVICE_URL").expect("PRODUCT_SERVICE_URL must be set"),
        std::env::var("CART_SERVICE_URL").expect("CART_SERVICE_URL must be set"),
        std::env::var("PAYMENT_SERVICE_URL").expect("PAYMENT_SERVICE_URL must be set"),
        std::env::var("NOTIFICATION_SERVICE_URL").expect("NOTIFICATION_SERVICE_URL must be set"),
    );

    let state = AppState {
        pool,
        client,
        jwt_secret,
    };

    let app = routes::create_router(state).layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8084));
    tracing::info!("Order service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
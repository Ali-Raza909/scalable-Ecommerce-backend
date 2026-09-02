use axum::{routing::{get, post, delete}, Router};
use redis::Client;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod routes;
mod handlers;
mod models;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "cart_service=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let redis_url = std::env::var("REDIS_URL")
        .expect("REDIS_URL must be set");
    
    let redis_client = Client::open(redis_url)
        .expect("Failed to connect to Redis");

    let app = routes::create_router(redis_client);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8083));
    tracing::info!("Cart service listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
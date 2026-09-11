use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod error;
mod handlers;
mod models;
mod routes;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cart_service=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let client = redis::Client::open(redis_url).expect("Invalid Redis URL");

    loop {
        match client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                let pong: String = redis::cmd("PING")
                    .query_async(&mut conn)
                    .await
                    .unwrap_or_default();
                tracing::info!("Connected to Redis: {}", pong);
                break;
            }
            Err(e) => {
                tracing::warn!("Redis connection failed, retrying in 2s: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }

    let app = routes::create_router(client).layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8083));
    tracing::info!("Cart service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}


// cart service tested manually , and all tests passes jiooooooooo
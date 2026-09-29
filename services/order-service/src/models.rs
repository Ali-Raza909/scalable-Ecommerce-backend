use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub status: String,
    pub total_cents: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub total_cents: i32,
    pub created_at: DateTime<Utc>,
    pub items: Vec<OrderItem>,
}

impl OrderResponse {
    pub fn from_order(order: Order, items: Vec<OrderItem>) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            status: order.status,
            total_cents: order.total_cents,
            created_at: order.created_at,
            items,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub order: OrderResponse,
    pub checkout_url: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymentConfirmRequest {
    pub status: String,
}

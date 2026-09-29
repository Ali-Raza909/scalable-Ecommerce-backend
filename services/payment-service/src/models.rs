use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub payment_provider_ref: Option<String>,
    pub amount_cents: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub amount_cents: i32,
}

#[derive(Debug, Serialize)]
pub struct CreatePaymentResponse {
    pub payment: Payment,
    pub checkout_url: String,
}

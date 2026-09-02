use sqlx::PgPool;
use uuid::Uuid;
use crate::models::Payment;

pub async fn create_payment(pool: &PgPool, order_id: Uuid, stripe_payment_intent_id: &str, amount_cents: i32) -> Result<Payment, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "INSERT INTO payments (order_id, stripe_payment_intent_id, amount_cents, status) VALUES ($1, $2, $3, 'pending') RETURNING id, order_id, stripe_payment_intent_id, amount_cents, status, created_at"
    )
    .bind(order_id)
    .bind(stripe_payment_intent_id)
    .bind(amount_cents)
    .fetch_one(pool)
    .await
}

pub async fn update_payment_status(pool: &PgPool, payment_id: Uuid, status: &str) -> Result<Option<Payment>, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "UPDATE payments SET status = $1 WHERE id = $2 RETURNING id, order_id, stripe_payment_intent_id, amount_cents, status, created_at"
    )
    .bind(status)
    .bind(payment_id)
    .fetch_optional(pool)
    .await
}
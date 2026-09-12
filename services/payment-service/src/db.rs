use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Payment;

pub async fn create_payment(
    pool: &PgPool,
    order_id: Uuid,
    user_id: Uuid,
    amount_cents: i32,
) -> Result<Payment, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "INSERT INTO payments (order_id, user_id, amount_cents, status) \
         VALUES ($1, $2, $3, 'paid') \
         RETURNING id, order_id, user_id, stripe_payment_intent_id, amount_cents, status, created_at",
    )
    .bind(order_id)
    .bind(user_id)
    .bind(amount_cents)
    .fetch_one(pool)
    .await
}

#[allow(dead_code)]
pub async fn update_payment_status(
    pool: &PgPool,
    payment_id: Uuid,
    status: &str,
) -> Result<Option<Payment>, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "UPDATE payments SET status = $1 WHERE id = $2 \
         RETURNING id, order_id, user_id, stripe_payment_intent_id, amount_cents, status, created_at",
    )
    .bind(status)
    .bind(payment_id)
    .fetch_optional(pool)
    .await
}
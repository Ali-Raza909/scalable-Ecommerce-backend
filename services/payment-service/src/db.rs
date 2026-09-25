use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Payment;

pub async fn create_payment(
    pool: &PgPool,
    order_id: Uuid,
    user_id: Uuid,
    amount_cents: i32,
    provider_ref: &str,
) -> Result<Payment, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "INSERT INTO payments (order_id, user_id, payment_provider_ref, amount_cents, status) \
         VALUES ($1, $2, $3, $4, 'pending') \
         RETURNING id, order_id, user_id, payment_provider_ref, amount_cents, status, created_at",
    )
    .bind(order_id)
    .bind(user_id)
    .bind(provider_ref)
    .bind(amount_cents)
    .fetch_one(pool)
    .await
}

pub async fn get_payment_by_ref(
    pool: &PgPool,
    provider_ref: &str,
) -> Result<Option<Payment>, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "SELECT id, order_id, user_id, payment_provider_ref, amount_cents, status, created_at \
         FROM payments WHERE payment_provider_ref = $1",
    )
    .bind(provider_ref)
    .fetch_optional(pool)
    .await
}

pub async fn update_payment_status_by_ref(
    pool: &PgPool,
    provider_ref: &str,
    status: &str,
) -> Result<Option<Payment>, sqlx::Error> {
    sqlx::query_as::<_, Payment>(
        "UPDATE payments SET status = $1 WHERE payment_provider_ref = $2 \
         RETURNING id, order_id, user_id, payment_provider_ref, amount_cents, status, created_at",
    )
    .bind(status)
    .bind(provider_ref)
    .fetch_optional(pool)
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
         RETURNING id, order_id, user_id, payment_provider_ref, amount_cents, status, created_at",
    )
    .bind(status)
    .bind(payment_id)
    .fetch_optional(pool)
    .await
}
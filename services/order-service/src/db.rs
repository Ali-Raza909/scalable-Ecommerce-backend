use sqlx::PgPool;
use uuid::Uuid;

use chrono::{DateTime, Utc};

use crate::models::{Order, OrderItem};

pub struct NewOrderItem {
    pub product_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i32,
}

pub async fn get_order_by_id(pool: &PgPool, order_id: Uuid) -> Result<Option<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "SELECT id, user_id, email, status, total_cents, created_at FROM orders WHERE id = $1",
    )
    .bind(order_id)
    .fetch_optional(pool)
    .await
}

pub async fn get_order_items(pool: &PgPool, order_id: Uuid) -> Result<Vec<OrderItem>, sqlx::Error> {
    sqlx::query_as::<_, OrderItem>(
        "SELECT id, order_id, product_id, quantity, unit_price_cents \
         FROM order_items WHERE order_id = $1",
    )
    .bind(order_id)
    .fetch_all(pool)
    .await
}

pub async fn get_orders_by_user_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "SELECT id, user_id, email, status, total_cents, created_at FROM orders \
         WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn create_order_with_items(
    pool: &PgPool,
    user_id: Uuid,
    email: &str,
    total_cents: i32,
    items: &[NewOrderItem],
) -> Result<(Order, Vec<OrderItem>), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let order = sqlx::query_as::<_, Order>(
        "INSERT INTO orders (user_id, email, total_cents) VALUES ($1, $2, $3) \
         RETURNING id, user_id, email, status, total_cents, created_at",
    )
    .bind(user_id)
    .bind(email)
    .bind(total_cents)
    .fetch_one(&mut *tx)
    .await?;

    let mut order_items = Vec::with_capacity(items.len());
    for item in items {
        let order_item = sqlx::query_as::<_, OrderItem>(
            "INSERT INTO order_items (order_id, product_id, quantity, unit_price_cents) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, order_id, product_id, quantity, unit_price_cents",
        )
        .bind(order.id)
        .bind(item.product_id)
        .bind(item.quantity)
        .bind(item.unit_price_cents)
        .fetch_one(&mut *tx)
        .await?;
        order_items.push(order_item);
    }

    tx.commit().await?;

    Ok((order, order_items))
}

pub async fn update_order_status(
    pool: &PgPool,
    order_id: Uuid,
    status: &str,
) -> Result<Option<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "UPDATE orders SET status = $1 WHERE id = $2 \
         RETURNING id, user_id, status, total_cents, created_at",
    )
    .bind(status)
    .bind(order_id)
    .fetch_optional(pool)
    .await
}

pub async fn mark_order_paid_if_pending(
    pool: &PgPool,
    order_id: Uuid,
) -> Result<Option<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "UPDATE orders SET status = 'paid' WHERE id = $1 AND status = 'pending' \
         RETURNING id, user_id, email, status, total_cents, created_at",
    )
    .bind(order_id)
    .fetch_optional(pool)
    .await
}

pub async fn mark_order_cancelled_if_pending(
    pool: &PgPool,
    order_id: Uuid,
) -> Result<Option<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "UPDATE orders SET status = 'cancelled' WHERE id = $1 AND status = 'pending' \
         RETURNING id, user_id, email, status, total_cents, created_at",
    )
    .bind(order_id)
    .fetch_optional(pool)
    .await
}

pub async fn get_stale_pending_orders(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM orders WHERE status = 'pending' AND created_at < $1")
        .bind(cutoff)
        .fetch_all(pool)
        .await
}

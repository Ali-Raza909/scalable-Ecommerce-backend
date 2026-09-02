use sqlx::PgPool;
use uuid::Uuid;
use crate::models::{Order, OrderItem};

pub async fn get_order_by_id(pool: &PgPool, order_id: Uuid) -> Result<Option<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "SELECT id, user_id, status, total_cents, created_at FROM orders WHERE id = $1"
    )
    .bind(order_id)
    .fetch_optional(pool)
    .await
}

pub async fn get_orders_by_user_id(pool: &PgPool, user_id: Uuid) -> Result<Vec<Order>, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "SELECT id, user_id, status, total_cents, created_at FROM orders WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn create_order(pool: &PgPool, user_id: Uuid, total_cents: i32) -> Result<Order, sqlx::Error> {
    sqlx::query_as::<_, Order>(
        "INSERT INTO orders (user_id, total_cents) VALUES ($1, $2) RETURNING id, user_id, status, total_cents, created_at"
    )
    .bind(user_id)
    .bind(total_cents)
    .fetch_one(pool)
    .await
}

pub async fn create_order_item(pool: &PgPool, order_id: Uuid, product_id: Uuid, quantity: i32, unit_price_cents: i32) -> Result<OrderItem, sqlx::Error> {
    sqlx::query_as::<_, OrderItem>(
        "INSERT INTO order_items (order_id, product_id, quantity, unit_price_cents) VALUES ($1, $2, $3, $4) RETURNING id, order_id, product_id, quantity, unit_price_cents"
    )
    .bind(order_id)
    .bind(product_id)
    .bind(quantity)
    .bind(unit_price_cents)
    .fetch_one(pool)
    .await
}
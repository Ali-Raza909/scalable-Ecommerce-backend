use sqlx::PgPool;
use uuid::Uuid;
use crate::models::Product;

pub async fn get_product_by_id(pool: &PgPool, product_id: Uuid) -> Result<Option<Product>, sqlx::Error> {
    sqlx::query_as::<_, Product>(
        "SELECT id, category_id, name, description, price_cents, stock_quantity, created_at FROM products WHERE id = $1"
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_stock(pool: &PgPool, product_id: Uuid, delta: i32) -> Result<Option<Product>, sqlx::Error> {
    sqlx::query_as::<_, Product>(
        "UPDATE products SET stock_quantity = stock_quantity + $1 WHERE id = $2 AND stock_quantity + $1 >= 0 RETURNING id, category_id, name, description, price_cents, stock_quantity, created_at"
    )
    .bind(delta)
    .bind(product_id)
    .fetch_optional(pool)
    .await
}
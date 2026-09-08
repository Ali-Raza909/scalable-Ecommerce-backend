use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Product;

pub async fn get_product_by_id(
    pool: &PgPool,
    product_id: Uuid,
) -> Result<Option<Product>, sqlx::Error> {
    sqlx::query_as::<_, Product>(
        "SELECT id, category_id, name, description, price_cents, stock_quantity, created_at \
         FROM products WHERE id = $1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_products(
    pool: &PgPool,
    category_id: Option<Uuid>,
    offset: i32,
    limit: i32,
) -> Result<Vec<Product>, sqlx::Error> {
    if let Some(cat_id) = category_id {
        sqlx::query_as::<_, Product>(
            "SELECT id, category_id, name, description, price_cents, stock_quantity, created_at \
             FROM products WHERE category_id = $1 \
             ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(cat_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, Product>(
            "SELECT id, category_id, name, description, price_cents, stock_quantity, created_at \
             FROM products ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}

pub async fn count_products(pool: &PgPool, category_id: Option<Uuid>) -> Result<i64, sqlx::Error> {
    if let Some(cat_id) = category_id {
        sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE category_id = $1")
            .bind(cat_id)
            .fetch_one(pool)
            .await
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(pool)
            .await
    }
}

pub async fn create_product(
    pool: &PgPool,
    category_id: Uuid,
    name: &str,
    description: Option<&str>,
    price_cents: i32,
    stock_quantity: i32,
) -> Result<Product, sqlx::Error> {
    sqlx::query_as::<_, Product>(
        "INSERT INTO products (category_id, name, description, price_cents, stock_quantity) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, category_id, name, description, price_cents, stock_quantity, created_at",
    )
    .bind(category_id)
    .bind(name)
    .bind(description)
    .bind(price_cents)
    .bind(stock_quantity)
    .fetch_one(pool)
    .await
}

pub async fn update_stock(
    pool: &PgPool,
    product_id: Uuid,
    delta: i32,
) -> Result<Option<Product>, sqlx::Error> {
    sqlx::query_as::<_, Product>(
        "UPDATE products SET stock_quantity = stock_quantity + $1 \
         WHERE id = $2 AND stock_quantity + $1 >= 0 \
         RETURNING id, category_id, name, description, price_cents, stock_quantity, created_at",
    )
    .bind(delta)
    .bind(product_id)
    .fetch_optional(pool)
    .await
}

pub async fn ensure_category_exists(pool: &PgPool, category_id: Uuid) -> Result<bool, sqlx::Error> {
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
        .bind(category_id)
        .fetch_optional(pool)
        .await?;
    Ok(exists.is_some())
}

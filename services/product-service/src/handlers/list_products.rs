use axum::{
    extract::{Query, State},
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::{ListProductsQuery, PaginatedProducts};

const DEFAULT_PAGE: i32 = 1;
const DEFAULT_LIMIT: i32 = 20;
const MAX_LIMIT: i32 = 100;

pub async fn list_products(
    State(pool): State<PgPool>,
    Query(query): Query<ListProductsQuery>,
) -> Result<Json<PaginatedProducts>, AppError> {
    let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    let limit = if limit < 1 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    };
    let offset = (page - 1) * limit;

    let category_id = match query.category {
        Some(cat) => Some(
            Uuid::parse_str(&cat)
                .map_err(|_| AppError::BadRequest("Invalid category id".to_string()))?,
        ),
        None => None,
    };

    let items = db::list_products(&pool, category_id, offset, limit).await?;
    let total = db::count_products(&pool, category_id).await?;

    Ok(Json(PaginatedProducts {
        items,
        total,
        page,
        limit,
    }))
}

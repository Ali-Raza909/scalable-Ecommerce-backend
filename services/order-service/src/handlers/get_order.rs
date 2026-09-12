use axum::{extract::{Extension, Path, State}, Json};
use common::Claims;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::OrderResponse;
use crate::state::AppState;

pub async fn get_order(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderResponse>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user id in token".to_string()))?;

    let order = db::get_order_by_id(&state.pool, order_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if order.user_id != user_id {
        return Err(AppError::NotFound);
    }

    let items = db::get_order_items(&state.pool, order.id).await?;

    Ok(Json(OrderResponse::from_order(order, items)))
}
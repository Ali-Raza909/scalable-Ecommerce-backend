use axum::{
    extract::{Extension, State},
    Json,
};
use common::Claims;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::OrderResponse;
use crate::state::AppState;

pub async fn list_orders(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<OrderResponse>>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user id in token".to_string()))?;

    let orders = db::get_orders_by_user_id(&state.pool, user_id).await?;

    let mut responses = Vec::with_capacity(orders.len());
    for order in orders {
        let items = db::get_order_items(&state.pool, order.id).await?;
        responses.push(OrderResponse::from_order(order, items));
    }

    Ok(Json(responses))
}

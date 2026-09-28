use axum::{extract::{Extension, Path, State}, Json};
use common::Claims;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::handlers::compensation;
use crate::models::{Order, UpdateStatusRequest};
use crate::state::AppState;

fn valid_next_status(current: &str, requested: &str) -> bool {
    let allowed: &[&str] = match current {
        "pending" => &["paid", "cancelled"],
        "paid" => &["shipped", "cancelled"],
        "shipped" => &["delivered"],
        _ => &[],
    };
    allowed.contains(&requested)
}

pub async fn update_order_status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<UpdateStatusRequest>,
) -> Result<Json<Order>, AppError> {
    if claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    let order = db::get_order_by_id(&state.pool, order_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if !valid_next_status(&order.status, &payload.status) {
        return Err(AppError::BadRequest(format!(
            "Cannot transition from {} to {}",
            order.status, payload.status
        )));
    }

    // Cancelling an unpaid order must release its reserved stock. Guarded so a
    // double PATCH restores inventory exactly once. paid -> cancelled is a
    // refund path and intentionally does not touch inventory.
    if order.status == "pending" && payload.status == "cancelled" {
        let updated =
            match compensation::cancel_order_and_restore_stock(&state, order_id).await? {
                Some(o) => o,
                None => order,
            };
        return Ok(Json(updated));
    }

    let updated = db::update_order_status(&state.pool, order_id, &payload.status)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(updated))
}
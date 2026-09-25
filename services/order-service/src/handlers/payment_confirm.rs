use axum::{extract::{Path, State}, http::StatusCode, Json};
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::PaymentConfirmRequest;
use crate::state::AppState;

pub async fn payment_confirm(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(body): Json<PaymentConfirmRequest>,
) -> Result<StatusCode, AppError> {
    if body.status != "paid" {
        tracing::warn!("Ignoring payment-confirm with unexpected status '{}'", body.status);
        return Ok(StatusCode::OK);
    }

    // guarded and idempotent: only flips pending -> paid; duplicate webhooks no-op.
    let order = match db::mark_order_paid_if_pending(&state.pool, order_id).await? {
        Some(o) => o,
        None => {
            tracing::info!("payment-confirm no-op for order {} (already paid/cancelled)", order_id);
            return Ok(StatusCode::OK);
        }
    };

    // payment confirmed; now send the order confirmation (best-effort).
    if let Err(e) = state
        .client
        .send_notification(order.id, order.user_id, &order.email)
        .await
    {
        tracing::warn!(
            "Failed to send notification for order {}: {:?}",
        order.id, e
        );
    }

    Ok(StatusCode::OK)
}
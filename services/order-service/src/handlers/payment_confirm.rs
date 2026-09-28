use axum::{extract::{Path, State}, http::StatusCode, Json};
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::handlers::compensation;
use crate::models::PaymentConfirmRequest;
use crate::state::AppState;

pub async fn payment_confirm(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(body): Json<PaymentConfirmRequest>,
) -> Result<StatusCode, AppError> {
    match body.status.as_str() {
        "paid" => {
            // guarded and idempotent: only flips pending -> paid; duplicate webhooks no-op.
            let order = match db::mark_order_paid_if_pending(&state.pool, order_id).await? {
                Some(o) => o,
                None => {
                    tracing::info!(
                        "payment-confirm no-op for order {} (already paid/cancelled)",
                        order_id
                    );
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
                    order.id,
                    e
                );
            }
        }
        "cancelled" => {
            // guarded + idempotent: first pending -> cancelled transition also
            // restores stock; retried/double-cancelled webhooks are no-ops.
            match compensation::cancel_order_and_restore_stock(&state, order_id).await? {
                Some(_) => {
                    tracing::info!("Order {} cancelled and stock restored", order_id);
                }
                None => {
                    tracing::info!(
                        "payment-confirm no-op for order {} (already paid/cancelled)",
                        order_id
                    );
                }
            }
        }
        other => {
            tracing::warn!(
                "Ignoring payment-confirm with unexpected status '{}'",
                other
            );
        }
    }

    Ok(StatusCode::OK)
}
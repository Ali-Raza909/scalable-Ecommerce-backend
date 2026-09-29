use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
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
) -> Result<(StatusCode, Json<Value>), AppError> {
    // Respond with the order's resulting status so the caller (payment-service)
    // can tell a genuine flip from a no-op -- e.g. a late "paid" for an order
    // the timeout job already cancelled must be surfaced, not silently OK'd.
    match body.status.as_str() {
        "paid" => {
            let flipped = match db::mark_order_paid_if_pending(&state.pool, order_id).await? {
                Some(o) => Some(o),
                None => {
                    tracing::info!(
                        "payment-confirm no-op for order {} (already paid/cancelled)",
                        order_id
                    );
                    None
                }
            };

            if let Some(order) = &flipped {
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

            let current = match &flipped {
                Some(o) => o.status.clone(),
                None => status_of(&state, order_id).await,
            };
            Ok((StatusCode::OK, Json(json!({ "order_status": current }))))
        }
        "cancelled" => {
            // guarded + idempotent: first pending -> cancelled transition also
            // restores stock; retried/double-cancelled webhooks are no-ops.
            match compensation::cancel_order_and_restore_stock(&state, order_id).await? {
                Some(o) => {
                    tracing::info!("Order {} cancelled and stock restored", order_id);
                    Ok((StatusCode::OK, Json(json!({ "order_status": o.status }))))
                }
                None => {
                    tracing::info!(
                        "payment-confirm no-op for order {} (already paid/cancelled)",
                        order_id
                    );
                    let current = status_of(&state, order_id).await;
                    Ok((StatusCode::OK, Json(json!({ "order_status": current }))))
                }
            }
        }
        other => {
            tracing::warn!(
                "Ignoring payment-confirm with unexpected status '{}'",
                other
            );
            let current = status_of(&state, order_id).await;
            Ok((StatusCode::OK, Json(json!({ "order_status": current }))))
        }
    }
}

async fn status_of(state: &AppState, order_id: Uuid) -> String {
    match db::get_order_by_id(&state.pool, order_id).await {
        Ok(Some(o)) => o.status,
        Ok(None) => "missing".to_string(),
        Err(e) => {
            tracing::error!("Failed to read order {} status: {:?}", order_id, e);
            "unknown".to_string()
        }
    }
}

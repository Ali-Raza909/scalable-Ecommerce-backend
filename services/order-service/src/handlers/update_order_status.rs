use axum::{
    extract::{Extension, Path, State},
    Json,
};
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
        // paid -> cancelled is intentionally BLOCKED: cancelling a paid order
        // without a real (Safepay) refund would strand the customer's money
        // while either holding stock or reselling it. Refunds need the refund
        // API + a new state machine branch; that is a feature of its own.
        "paid" => &["shipped"],
        "shipped" => &["delivered"],
        _ => &[],
    };
    allowed.contains(&requested)
}

/// The only endpoint carrying admin powers is the status PATCH; anything else
/// the system allows a normal user to do is not in scope for role gating.
fn is_admin(claims: &Claims) -> bool {
    claims.role == "admin"
}

pub async fn update_order_status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<UpdateStatusRequest>,
) -> Result<Json<Order>, AppError> {
    if !is_admin(&claims) {
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
    // double PATCH restores inventory exactly once.
    if order.status == "pending" && payload.status == "cancelled" {
        let updated = match compensation::cancel_order_and_restore_stock(&state, order_id).await? {
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

#[cfg(test)]
mod tests {
    use super::{is_admin, valid_next_status};
    use common::Claims;

    fn claims(role: &str) -> Claims {
        Claims {
            sub: "00000000-0000-4000-8000-000000000001".into(),
            email: "user@example.com".into(),
            role: role.into(),
            exp: 9_999_999_999,
            iat: 1_700_000_000,
        }
    }

    #[test]
    fn only_admin_role_passes_the_gate() {
        assert!(is_admin(&claims("admin")));
        assert!(!is_admin(&claims("user")));
        assert!(!is_admin(&claims("")));
        assert!(!is_admin(&claims("ADMIN")));
    }

    #[test]
    fn pending_may_become_paid_or_cancelled() {
        assert!(valid_next_status("pending", "paid"));
        assert!(valid_next_status("pending", "cancelled"));
    }

    #[test]
    fn paid_may_only_be_shipped_never_cancelled() {
        // paid -> cancelled is deliberately blocked (refund semantics), and
        // delivery/skipping states are not allowed from paid.
        assert!(valid_next_status("paid", "shipped"));
        assert!(!valid_next_status("paid", "cancelled"));
        assert!(!valid_next_status("paid", "delivered"));
        assert!(!valid_next_status("paid", "paid"));
    }

    #[test]
    fn shipped_may_only_be_delivered() {
        assert!(valid_next_status("shipped", "delivered"));
        assert!(!valid_next_status("shipped", "cancelled"));
        assert!(!valid_next_status("shipped", "paid"));
        assert!(!valid_next_status("shipped", "shipped"));
    }

    #[test]
    fn delivered_has_no_further_transitions() {
        assert!(!valid_next_status("delivered", "shipped"));
        assert!(!valid_next_status("delivered", "cancelled"));
        assert!(!valid_next_status("delivered", "paid"));
        assert!(!valid_next_status("delivered", "delivered"));
    }

    #[test]
    fn unknown_current_state_rejects_everything() {
        assert!(!valid_next_status("refunded", "cancelled"));
        assert!(!valid_next_status("", "paid"));
    }

    #[test]
    fn rejected_transitions_are_symmetric_reversals() {
        // No backward moves through the state machine.
        assert!(!valid_next_status("paid", "pending"));
        assert!(!valid_next_status("cancelled", "pending"));
        assert!(!valid_next_status("delivered", "shipped"));
    }
}

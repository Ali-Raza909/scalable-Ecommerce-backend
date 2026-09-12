use axum::{extract::{Extension, State}, Json};
use common::Claims;
use uuid::Uuid;

use crate::clients::Product;
use crate::db::{self, NewOrderItem};
use crate::error::AppError;
use crate::models::OrderResponse;
use crate::state::AppState;

struct ReservedStock {
    product_id: Uuid,
    quantity: i32,
}

pub async fn create_order(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(token): Extension<String>,
) -> Result<Json<OrderResponse>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user id in token".to_string()))?;

    // 1. Validate the user exists (User Service).
    state.client.validate_user(user_id, &token).await?;

    // 2. Fetch the cart (Cart Service).
    let cart = state.client.get_cart(&token).await?;
    if cart.items.is_empty() {
        return Err(AppError::BadRequest("Cart is empty".to_string()));
    }

    // 3. Reserve stock for every item (Product Service). On any failure,
    //    roll back the stock we already reserved.
    let mut reserved: Vec<ReservedStock> = Vec::new();
    let mut order_items: Vec<NewOrderItem> = Vec::new();
    let mut total_cents: i32 = 0;

    for item in &cart.items {
        let product_id = Uuid::parse_str(&item.product_id).map_err(|_| {
            AppError::BadRequest(format!("Invalid product id '{}' in cart", item.product_id))
        })?;

        let product: Product = match state.client.decrement_stock(product_id, item.quantity).await {
            Ok(p) => p,
            Err(e) => {
                restore_stock(&state, &reserved).await;
                return Err(e);
            }
        };

        reserved.push(ReservedStock {
            product_id,
            quantity: item.quantity,
        });
        total_cents += product.price_cents * item.quantity;
        order_items.push(NewOrderItem {
            product_id,
            quantity: item.quantity,
            unit_price_cents: product.price_cents,
        });
    }

    // 4. Persist the order as pending (own database, single transaction).
    let (order, items) = db::create_order_with_items(&state.pool, user_id, total_cents, &order_items)
        .await?;

    // 5. Charge the payment (Payment Service). On failure, compensate:
    //    restore stock and mark the order cancelled.
    if let Err(e) = state.client.request_payment(order.id, user_id, total_cents).await {
        tracing::error!("Payment failed for order {}: {:?}", order.id, e);
        compensate(&state, &reserved, order.id).await;
        return Err(e);
    }

    // 6. Payment is committed (point of no return). Record it: pending -> paid.
    //    Failures from here on warn-log only; there is nothing to roll back.
    let order = match db::update_order_status(&state.pool, order.id, "paid").await {
        Ok(Some(updated)) => updated,
        Ok(None) => {
            tracing::warn!("Order {} vanished after payment", order.id);
            order
        }
        Err(e) => {
            tracing::warn!("Failed to mark order {} paid: {:?}", order.id, e);
            order
        }
    };

    // 7. Clear the cart and notify (both best-effort: payment already succeeded).
    if let Err(e) = state.client.clear_cart(&token).await {
        tracing::warn!("Failed to clear cart for user {}: {:?}", user_id, e);
    }
    if let Err(e) = state.client.send_notification(order.id, user_id, &claims.email).await {
        tracing::warn!("Failed to send notification for order {}: {:?}", order.id, e);
    }

    Ok(Json(OrderResponse::from_order(order, items)))
}

async fn compensate(state: &AppState, reserved: &[ReservedStock], order_id: Uuid) {
    restore_stock(state, reserved).await;
    if let Err(e) = db::update_order_status(&state.pool, order_id, "cancelled").await {
        tracing::error!("Failed to mark order {} cancelled: {:?}", order_id, e);
    }
}

async fn restore_stock(state: &AppState, reserved: &[ReservedStock]) {
    for r in reserved {
        if let Err(e) = state.client.restore_stock(r.product_id, r.quantity).await {
            tracing::error!(
                "Failed to restore stock for product {}: {:?}",
                r.product_id,
                e
            );
        }
    }
}
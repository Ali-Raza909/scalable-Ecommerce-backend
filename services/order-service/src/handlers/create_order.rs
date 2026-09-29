use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use common::Claims;
use uuid::Uuid;

use crate::clients::Product;
use crate::db::{self, NewOrderItem};
use crate::error::AppError;
use crate::handlers::compensation::{self, StockUnit};
use crate::models::{CheckoutResponse, OrderResponse};
use crate::state::AppState;

pub async fn create_order(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(token): Extension<String>,
) -> Result<(StatusCode, Json<CheckoutResponse>), AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user id in token".to_string()))?;

    //  validate the user exists through user service
    state.client.validate_user(user_id, &token).await?;

    // fetch the cart using cart service
    let cart = state.client.get_cart(&token).await?;
    if cart.items.is_empty() {
        return Err(AppError::BadRequest("Cart is empty".to_string()));
    }

    // reserve stock for every item using product service, and in case of any failure, roll back the reserved stock and return an error

    let mut reserved: Vec<StockUnit> = Vec::new();
    let mut order_items: Vec<NewOrderItem> = Vec::new();
    let mut total_cents: i32 = 0;

    for item in &cart.items {
        let product_id = Uuid::parse_str(&item.product_id).map_err(|_| {
            AppError::BadRequest(format!("Invalid product id '{}' in cart", item.product_id))
        })?;

        let product: Product = match state
            .client
            .decrement_stock(product_id, item.quantity)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                compensation::restore_stock(&state, &reserved).await;
                return Err(e);
            }
        };

        reserved.push(StockUnit {
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

    // persist the order as pending (own database, single transaction); on
    // failure the reservations must be released too.
    let (order, items) = match db::create_order_with_items(
        &state.pool,
        user_id,
        &claims.email,
        total_cents,
        &order_items,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            compensation::restore_stock(&state, &reserved).await;
            return Err(e.into());
        }
    };

    // request an async payment via payment service; if it fails, cancel the order and restore the reserved stock

    let checkout_url = match state
        .client
        .request_payment(order.id, user_id, total_cents)
        .await
    {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("Payment init failed for order {}: {:?}", order.id, e);
            if let Err(ce) = compensation::cancel_order_and_restore_stock(&state, order.id).await {
                tracing::error!(
                    "Failed to cancel order {} and restore stock: {:?}",
                    order.id,
                    ce
                );
            }
            return Err(e);
        }
    };

    // order stays 'pending'; the webhook-driven payment-confirm marks it paid (or cancelled).
    // cart is cleared now (only the caller's token can do this); notification
    // moves to payment-confirm so it only fires after payment actually succeeds.

    if let Err(e) = state.client.clear_cart(&token).await {
        tracing::warn!("Failed to clear cart for user {}: {:?}", user_id, e);
    }

    Ok((
        StatusCode::CREATED,
        Json(CheckoutResponse {
            order: OrderResponse::from_order(order, items),
            checkout_url,
        }),
    ))
}

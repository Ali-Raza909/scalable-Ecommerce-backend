use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::Order;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct StockUnit {
    pub product_id: Uuid,
    pub quantity: i32,
}

pub async fn restore_stock(state: &AppState, units: &[StockUnit]) {
    for unit in units {
        if let Err(e) = state
            .client
            .restore_stock(unit.product_id, unit.quantity)
            .await
        {
            tracing::error!(
                "Failed to restore stock for product {}: {:?}",
                unit.product_id,
                e
            );
        }
    }
}

pub async fn cancel_order_and_restore_stock(
    state: &AppState,
    order_id: Uuid,
) -> Result<Option<Order>, AppError> {
    // guarded: only the pending -> cancelled transition returns a row, so a
    // retried/double-cancelled order restores its stock exactly once.
    let order = db::mark_order_cancelled_if_pending(&state.pool, order_id).await?;

    if order.is_some() {
        let items = db::get_order_items(&state.pool, order_id).await?;
        let units: Vec<StockUnit> = items
            .iter()
            .map(|item| StockUnit {
                product_id: item.product_id,
                quantity: item.quantity,
            })
            .collect();
        restore_stock(state, &units).await;
    }

    Ok(order)
}
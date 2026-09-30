use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::{Order, OrderItem};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct StockUnit {
    pub product_id: Uuid,
    pub quantity: i32,
}

/// Maps order items onto the stock units they must restore. Pure so the
/// mapping contract (product_id + quantity pass-through) is unit-testable
/// independent of the DB.
pub fn order_items_to_units(items: &[OrderItem]) -> Vec<StockUnit> {
    items
        .iter()
        .map(|item| StockUnit {
            product_id: item.product_id,
            quantity: item.quantity,
        })
        .collect()
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
        let units = order_items_to_units(&items);
        restore_stock(state, &units).await;
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::order_items_to_units;
    use crate::models::OrderItem;
    use uuid::Uuid;

    fn item(product_id: Uuid, quantity: i32) -> OrderItem {
        OrderItem {
            id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            product_id,
            quantity,
            unit_price_cents: 0,
        }
    }

    #[test]
    fn maps_product_id_and_quantity() {
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let units = order_items_to_units(&[item(p1, 2), item(p2, 5)]);

        assert_eq!(units.len(), 2);
        assert_eq!(units[0].product_id, p1);
        assert_eq!(units[0].quantity, 2);
        assert_eq!(units[1].product_id, p2);
        assert_eq!(units[1].quantity, 5);
    }

    #[test]
    fn empty_items_yield_no_units() {
        assert!(order_items_to_units(&[]).is_empty());
    }
}

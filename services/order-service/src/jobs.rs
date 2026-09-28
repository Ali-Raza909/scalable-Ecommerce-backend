use std::time::Duration;

use chrono::Utc;
use tokio::time;

use crate::db;
use crate::handlers::compensation;
use crate::state::AppState;

pub fn spawn_pending_order_timeout(state: AppState, grace: Duration, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = time::interval(interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;

            let cutoff = Utc::now() - chrono::Duration::from_std(grace).unwrap_or_default();
            match db::get_stale_pending_orders(&state.pool, cutoff).await {
                Ok(ids) => {
                    for id in ids {
                        // guarded + idempotent: a cancelled order (webhook, admin,
                        // earlier sweep) is a no-op here and restores stock once.
                        match compensation::cancel_order_and_restore_stock(&state, id).await {
                            Ok(Some(_)) => tracing::info!(
                                "Timeout: order {} past grace period; cancelled and stock restored",
                                id
                            ),
                            Ok(None) => {}
                            Err(e) => tracing::error!(
                                "Timeout: failed to cancel stale order {}: {:?}",
                                id,
                                e
                            ),
                        }
                    }
                }
                Err(e) => tracing::error!("Timeout sweep DB error: {:?}", e),
            }
        }
    });
}
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time;

use crate::db;
use crate::handlers::compensation;
use crate::state::AppState;

/// Orders with `created_at` before this instant are past the grace period.
fn stale_cutoff(now: DateTime<Utc>, grace: Duration) -> DateTime<Utc> {
    // A duration too large for chrono (or a zero/negative one) degrades to "now",
    // which either times everything out immediately or enforces no timeout.
    now - chrono::Duration::from_std(grace).unwrap_or_default()
}

pub fn spawn_pending_order_timeout(state: AppState, grace: Duration, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = time::interval(interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;

            let cutoff = stale_cutoff(Utc::now(), grace);
            match db::get_stale_pending_orders(&state.pool, cutoff).await {
                Ok(ids) => {
                    for id in ids {
                        // guarded + idempotent: a cancelled order (webhook, admin,
                        // earlier sweep) is a no-op here and restores stock once.
                        // Background task: no request context to propagate.
                        match compensation::cancel_order_and_restore_stock(&state, id, None).await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDateTime, TimeZone};

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(&NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap(),
            chrono::NaiveTime::from_hms_opt(h, mi, s).unwrap(),
        ))
    }

    #[test]
    fn cutoff_moves_back_by_exactly_the_grace() {
        let now = at(2026, 9, 28, 16, 0, 0);
        let cutoff = stale_cutoff(now, Duration::from_secs(15 * 60));
        assert_eq!(cutoff, at(2026, 9, 28, 15, 45, 0));
    }

    #[test]
    fn zero_grace_times_out_everything_immediately() {
        let now = at(2026, 9, 28, 16, 0, 0);
        assert_eq!(stale_cutoff(now, Duration::ZERO), now);
    }

    #[test]
    fn grace_larger_than_chrono_can_represent_degrades_to_now() {
        // 2^64 seconds exceeds chrono's range -> from_std fails -> no timeout.
        let now = at(2026, 9, 28, 16, 0, 0);
        assert_eq!(stale_cutoff(now, Duration::from_secs(u64::MAX)), now);
    }

    #[test]
    fn sub_second_grace_rounds_near_now() {
        let now = at(2026, 9, 28, 16, 0, 0);
        let cutoff = stale_cutoff(now, Duration::from_millis(500));
        // chrono.nanosecond resolution subtraction; must be just before now.
        assert_eq!(now - cutoff, chrono::Duration::milliseconds(500));
    }
}

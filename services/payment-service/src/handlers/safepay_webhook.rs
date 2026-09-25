use axum::{extract::State, http::StatusCode, body::Bytes, http::HeaderMap};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::db;
use crate::error::AppError;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

pub async fn safepay_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    // TEMPORARY defensive log: confirm real payload shape before trusting
    // the state/token field extraction below (Safepay docs are thin here).
    tracing::info!("Raw Safepay webhook payload: {}", String::from_utf8_lossy(&body));

    let signature = headers
        .get("X-SFPY-SIGNATURE")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing signature".into()))?;

    let secret =
        std::env::var("SAFEPAY_WEBHOOK_SECRET").expect("SAFEPAY_WEBHOOK_SECRET not set");

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| {
            tracing::error!("Invalid webhook secret: {}", e);
            AppError::PaymentFailed
        })?;
    mac.update(&body);
    let expected = hex::encode(mac.finalize().into_bytes());

    if expected != signature {
        tracing::warn!("Webhook signature mismatch");
        return Err(AppError::Unauthorized);
    }

    let event: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("Invalid JSON".into()))?;

    let state_field = event["data"]["state"].as_str().unwrap_or("");
    let token = event["data"]["token"].as_str().unwrap_or("");

    match state_field {
        "TRACKER_ENDED" | "PAID" => {
            let updated = db::update_payment_status_by_ref(&state.pool, token, "succeeded").await?;
            if updated.is_some() {
                notify_order_confirmed(&state, token).await;
            }
        }
        "FAILED" | "CANCELLED" => {
            db::update_payment_status_by_ref(&state.pool, token, "failed").await?;
        }
        other => {
            tracing::info!("Unhandled Safepay webhook state: {}", other);
        }
    }

    Ok(StatusCode::OK)
}

async fn notify_order_confirmed(state: &AppState, token: &str) {
    let order_id = match db::get_payment_by_ref(&state.pool, token).await {
        Ok(Some(p)) => p.order_id,
        _ => {
            tracing::warn!("Webhook token not found: {}", token);
            return;
        }
    };

    let order_service_url =
        std::env::var("ORDER_SERVICE_URL").expect("ORDER_SERVICE_URL not set");

    match state
        .http
        .post(format!("{}/orders/{}/payment-confirm", order_service_url, order_id))
        .json(&serde_json::json!({ "status": "paid" }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {}
        Ok(resp) => tracing::warn!(
            "Order confirm failed for order {}: status {}",
            order_id,
            resp.status()
        ),
        Err(e) => tracing::warn!(
            "Order confirm failed for order {}: {}",
            order_id,
            e
        ),
    }
}
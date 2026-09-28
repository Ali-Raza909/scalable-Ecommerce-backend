use axum::{body::Bytes, extract::State, http::{HeaderMap, StatusCode}};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha512};

use crate::db;
use crate::error::AppError;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

fn hex_hmac_sha256(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

fn hex_hmac_sha512(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha512::new_from_slice(key).expect("hmac key");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

fn verify_signature(signature: &str, headers: &HeaderMap, body: &[u8], secret: &[u8]) -> bool {
    let variants: Vec<String> = vec![
        hex_hmac_sha512(secret, body),
        hex_hmac_sha256(secret, body),
    ];

    let scheme_c = headers
        .get("X-SFPY-TIMESTAMP")
        .and_then(|v| v.to_str().ok())
        .map(|ts| {
            let mut signed = Vec::with_capacity(ts.len() + 1 + body.len());
            signed.extend_from_slice(ts.as_bytes());
            signed.push(b'.');
            signed.extend_from_slice(body);
            signed
        }).map(|mut signed_bytes| {
            let key = STANDARD.decode(secret).unwrap_or_default();
            if key.is_empty() {
                return None;
            }
            signed_bytes = std::mem::take(&mut signed_bytes);
            Some(hex_hmac_sha256(&key, &signed_bytes))
        })
        .flatten();

    if let Some(c) = scheme_c {
        if c == signature {
            return true;
        }
    }

    variants.iter().any(|v| v == signature)
}

pub async fn safepay_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    // TEMPORARY defensive log: confirm real payload shape before trusting
    // the event extraction below (Safepay docs are thin here).
    tracing::info!("Raw Safepay webhook payload: {}", String::from_utf8_lossy(&body));

    let signature = headers
        .get("X-SFPY-SIGNATURE")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing signature".into()))?;

    let secret =
        std::env::var("SAFEPAY_WEBHOOK_SECRET").expect("SAFEPAY_WEBHOOK_SECRET not set");

    if !verify_signature(signature, &headers, &body, secret.as_bytes()) {
        tracing::warn!("Webhook signature mismatch");
        return Err(AppError::Unauthorized);
    }

    let event: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("Invalid JSON".into()))?;

    let tracker = event["data"]["tracker"]
        .as_str()
        .or_else(|| event["data"]["token"].as_str())
        .unwrap_or("");

    let state_field = event["data"]["state"].as_str().unwrap_or("");
    let event_type = event["type"].as_str().unwrap_or("");

    let succeeded = event_type == "payment.succeeded"
        || matches!(state_field, "TRACKER_ENDED" | "PAID");
    let failed = event_type == "payment.failed"
        || matches!(state_field, "FAILED" | "CANCELLED" | "TRACKER_FAILED");

    match (succeeded, failed) {
        (true, _) => {
            let updated = db::update_payment_status_by_ref(&state.pool, tracker, "succeeded").await?;
            if updated.is_some() {
                confirm_order(&state, tracker, "paid").await;
            }
        }
        (_, true) => {
            let updated = db::update_payment_status_by_ref(&state.pool, tracker, "failed").await?;
            if updated.is_some() {
                confirm_order(&state, tracker, "cancelled").await;
            }
        }
        _ => {
            tracing::info!(
                "Unhandled Safepay webhook event (type={}, state={})",
                event_type,
                state_field
            );
        }
    }

    Ok(StatusCode::OK)
}

async fn confirm_order(state: &AppState, tracker: &str, status: &str) {
    let order_id = match db::get_payment_by_ref(&state.pool, tracker).await {
        Ok(Some(p)) => p.order_id,
        _ => {
            tracing::warn!("Webhook token not found: {}", tracker);
            return;
        }
    };

    let order_service_url =
        std::env::var("ORDER_SERVICE_URL").expect("ORDER_SERVICE_URL not set");

    match state
        .http
        .post(format!("{}/orders/{}/payment-confirm", order_service_url, order_id))
        .json(&serde_json::json!({ "status": status }))
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
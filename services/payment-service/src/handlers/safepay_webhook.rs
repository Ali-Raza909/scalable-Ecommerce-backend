use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use common::RequestId;
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
    let timestamp = headers
        .get("X-SFPY-TIMESTAMP")
        .and_then(|v| v.to_str().ok());
    verify_hmac(signature, body, secret)
        || verify_timestamp_scheme(signature, timestamp, body, secret)
}

/// Plain HMAC scheme: signature is HMAC-SHA512 (or SHA-256) of the raw body,
/// hex-encoded, keyed with the raw webhook secret.
fn verify_hmac(signature: &str, body: &[u8], secret: &[u8]) -> bool {
    let computed = [hex_hmac_sha512(secret, body), hex_hmac_sha256(secret, body)];
    computed.iter().any(|c| c == signature)
}

/// Timestamp scheme: signature is HMAC-SHA256 over `"<timestamp>.<body>"`,
/// keyed with the base64-decoded secret.
fn verify_timestamp_scheme(
    signature: &str,
    timestamp: Option<&str>,
    body: &[u8],
    secret: &[u8],
) -> bool {
    let Some(ts) = timestamp else { return false };
    let key = STANDARD.decode(secret).unwrap_or_default();
    if key.is_empty() {
        return false;
    }
    let mut signed = Vec::with_capacity(ts.len() + 1 + body.len());
    signed.extend_from_slice(ts.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(body);
    hex_hmac_sha256(&key, &signed) == signature
}

#[derive(Debug, PartialEq, Eq)]
enum WebhookKind {
    Succeeded,
    Failed,
}

/// Classifies a webhook by its event type (preferred) then by state field.
/// Type wins, matching how the handler dispatches: a `payment.failed` event
/// can carry `state: TRACKER_ENROLLED` (observed in production), so the event
/// type must be authoritative.
fn classify_event(event_type: &str, state: &str) -> Option<WebhookKind> {
    if event_type == "payment.succeeded" || matches!(state, "TRACKER_ENDED" | "PAID") {
        Some(WebhookKind::Succeeded)
    } else if event_type == "payment.failed"
        || matches!(state, "FAILED" | "CANCELLED" | "TRACKER_FAILED")
    {
        Some(WebhookKind::Failed)
    } else {
        None
    }
}

pub async fn safepay_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    trace: Option<Extension<RequestId>>,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let trace_id = trace.map(|Extension(RequestId(id))| id.to_string());
    tracing::info!(?trace_id, "Safepay webhook received");

    // TEMPORARY defensive log: confirm real payload shape before trusting
    // the event extraction below (Safepay docs are thin here).
    tracing::info!(
        "Raw Safepay webhook payload: {}",
        String::from_utf8_lossy(&body)
    );

    let signature = headers
        .get("X-SFPY-SIGNATURE")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing signature".into()))?;

    let secret = std::env::var("SAFEPAY_WEBHOOK_SECRET").expect("SAFEPAY_WEBHOOK_SECRET not set");

    if !verify_signature(signature, &headers, &body, secret.as_bytes()) {
        tracing::warn!("Webhook signature mismatch");
        return Err(AppError::Unauthorized);
    }

    let event: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("Invalid JSON".into()))?;

    let tracker = event["data"]["tracker"]
        .as_str()
        .or_else(|| event["data"]["token"].as_str())
        .unwrap_or("");

    let state_field = event["data"]["state"].as_str().unwrap_or("");
    let event_type = event["type"].as_str().unwrap_or("");

    match classify_event(event_type, state_field) {
        Some(WebhookKind::Succeeded) => {
            let updated =
                db::update_payment_status_by_ref(&state.pool, tracker, "succeeded").await?;
            if updated.is_some() {
                confirm_order(&state, tracker, "paid").await;
            }
        }
        Some(WebhookKind::Failed) => {
            let updated = db::update_payment_status_by_ref(&state.pool, tracker, "failed").await?;
            if updated.is_some() {
                confirm_order(&state, tracker, "cancelled").await;
            }
        }
        None => {
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

    let order_service_url = std::env::var("ORDER_SERVICE_URL").expect("ORDER_SERVICE_URL not set");

    match state
        .http
        .post(format!(
            "{}/orders/{}/payment-confirm",
            order_service_url, order_id
        ))
        .json(&serde_json::json!({ "status": status }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if status == "paid" {
                // Distinguish a genuine pending -> paid flip from a late payment
                // that arrived after the order was already cancelled (e.g. by the
                // timeout job) or was already paid. Money has moved for an order
                // we will not fulfil: surface it loudly and flag the payment.
                match resp.json::<serde_json::Value>().await {
                    Ok(body) => {
                        let order_status = body["order_status"].as_str().unwrap_or("unknown");
                        if order_status != "paid" {
                            tracing::error!(
                                "LATE PAYMENT: order {} is '{}' but payment {} was confirmed paid; \
                                 marking paid_after_cancel -- manual refund required",
                                order_id, order_status, tracker
                            );
                            if let Err(e) = db::update_payment_status_by_ref(
                                &state.pool,
                                tracker,
                                "paid_after_cancel",
                            )
                            .await
                            {
                                tracing::error!(
                                    "Failed to mark payment {} paid_after_cancel: {:?}",
                                    tracker,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => tracing::warn!(
                        "Failed to parse order-confirm response for order {}: {}",
                        order_id,
                        e
                    ),
                }
            }
        }
        Ok(resp) => tracing::warn!(
            "Order confirm failed for order {}: status {}",
            order_id,
            resp.status()
        ),
        Err(e) => tracing::warn!("Order confirm failed for order {}: {}", order_id, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;

    const SECRET: &[u8] = b"test-webhook-secret";

    fn sha512(body: impl AsRef<[u8]>) -> String {
        hex_hmac_sha512(SECRET, body.as_ref())
    }

    fn sha256(body: impl AsRef<[u8]>) -> String {
        hex_hmac_sha256(SECRET, body.as_ref())
    }

    #[test]
    fn accepts_sha512_signature() {
        let body = br#"{"type":"payment.succeeded"}"#;
        assert!(verify_hmac(&sha512(body), body, SECRET));
    }

    #[test]
    fn accepts_sha256_signature() {
        let body = br#"{"type":"payment.succeeded"}"#;
        assert!(verify_hmac(&sha256(body), body, SECRET));
    }

    #[test]
    fn rejects_tampered_body() {
        let body = br#"{"type":"payment.succeeded"}"#;
        let sig = sha512(body);
        assert!(!verify_hmac(&sig, br#"{"type":"payment.failed"}"#, SECRET));
    }

    #[test]
    fn rejects_signature_from_wrong_secret() {
        let body = br#"{"type":"payment.succeeded"}"#;
        let sig = hex_hmac_sha512(b"other-secret", body);
        assert!(!verify_hmac(&sig, body, SECRET));
    }

    #[test]
    fn rejects_garbage_and_empty_signatures() {
        let body = br#"{"type":"payment.succeeded"}"#;
        assert!(!verify_hmac("not-a-signature", body, SECRET));
        assert!(!verify_hmac("", body, SECRET));
    }

    #[test]
    fn verify_signature_short_circuits_to_timestamp_scheme() {
        // No timestamp header, plain HMAC must still verify through the wrapper.
        let body = br#"{"type":"payment.failed"}"#;
        let headers = HeaderMap::new();
        assert!(verify_signature(&sha512(body), &headers, body, SECRET));
    }

    #[test]
    fn timestamp_scheme_accepts_valid_and_rejects_tampered() {
        let body = br#"{"type":"payment.succeeded"}"#;
        let base64_secret = STANDARD.encode(SECRET);
        let decoded_key = STANDARD.decode(&base64_secret).unwrap();

        let mut signed = Vec::from("1700000000".as_bytes());
        signed.push(b'.');
        signed.extend_from_slice(body);
        let good = hex_hmac_sha256(&decoded_key, &signed);

        assert!(verify_timestamp_scheme(
            &good,
            Some("1700000000"),
            body,
            base64_secret.as_bytes()
        ));
        assert!(!verify_timestamp_scheme(
            &good,
            Some("1699999999"),
            body,
            base64_secret.as_bytes()
        ));
        assert!(!verify_timestamp_scheme(
            &good,
            Some("1700000000"),
            br#"{"type":"payment.failed"}"#,
            base64_secret.as_bytes()
        ));
    }

    #[test]
    fn timestamp_scheme_requires_present_timestamp() {
        let body = br#"{}"#;
        assert!(!verify_timestamp_scheme(&sha256(body), None, body, SECRET));
        // Non-base64 secret cannot derive the timestamp-scheme key.
        assert!(!verify_timestamp_scheme(
            &sha256(body),
            Some("0"),
            body,
            b"not base64!!"
        ));
    }

    #[test]
    fn classify_event_type_precedes_state() {
        // A real observed case: payment.failed with state TRACKER_ENROLLED (403).
        assert_eq!(
            classify_event("payment.failed", "TRACKER_ENROLLED"),
            Some(WebhookKind::Failed)
        );
        assert_eq!(
            classify_event("payment.succeeded", "anything"),
            Some(WebhookKind::Succeeded)
        );
    }

    #[test]
    fn classify_event_by_state_when_type_unknown() {
        assert_eq!(
            classify_event("tracker.updated", "TRACKER_ENDED"),
            Some(WebhookKind::Succeeded)
        );
        assert_eq!(
            classify_event("tracker.updated", "TRACKER_FAILED"),
            Some(WebhookKind::Failed)
        );
        assert_eq!(
            classify_event("tracker.updated", "FAILED"),
            Some(WebhookKind::Failed)
        );
        assert_eq!(
            classify_event("tracker.updated", "PAID"),
            Some(WebhookKind::Succeeded)
        );
    }

    #[test]
    fn classify_event_unknown_state_and_type() {
        assert_eq!(classify_event("tracker.updated", "TRACKER_ENROLLED"), None);
        assert_eq!(classify_event("", ""), None);
    }
}

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use common::RequestId;
use serde::Deserialize;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::{CreatePaymentRequest, CreatePaymentResponse};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct SafepaySessionResponse {
    data: SafepaySessionData,
}

#[derive(Debug, Deserialize)]
struct SafepaySessionData {
    tracker: SafepayTracker,
}

#[derive(Debug, Deserialize)]
struct SafepayTracker {
    token: String,
}

#[derive(Debug, Deserialize)]
struct SafepayPassportResponse {
    data: String,
}

pub fn build_checkout_url(tracker: &str, tbt: &str, order_id: Uuid) -> String {
    let public_base =
        std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:80".to_string());
    format!(
        "https://sandbox.api.getsafepay.com/embedded/?environment=sandbox&tbt={}&tracker={}&source=hosted&order_id={}&redirect_url={}/api/orders/payment-result/{}/success&cancel_url={}/api/orders/payment-result/{}/cancelled",
        tbt, tracker, order_id, public_base, order_id, public_base, order_id
    )
}

fn mock_mode() -> bool {
    std::env::var("SAFEPAY_MOCK")
        .map(|v| v == "true")
        .unwrap_or(false)
}

async fn create_payment_session(
    state: &AppState,
    input: &CreatePaymentRequest,
) -> Result<String, AppError> {
    if mock_mode() {
        // Hermetic mode for CI/demo: mint a local tracker instead of calling
        // Safepay. The E2E regression later drives the real webhook path with a
        // signed payload for this tracker, so nothing downstream changes.
        let tracker = format!("track_mock_{}", Uuid::new_v4());
        tracing::info!("SAFEPAY_MOCK: minting local tracker {}", tracker);
        return Ok(tracker);
    }

    let api_key = std::env::var("SAFEPAY_API_KEY").expect("SAFEPAY_API_KEY not set");
    let merchant_secret =
        std::env::var("SAFEPAY_MERCHANT_SECRET").expect("SAFEPAY_MERCHANT_SECRET not set");

    let resp = state
        .http
        .post("https://sandbox.api.getsafepay.com/order/payments/v3/")
        .header("X-SFPY-MERCHANT-SECRET", merchant_secret)
        .json(&serde_json::json!({
            "merchant_api_key": api_key,
            "intent": "CYBERSOURCE",
            "mode": "payment",
            "entry_mode": "raw",
            "currency": "PKR",
            "amount": input.amount_cents,
            "metadata": { "order_id": input.order_id.to_string() }
        }))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Safepay create session failed: {}", e);
            AppError::PaymentFailed
        })?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!("Safepay create session returned error: {}", body);
        return Err(AppError::PaymentFailed);
    }

    let parsed: SafepaySessionResponse = resp.json().await.map_err(|e| {
        tracing::error!("Failed to parse Safepay session response: {}", e);
        AppError::PaymentFailed
    })?;

    Ok(parsed.data.tracker.token)
}

async fn create_passport_token(state: &AppState) -> Result<String, AppError> {
    if mock_mode() {
        tracing::info!("SAFEPAY_MOCK: minting local passport token");
        return Ok("tbt_mock".into());
    }

    let merchant_secret =
        std::env::var("SAFEPAY_MERCHANT_SECRET").expect("SAFEPAY_MERCHANT_SECRET not set");

    let resp = state
        .http
        .post("https://sandbox.api.getsafepay.com/client/passport/v1/token")
        .header("X-SFPY-MERCHANT-SECRET", merchant_secret)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Safepay passport request failed: {}", e);
            AppError::PaymentFailed
        })?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!("Safepay passport returned error: {}", body);
        return Err(AppError::PaymentFailed);
    }

    let parsed: SafepayPassportResponse = resp.json().await.map_err(|e| {
        tracing::error!("Failed to parse Safepay passport response: {}", e);
        AppError::PaymentFailed
    })?;

    Ok(parsed.data)
}

pub async fn create_payment(
    State(state): State<AppState>,
    trace: Option<Extension<RequestId>>,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<(StatusCode, Json<CreatePaymentResponse>), AppError> {
    let trace_id = trace.map(|Extension(RequestId(id))| id.to_string());
    tracing::info!(?trace_id, order_id = %input.order_id, "create_payment started");

    if input.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".to_string(),
        ));
    }

    let tracker_token = create_payment_session(&state, &input).await?;
    let tbt = create_passport_token(&state).await?;

    let payment = db::create_payment(
        &state.pool,
        input.order_id,
        input.user_id,
        input.amount_cents,
        &tracker_token,
    )
    .await?;

    let checkout_url = build_checkout_url(&tracker_token, &tbt, input.order_id);

    Ok((
        StatusCode::CREATED,
        Json(CreatePaymentResponse {
            payment,
            checkout_url,
        }),
    ))
}

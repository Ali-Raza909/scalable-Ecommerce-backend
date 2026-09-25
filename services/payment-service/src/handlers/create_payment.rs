use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::{CreatePaymentRequest, CreatePaymentResponse};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct SafepayInitResponse {
    data: SafepayInitData,
}

#[derive(Debug, Deserialize)]
struct SafepayInitData {
    token: String,
}

pub fn build_checkout_url(token: &str, order_id: Uuid) -> String {
    format!(
        "https://sandbox.api.getsafepay.com/components?env=sandbox&beacon={}&source=custom&order_id={}&success_url=http://localhost:8085/payments/success&cancel_url=http://localhost:8085/payments/cancel",
        token, order_id
    )
}

pub async fn create_payment(
    State(state): State<AppState>,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<(StatusCode, Json<CreatePaymentResponse>), AppError> {
    if input.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".to_string(),
        ));
    }

    let amount_pkr = input.amount_cents as f64 / 100.0;
    let api_key =
        std::env::var("SAFEPAY_API_KEY").expect("SAFEPAY_API_KEY not set");

    let resp = state
        .http
        .post("https://sandbox.api.getsafepay.com/order/v1/init")
        .json(&serde_json::json!({
            "client": api_key,
            "amount": amount_pkr,
            "currency": "PKR",
            "environment": "sandbox"
        }))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Safepay init request failed: {}", e);
            AppError::PaymentFailed
        })?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!("Safepay init returned error: {}", body);
        return Err(AppError::PaymentFailed);
    }

    let parsed: SafepayInitResponse = resp.json().await.map_err(|e| {
        tracing::error!("Failed to parse Safepay response: {}", e);
        AppError::PaymentFailed
    })?;

    let payment = db::create_payment(
        &state.pool,
        input.order_id,
        input.user_id,
        input.amount_cents,
        &parsed.data.token,
    )
    .await?;

    let checkout_url = build_checkout_url(&parsed.data.token, input.order_id);

    Ok((
        StatusCode::CREATED,
        Json(CreatePaymentResponse {
            payment,
            checkout_url,
        }),
    ))
}
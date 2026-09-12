use axum::http::StatusCode;

pub async fn stripe_webhook() -> StatusCode {
    // TODO: implement once real Stripe integration is added;
    // will need signature verification (Stripe-Signature header)
    // and should call db::update_payment_status(...) based on event.type
    StatusCode::NOT_IMPLEMENTED
}
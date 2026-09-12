use axum::{extract::State, http::StatusCode, Json};
use sqlx::PgPool;

use crate::db;
use crate::error::AppError;
use crate::models::{CreatePaymentRequest, Payment};

pub async fn create_payment(
    State(pool): State<PgPool>,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<(StatusCode, Json<Payment>), AppError> {
    if input.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".to_string(),
        ));
    }

    let payment = db::create_payment(&pool, input.order_id, input.user_id, input.amount_cents)
        .await?;

    Ok((StatusCode::CREATED, Json(payment)))
}
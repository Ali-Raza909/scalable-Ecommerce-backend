use axum::{http::StatusCode, Json};

use crate::email;
use crate::error::AppError;
use crate::models::{CreateNotificationRequest, NotificationResponse};

pub async fn send_email(
    Json(input): Json<CreateNotificationRequest>,
) -> Result<(StatusCode, Json<NotificationResponse>), AppError> {
    if input.email.is_empty() {
        return Err(AppError::BadRequest("Email is required".to_string()));
    }

    email::send_email(&input).await?;

    Ok((
        StatusCode::CREATED,
        Json(NotificationResponse {
            order_id: input.order_id,
            user_id: input.user_id,
            email: input.email,
            subject: format!("Order {} confirmed", input.order_id),
        }),
    ))
}

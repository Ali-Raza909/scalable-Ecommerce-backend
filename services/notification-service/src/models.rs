use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateNotificationRequest {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationResponse {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub subject: String,
}
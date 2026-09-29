use crate::models::CreateNotificationRequest;

pub async fn send_email(input: &CreateNotificationRequest) -> Result<(), anyhow::Error> {
    tracing::info!(
        "Notification stub: would email {} (user {}) about order {}",
        input.email,
        input.user_id,
        input.order_id
    );
    Ok(())
}

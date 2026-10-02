use std::time::Duration;

use crate::models::CreateNotificationRequest;

const RESEND_ENDPOINT: &str = "https://api.resend.com/emails";
const DEFAULT_FROM: &str = "onboarding@resend.dev";

fn should_mock(api_key: &str, email_mock: bool) -> bool {
    email_mock || api_key.is_empty()
}

pub async fn send_email(input: &CreateNotificationRequest) -> Result<(), anyhow::Error> {
    let api_key = std::env::var("RESEND_API_KEY").unwrap_or_default();
    let email_mock = std::env::var("EMAIL_MOCK")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if should_mock(&api_key, email_mock) {
        tracing::info!(
            "Notification stub: would email {} (user {}) about order {}",
            input.email,
            input.user_id,
            input.order_id
        );
        return Ok(());
    }

    let from = std::env::var("RESEND_FROM").unwrap_or_else(|_| DEFAULT_FROM.to_string());
    let subject = format!("Order {} confirmed", input.order_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let resp = client
        .post(RESEND_ENDPOINT)
        .bearer_auth(&api_key)
        .json(&serde_json::json!({
            "from": from,
            "to": [input.email],
            "subject": subject,
            "text": format!(
                "Your order {} has been confirmed. Thank you for your purchase.",
                input.order_id
            ),
        }))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = %body, "Resend send failed");
        anyhow::bail!("Resend API error {status}: {body}");
    }

    tracing::info!(order_id = %input.order_id, "Confirmation email sent via Resend");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_mock;

    #[test]
    fn mocks_when_flag_set_even_with_key() {
        assert!(should_mock("re_abc", true));
    }

    #[test]
    fn mocks_when_key_missing() {
        assert!(should_mock("", false));
    }

    #[test]
    fn sends_when_key_present_and_flag_off() {
        assert!(!should_mock("re_abc", false));
    }
}

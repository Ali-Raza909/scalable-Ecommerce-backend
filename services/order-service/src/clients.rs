use reqwest::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct ServiceClient {
    http: reqwest::Client,
    user_service_url: String,
    product_service_url: String,
    cart_service_url: String,
    payment_service_url: String,
    notification_service_url: String,
}

impl ServiceClient {
    pub fn new(
        user_service_url: String,
        product_service_url: String,
        cart_service_url: String,
        payment_service_url: String,
        notification_service_url: String,
    ) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            user_service_url,
            product_service_url,
            cart_service_url,
            payment_service_url,
            notification_service_url,
        }
    }

    pub async fn validate_user(&self, user_id: Uuid, token: &str) -> Result<(), AppError> {
        let resp = self
            .http
            .get(format!("{}/users/{}", self.user_service_url, user_id))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("User service unreachable: {e}")))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::BadRequest("User not found".to_string()))
        }
    }

    pub async fn get_cart(&self, token: &str) -> Result<CartResponse, AppError> {
        let resp = self
            .http
            .get(format!("{}/cart", self.cart_service_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("Cart service unreachable: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::ServiceUnavailable(format!(
                "Cart service returned status {}",
                resp.status()
            )));
        }

        resp.json::<CartResponse>()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Cart service returned invalid JSON: {e}")))
    }

    pub async fn decrement_stock(&self, product_id: Uuid, quantity: i32) -> Result<Product, AppError> {
        let resp = self
            .http
            .patch(format!("{}/products/{}/stock", self.product_service_url, product_id))
            .json(&serde_json::json!({ "delta": -quantity }))
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("Product service unreachable: {e}")))?;

        match resp.status() {
            StatusCode::CONFLICT => Err(AppError::Conflict(format!(
                "Insufficient stock for product {product_id}"
            ))),
            s if s.is_success() => resp
                .json::<Product>()
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("Product service returned invalid JSON: {e}"))),
            s => Err(AppError::ServiceUnavailable(format!(
                "Product service returned status {s}"
            ))),
        }
    }

    pub async fn clear_cart(&self, token: &str) -> Result<(), AppError> {
        let resp = self
            .http
            .delete(format!("{}/cart", self.cart_service_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("Cart service unreachable: {e}")))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::ServiceUnavailable(format!(
                "Cart service returned status {}",
                resp.status()
            )))
        }
    }

    pub async fn request_payment(&self, order_id: Uuid, user_id: Uuid, amount_cents: i32) -> Result<(), AppError> {
        let resp = self
            .http
            .post(format!("{}/payments", self.payment_service_url))
            .json(&serde_json::json!({
                "order_id": order_id,
                "user_id": user_id,
                "amount_cents": amount_cents,
            }))
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("Payment service unreachable: {e}")))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::ServiceUnavailable(format!(
                "Payment service returned status {}",
                resp.status()
            )))
        }
    }

    pub async fn send_notification(&self, order_id: Uuid, user_id: Uuid, email: &str) -> Result<(), AppError> {
        let resp = self
            .http
            .post(format!("{}/notifications", self.notification_service_url))
            .json(&serde_json::json!({
                "order_id": order_id,
                "user_id": user_id,
                "email": email,
            }))
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("Notification service unreachable: {e}")))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::ServiceUnavailable(format!(
                "Notification service returned status {}",
                resp.status()
            )))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CartResponse {
    pub items: Vec<CartItem>,
}

#[derive(Debug, Deserialize)]
pub struct CartItem {
    pub product_id: String,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub price_cents: i32,
    pub stock_quantity: i32,
}
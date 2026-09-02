use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub jwt_secret: String,
    pub user_service_url: Option<String>,
    pub product_service_url: Option<String>,
    pub cart_service_url: Option<String>,
    pub payment_service_url: Option<String>,
    pub notification_service_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            user_service_url: env::var("USER_SERVICE_URL").ok(),
            product_service_url: env::var("PRODUCT_SERVICE_URL").ok(),
            cart_service_url: env::var("CART_SERVICE_URL").ok(),
            payment_service_url: env::var("PAYMENT_SERVICE_URL").ok(),
            notification_service_url: env::var("NOTIFICATION_SERVICE_URL").ok(),
        }
    }
}
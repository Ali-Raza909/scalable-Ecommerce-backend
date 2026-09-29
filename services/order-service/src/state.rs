use sqlx::PgPool;

use crate::clients::ServiceClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub client: ServiceClient,
    pub jwt_secret: String,
}

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use common::{bearer_token, decode_token, encode_token, Claims};

pub async fn auth_middleware(
    State(jwt_secret): State<String>,
    mut request: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    let token = bearer_token(auth_header).ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    let claims =
        decode_token(token, &jwt_secret).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;

    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

pub fn create_token(
    user_id: &str,
    email: &str,
    role: &str,
    secret: &str,
) -> Result<String, common::AppError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        exp: (now + Duration::hours(24)).timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    encode_token(&claims, secret).map_err(|e| common::AppError::Internal(e.into()))
}

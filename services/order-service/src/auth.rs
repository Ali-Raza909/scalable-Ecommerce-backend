use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use common::decode_token;

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

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    let claims = decode_token(token, &jwt_secret)
        .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;

    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}
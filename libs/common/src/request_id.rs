use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// A single trace id shared by every service hop of one user action
/// (reserve -> pay -> confirm -> notify), so logs from different services can
/// be stitched together by grepping one id.
#[derive(Debug, Clone, Copy)]
pub struct RequestId(pub Uuid);

/// Reuses a valid inbound `x-request-id` so the trace survives service hops.
/// Anything unparsable is replaced with a fresh id rather than trusted, so a
/// caller cannot smuggle arbitrary text into our logs.
pub fn resolve_request_id(inbound: Option<&str>) -> Uuid {
    inbound
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .unwrap_or_else(Uuid::new_v4)
}

/// Stashes the trace id in request extensions (for handlers to log) and echoes
/// it back on the response so a client can quote it in a bug report.
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let inbound = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok());

    let id = resolve_request_id(inbound);
    request.extensions_mut().insert(RequestId(id));

    let mut response = next.run(request).await;

    if let Ok(value) = HeaderValue::from_str(&id.to_string()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXED: &str = "3f2504e0-4f89-11d3-9a0c-0305e82c3301";

    #[test]
    fn reuses_a_valid_inbound_id() {
        assert_eq!(resolve_request_id(Some(FIXED)).to_string(), FIXED);
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        assert_eq!(
            resolve_request_id(Some(&format!("  {FIXED}  "))).to_string(),
            FIXED
        );
    }

    #[test]
    fn generates_when_absent() {
        assert_eq!(resolve_request_id(None).get_version_num(), 4);
    }

    #[test]
    fn replaces_unparsable_inbound_values() {
        // Never trust caller-supplied text: garbage in, fresh id out.
        assert_eq!(resolve_request_id(Some("not-a-uuid")).get_version_num(), 4);
        assert_eq!(resolve_request_id(Some("")).get_version_num(), 4);
        assert_eq!(
            resolve_request_id(Some("../../etc/passwd")).get_version_num(),
            4
        );
    }

    #[test]
    fn generated_ids_are_unique() {
        assert_ne!(resolve_request_id(None), resolve_request_id(None));
    }
}

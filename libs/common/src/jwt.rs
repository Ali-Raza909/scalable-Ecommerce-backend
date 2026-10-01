use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn encode_token(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/// Extracts the bare token from an `Authorization: Bearer <token>` header value.
/// Shared by every service's auth middleware so parsing is defined once.
pub fn bearer_token(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(role: &str, exp: usize) -> Claims {
        Claims {
            sub: "00000000-0000-4000-8000-000000000001".into(),
            email: "user@example.com".into(),
            role: role.into(),
            exp,
            iat: 1_700_000_000,
        }
    }

    #[test]
    fn round_trip_preserves_claims() {
        let token = encode_token(&claims("admin", 9_999_999_999), "secret").unwrap();
        let decoded = decode_token(&token, "secret").unwrap();
        assert_eq!(decoded.sub, claims("admin", 9_999_999_999).sub);
        assert_eq!(decoded.email, "user@example.com");
        assert_eq!(decoded.role, "admin");
    }

    #[test]
    fn rejects_token_signed_with_a_different_secret() {
        let token = encode_token(&claims("user", 9_999_999_999), "right-secret").unwrap();
        assert!(decode_token(&token, "wrong-secret").is_err());
    }

    #[test]
    fn rejects_tampered_token() {
        let mut token = encode_token(&claims("user", 9_999_999_999), "secret").unwrap();
        let last = token.len() - 1;
        let flip = if token.as_bytes()[last] == b'a' {
            'b'
        } else {
            'a'
        };
        token.replace_range(last..last + 1, &flip.to_string());
        assert!(decode_token(&token, "secret").is_err());
    }

    #[test]
    fn rejects_expired_token() {
        // exp is in the past -> Validation::default() rejects it.
        let token = encode_token(&claims("admin", 1_600_000_000), "secret").unwrap();
        assert!(decode_token(&token, "secret").is_err());
    }

    #[test]
    fn rejects_garbage_token() {
        assert!(decode_token("not.a.valid.token", "secret").is_err());
        assert!(decode_token("", "secret").is_err());
    }

    #[test]
    fn bearer_token_parses_only_bearer_scheme() {
        assert_eq!(bearer_token("Bearer abc.def.ghi"), Some("abc.def.ghi"));
        assert_eq!(bearer_token("bearer abc"), None);
        assert_eq!(bearer_token("Token abc"), None);
        assert_eq!(bearer_token(""), None);
        assert_eq!(bearer_token("Bearer"), None);
    }
}

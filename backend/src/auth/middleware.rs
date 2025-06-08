use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode, Request},
    middleware::Next,
    response::Response,
    body::Body,
};
use uuid::Uuid;
use super::validate_jwt;

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct UserId(pub Uuid);

#[async_trait]
impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));

        let token = match auth_header {
            Some(token) => token.trim(),
            None => return Err(StatusCode::UNAUTHORIZED),
        };

        let user_id = match validate_jwt(token) {
            Ok(id) => id,
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        };

        Ok(UserId(user_id))
    }
}

pub async fn require_auth(
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token.trim(),
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    match validate_jwt(token) {
        Ok(user_id) => {
            request.extensions_mut().insert(UserId(user_id));
            Ok(next.run(request).await)
        },
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
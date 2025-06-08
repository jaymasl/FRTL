use axum::http::StatusCode;

#[derive(Debug)]
pub enum Error {
    Database,
    Auth(crate::auth::AuthError),
}

impl From<sqlx::Error> for Error {
    fn from(_: sqlx::Error) -> Self {
        Error::Database
    }
}

impl From<crate::auth::AuthError> for Error {
    fn from(err: crate::auth::AuthError) -> Self {
        Error::Auth(err)
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Error::Database => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
            Error::Auth(e) => match e {
                crate::auth::AuthError::InvalidCredentials => 
                    (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()),
                crate::auth::AuthError::UserExists =>
                    (StatusCode::CONFLICT, "Account already exists".to_string()),
                crate::auth::AuthError::WeakPassword =>
                    (StatusCode::BAD_REQUEST, "Password requirements not met".to_string()),
                crate::auth::AuthError::RateLimitExceeded =>
                    (StatusCode::TOO_MANY_REQUESTS, "Too many attempts".to_string()),
                crate::auth::AuthError::TokenExpired =>
                    (StatusCode::UNAUTHORIZED, "Token has expired".to_string()),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Authentication error".to_string()),
            },
        };
 
        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({
                "error": message
            })).unwrap()))
            .unwrap()
    }
 }
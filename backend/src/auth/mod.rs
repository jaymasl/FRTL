use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use cookie::{Cookie, SameSite};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::{env, time::{SystemTime, UNIX_EPOCH}, fmt};

pub mod services;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod utils;

pub use services::*;

#[derive(Debug)]
pub enum AuthError {
    Database(sqlx::Error),
    JWT(jsonwebtoken::errors::Error),
    InvalidToken,
    TokenExpired,
    InvalidSignature,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "Database error: {}", e),
            Self::JWT(e) => write!(f, "JWT error: {}", e),
            Self::InvalidToken => write!(f, "Invalid token"),
            Self::TokenExpired => write!(f, "Token expired"),
            Self::InvalidSignature => write!(f, "Invalid signature"),
        }
    }
}

impl std::error::Error for AuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(e) => Some(e),
            Self::JWT(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for AuthError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl From<jsonwebtoken::errors::Error> for AuthError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Self::JWT(err)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Error processing request"),
            Self::JWT(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation failed"),
            Self::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            Self::TokenExpired => (StatusCode::UNAUTHORIZED, "Token has expired"),
            Self::InvalidSignature => (StatusCode::UNAUTHORIZED, "Invalid signature"),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub csrf_token: String,
}

fn get_token_duration(refresh: bool) -> i64 {
    if refresh {
        env::var("REFRESH_TOKEN_DURATION")
            .unwrap_or_else(|_| "604800".to_string())
            .parse()
            .unwrap_or(604800)
    } else {
        env::var("ACCESS_TOKEN_DURATION")
            .unwrap_or_else(|_| "86400".to_string())
            .parse()
            .unwrap_or(86400)
    }
}

pub fn set_auth_cookies(user_id: Uuid, headers: &mut HeaderMap) -> Result<(String, String), AuthError> {
    let (access_token, refresh_token, csrf_token) = create_tokens(user_id)?;
    let secure = env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) == "true";
    
    // Always set persistent cookies with explicit expiration times
    let mut access_cookie = Cookie::new("access_token", access_token.clone());
    access_cookie.set_http_only(true);
    access_cookie.set_secure(secure);
    access_cookie.set_same_site(SameSite::Strict);
    access_cookie.set_path("/");
    access_cookie.set_max_age(time::Duration::seconds(get_token_duration(false)));
    
    let mut refresh_cookie = Cookie::new("refresh_token", refresh_token);
    refresh_cookie.set_http_only(true);
    refresh_cookie.set_secure(secure);
    refresh_cookie.set_same_site(SameSite::Strict);
    refresh_cookie.set_path("/api/refresh");
    refresh_cookie.set_max_age(time::Duration::seconds(get_token_duration(true)));
    
    headers.insert(header::SET_COOKIE, access_cookie.to_string().parse().unwrap());
    headers.append(header::SET_COOKIE, refresh_cookie.to_string().parse().unwrap());
    
    Ok((access_token, csrf_token))
}

pub fn validate_jwt(token: &str) -> Result<Uuid, AuthError> {
    let secret = env::var("JWT_SECRET_KEY").expect("JWT_SECRET_KEY must be set");
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.set_required_spec_claims(&["exp", "sub"]);
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation
    ).map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
        jsonwebtoken::errors::ErrorKind::InvalidSignature => AuthError::InvalidSignature,
        _ => AuthError::JWT(e)
    })?;

    Uuid::parse_str(&token_data.claims.sub).map_err(|_| AuthError::InvalidToken)
}

fn create_tokens(user_id: Uuid) -> Result<(String, String, String), AuthError> {
    let csrf_token = Uuid::new_v4().to_string();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

    let access_token = encode(
        &Header::default(),
        &Claims {
            sub: user_id.to_string(),
            exp: now as usize + get_token_duration(false) as usize,
            csrf_token: csrf_token.clone(),
        },
        &EncodingKey::from_secret(env::var("JWT_SECRET_KEY").expect("JWT_SECRET_KEY must be set").as_bytes())
    )?;

    let refresh_token = encode(
        &Header::default(),
        &Claims {
            sub: user_id.to_string(),
            exp: now as usize + get_token_duration(true) as usize,
            csrf_token: csrf_token.clone(),
        },
        &EncodingKey::from_secret(env::var("JWT_REFRESH_SECRET_KEY").expect("JWT_REFRESH_SECRET_KEY must be set").as_bytes())
    )?;

    Ok((access_token, refresh_token, csrf_token))
}
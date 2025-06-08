use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LoginCredentials {
    pub username: String,
}

#[derive(Deserialize)]
pub struct RegisterCredentials {
    pub username: String,
    pub email: String,
    pub captcha_token: String,
}

#[derive(Deserialize)]
pub struct MagicLinkRequest {
    pub email: String,
    pub captcha_token: String,
}

#[derive(Deserialize)]
pub struct MagicLinkVerification {
    pub token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub csrf_token: String,
    pub token: String,
    pub requires_captcha: bool,
    pub current_attempts: Option<i64>,
    pub currency_balance: i32,
    pub user_id: String,
    pub username: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub id: Uuid,
    pub csrf_token: String,
    pub token: String,
}
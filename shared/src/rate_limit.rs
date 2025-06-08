use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const LOGIN_WINDOW: Duration = Duration::from_secs(15 * 60);
pub const PASSWORD_RESET_WINDOW: Duration = Duration::from_secs(3600);
pub const API_WINDOW: Duration = Duration::from_secs(60);
pub const REFRESH_TOKEN_WINDOW: Duration = Duration::from_secs(3600);
pub const REGISTRATION_WINDOW: Duration = Duration::from_secs(3600 * 24);

pub const LOGIN_MAX_ATTEMPTS: u32 = 5;
pub const LOGIN_CAPTCHA_THRESHOLD: u32 = 3;
pub const PASSWORD_RESET_MAX_ATTEMPTS: u32 = 3;
pub const API_MAX_REQUESTS: u32 = 3000;
pub const REFRESH_TOKEN_MAX_ATTEMPTS: u32 = 30;
pub const REGISTRATION_MAX_ATTEMPTS: u32 = 3;

pub const RATE_LIMIT_ERROR: &str = "Rate limit exceeded. Please try again later.";
pub const LOGIN_RATE_LIMIT_ERROR: &str = "Too many login attempts. Please try again in 15 minutes.";
pub const LOGIN_CAPTCHA_REQUIRED: &str = "Please complete the captcha verification before continuing.";
pub const PASSWORD_RESET_RATE_LIMIT_ERROR: &str = "Too many password reset attempts. Please try again in 1 hour.";
pub const API_RATE_LIMIT_ERROR: &str = "Too Many Requests";
pub const REFRESH_TOKEN_RATE_LIMIT_ERROR: &str = "Too many token refresh attempts. Please try again in 1 hour.";
pub const REGISTRATION_RATE_LIMIT_ERROR: &str = "Too many registration attempts. Please try again in 24 hours.";

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum RateLimitType {
    Login,
    PasswordReset,
    Api,
    RefreshToken,
    Registration,
}

impl RateLimitType {
    pub fn get_window(&self) -> Duration {
        match self {
            Self::Login => LOGIN_WINDOW,
            Self::PasswordReset => PASSWORD_RESET_WINDOW,
            Self::Api => API_WINDOW,
            Self::RefreshToken => REFRESH_TOKEN_WINDOW,
            Self::Registration => REGISTRATION_WINDOW,
        }
    }

    pub fn get_max_attempts(&self) -> u32 {
        match self {
            Self::Login => LOGIN_MAX_ATTEMPTS,
            Self::PasswordReset => PASSWORD_RESET_MAX_ATTEMPTS,
            Self::Api => API_MAX_REQUESTS,
            Self::RefreshToken => REFRESH_TOKEN_MAX_ATTEMPTS,
            Self::Registration => REGISTRATION_MAX_ATTEMPTS,
        }
    }

    pub fn get_error_message(&self) -> &'static str {
        match self {
            Self::Login => LOGIN_RATE_LIMIT_ERROR,
            Self::PasswordReset => PASSWORD_RESET_RATE_LIMIT_ERROR,
            Self::Api => API_RATE_LIMIT_ERROR,
            Self::RefreshToken => REFRESH_TOKEN_RATE_LIMIT_ERROR,
            Self::Registration => REGISTRATION_RATE_LIMIT_ERROR,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub limit_type: RateLimitType,
    pub remaining_attempts: u32,
    pub reset_after: Duration,
    pub requires_captcha: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitCheck {
    pub current_attempts: u32,
    pub requires_captcha: bool,
    pub is_locked: bool,
}

impl RateLimitCheck {
    pub fn new(attempts: u32, limit_type: RateLimitType) -> Self {
        let max_attempts = limit_type.get_max_attempts();
        let requires_captcha = match limit_type {
            RateLimitType::Login => attempts >= LOGIN_CAPTCHA_THRESHOLD,
            _ => false,
        };
        
        Self {
            current_attempts: attempts,
            requires_captcha,
            is_locked: attempts >= max_attempts,
        }
    }
}

pub fn get_rate_limit_key(limit_type: RateLimitType, identifier: &str) -> String {
    format!("rate_limit:{}:{}", 
        match limit_type {
            RateLimitType::Login => "login",
            RateLimitType::PasswordReset => "password_reset",
            RateLimitType::Api => "api",
            RateLimitType::RefreshToken => "refresh_token",
            RateLimitType::Registration => "registration",
        },
        identifier
    )
}

pub fn create_rate_limit_error(limit_type: RateLimitType, remaining: Option<Duration>) -> RateLimitError {
    RateLimitError {
        message: limit_type.get_error_message().to_string(),
        remaining: remaining.unwrap_or(limit_type.get_window()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitError {
    pub message: String,
    pub remaining: Duration,
}
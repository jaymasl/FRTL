use serde::{Deserialize, Serialize};
use validator::Validate;
use crate::validation::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RequestResetRequest {
    #[validate(custom = "validate_email")]
    pub email: String,
    pub recaptcha_token: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct VerifyCodeRequest {
    #[validate(custom = "validate_email")]
    pub email: String,
    #[validate(custom = "validate_reset_code")]
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(custom = "validate_email")]
    pub email: String,
    #[validate(custom = "validate_reset_code")]
    pub code: String,
    #[validate(custom = "validate_password")]
    pub new_password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordResetResponse {
    pub message: String,
    pub success: bool,
}
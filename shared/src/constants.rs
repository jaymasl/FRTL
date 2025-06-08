pub const API_BASE_URL: &str = "http://localhost:3000/api";
pub const PASSWORD_RESET_REQUEST_ENDPOINT: &str = "/forgot-password/request";
pub const PASSWORD_RESET_VERIFY_ENDPOINT: &str = "/forgot-password/verify";
pub const PASSWORD_RESET_ENDPOINT: &str = "/forgot-password/reset";

pub const INVALID_EMAIL_ERROR: &str = "Please enter a valid email address";
pub const INVALID_CODE_ERROR: &str = "Please enter a valid 6-digit code";
pub const INVALID_PASSWORD_ERROR: &str = "Password must be at least 8 characters long and contain uppercase, lowercase, number, and special character";
pub const CAPTCHA_REQUIRED_ERROR: &str = "Please complete the hCaptcha";
pub const NETWORK_ERROR: &str = "Network error. Please try again";
pub const SAME_PASSWORD_ERROR: &str = "New password must be different from the current password";

pub const RESET_CODE_LENGTH: usize = 6;
pub const MIN_PASSWORD_LENGTH: usize = 8;
use regex::Regex;
use validator::ValidationError;

pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if email.is_empty() || !email.contains('@') {
        return Err(ValidationError::new("invalid_email_format"));
    }
    Ok(())
}

pub fn validate_reset_code(code: &str) -> Result<(), ValidationError> {
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::new("invalid_reset_code"));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    let has_minimum_length = password.len() >= 8;
    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = Regex::new(r#"[!@#$%^&*(),.?":{}|<>]"#)
        .unwrap()
        .is_match(password);

    if !has_minimum_length 
        || !has_uppercase 
        || !has_lowercase 
        || !has_digit 
        || !has_special {
        return Err(ValidationError::new("invalid_password"));
    }
    Ok(())
}
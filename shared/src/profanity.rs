use rustrict::{CensorStr, Type};

#[derive(Debug)]
pub struct ProfanityFilter;

impl ProfanityFilter {
    pub fn contains_profanity(text: &str) -> bool {
        text.is_inappropriate()
    }

    pub fn validate_username(username: &str) -> Result<(), String> {
        if username.is_inappropriate() {
            return Err(format!("Inappropriate language detected: {}", username));
        }
        Ok(())
    }

    pub fn validate_email_local_part(email: &str) -> Result<(), String> {
        if let Some(local_part) = email.split('@').next() {
            if local_part.is_inappropriate() {
                return Err(format!("Inappropriate language detected: {}", local_part));
            }
        }
        Ok(())
    }

    pub fn get_censored_text(text: &str) -> String {
        text.censor()
    }

    pub fn get_content_details(text: &str) -> ContentDetails {
        let censored = text.censor();
        ContentDetails {
            has_profanity: text.is_inappropriate(),
            is_safe: text.is(Type::SAFE),
            is_evasive: text.is(Type::EVASIVE),
            censored,
        }
    }
}

#[derive(Debug)]
pub struct ContentDetails {
    pub has_profanity: bool,
    pub is_safe: bool,
    pub is_evasive: bool,
    pub censored: String,
}

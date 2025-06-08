use yew::prelude::*;
use regex::Regex;

#[derive(Clone, Debug, PartialEq)]
pub struct PasswordValidationState {
    pub has_min_length: bool,
    pub has_uppercase: bool,
    pub has_lowercase: bool,
    pub has_number: bool,
    pub has_special: bool,
    pub strength: PasswordStrength,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PasswordStrength {
    None,
    Weak,
    Medium,
    Strong,
}

impl Default for PasswordValidationState {
    fn default() -> Self {
        Self {
            has_min_length: false,
            has_uppercase: false,
            has_lowercase: false,
            has_number: false,
            has_special: false,
            strength: PasswordStrength::None,
        }
    }
}

impl PasswordValidationState {
    pub fn is_valid(&self) -> bool {
        self.has_min_length && self.has_uppercase && self.has_lowercase && self.has_number && self.has_special
    }

    fn calculate_strength(&self) -> PasswordStrength {
        let requirements_met = [
            self.has_min_length,
            self.has_uppercase,
            self.has_lowercase,
            self.has_number,
            self.has_special,
        ].iter().filter(|&&x| x).count();

        match requirements_met {
            0..=1 => PasswordStrength::None,
            2..=3 => PasswordStrength::Weak,
            4 => PasswordStrength::Medium,
            5 => PasswordStrength::Strong,
            _ => PasswordStrength::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsernameValidationState {
    pub has_min_length: bool,
    pub has_max_length: bool,
    pub valid_characters: bool,
}

impl Default for UsernameValidationState {
    fn default() -> Self {
        Self {
            has_min_length: false,
            has_max_length: false,
            valid_characters: false,
        }
    }
}

impl UsernameValidationState {
    pub fn is_valid(&self) -> bool {
        self.has_min_length && self.has_max_length && self.valid_characters
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmailValidationState {
    pub has_at_symbol: bool,
    pub has_domain: bool,
    pub valid_format: bool,
}

impl Default for EmailValidationState {
    fn default() -> Self {
        Self {
            has_at_symbol: false,
            has_domain: false,
            valid_format: false,
        }
    }
}

impl EmailValidationState {
    pub fn is_valid(&self) -> bool {
        self.has_at_symbol && self.has_domain && self.valid_format
    }
}

#[hook]
pub fn use_password_validation() -> (UseStateHandle<PasswordValidationState>, Callback<String>) {
    let validation = use_state_eq(PasswordValidationState::default);
    
    let validate = {
        let validation = validation.clone();
        Callback::from(move |current_password: String| {
            let mut new_state = (*validation).clone();
            
            if current_password.is_empty() {
                validation.set(PasswordValidationState::default());
                return;
            }

            new_state.has_min_length = current_password.len() >= 8;
            new_state.has_uppercase = current_password.chars().any(|c| c.is_uppercase());
            new_state.has_lowercase = current_password.chars().any(|c| c.is_lowercase());
            new_state.has_number = current_password.chars().any(|c| c.is_digit(10));
            new_state.has_special = current_password.chars().any(|c| !c.is_alphanumeric());
            new_state.strength = new_state.calculate_strength();
            validation.set(new_state);
        })
    };

    (validation, validate)
}

#[hook]
pub fn use_username_validation() -> (UseStateHandle<UsernameValidationState>, Callback<String>) {
    let validation = use_state_eq(UsernameValidationState::default);
    
    let validate = {
        let validation = validation.clone();
        Callback::from(move |username: String| {
            let mut new_state = (*validation).clone();
            
            if username.is_empty() {
                validation.set(UsernameValidationState::default());
                return;
            }

            new_state.has_min_length = username.len() >= 3;
            new_state.has_max_length = username.len() <= 20;
            new_state.valid_characters = username.chars().all(|c| c.is_alphanumeric() || c == '_');
            validation.set(new_state);
        })
    };

    (validation, validate)
}

#[hook]
pub fn use_email_validation() -> (UseStateHandle<EmailValidationState>, Callback<String>) {
    let validation = use_state_eq(EmailValidationState::default);
    
    let validate = {
        let validation = validation.clone();
        Callback::from(move |email: String| {
            let mut new_state = (*validation).clone();
            
            if email.is_empty() {
                validation.set(EmailValidationState::default());
                return;
            }

            let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
            new_state.has_at_symbol = email.contains('@');
            new_state.has_domain = email.split('@').nth(1).is_some();
            new_state.valid_format = email_regex.is_match(&email);
            validation.set(new_state);
        })
    };

    (validation, validate)
}

pub fn validation_style(valid: bool) -> &'static str {
    if valid {
        "flex items-center text-xs text-green-600 dark:text-green-400"
    } else {
        "flex items-center text-xs text-gray-500 dark:text-gray-400"
    }
}

pub fn validation_icon(valid: bool) -> &'static str {
    if valid {
        "✓"
    } else {
        "•"
    }
}

#[derive(Properties, PartialEq)]
pub struct ValidationProps<T: PartialEq> {
    pub validation: T,
}

#[function_component(PasswordRequirements)]
pub fn password_requirements(props: &ValidationProps<PasswordValidationState>) -> Html {
    let validation = &props.validation;
    let show_requirements = use_state(|| false);
    
    let strength_color = match validation.strength {
        PasswordStrength::None => "bg-gray-200 dark:bg-gray-700",
        PasswordStrength::Weak => "bg-red-500",
        PasswordStrength::Medium => "bg-yellow-500",
        PasswordStrength::Strong => "bg-green-500",
    };
    
    let strength_width = match validation.strength {
        PasswordStrength::None => "w-0",
        PasswordStrength::Weak => "w-1/3",
        PasswordStrength::Medium => "w-2/3",
        PasswordStrength::Strong => "w-full",
    };
    
    let strength_text = match validation.strength {
        PasswordStrength::None => "",
        PasswordStrength::Weak => "Weak",
        PasswordStrength::Medium => "Medium",
        PasswordStrength::Strong => "Strong",
    };

    html! {
        <div class="mt-2 space-y-2">
            <div class="h-1 w-full bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div class={classes!("h-full", "transition-all", "duration-300", strength_width, strength_color)} />
            </div>
            
            <div class="flex justify-between items-center text-sm">
                <span class="text-gray-600 dark:text-gray-400">
                    {strength_text}
                </span>
                <button
                    type="button"
                    class="text-blue-600 dark:text-blue-400 hover:underline focus:outline-none"
                    onclick={let show = show_requirements.clone(); move |_| show.set(!*show)}
                >
                    {"Show Requirements"}
                </button>
            </div>

            if *show_requirements {
                <div class="text-sm space-y-1 text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 p-2 rounded-md">
                    <div class={validation_style(validation.has_min_length)}>
                        <span class="mr-2">{validation_icon(validation.has_min_length)}</span>
                        {"At least 8 characters"}
                    </div>
                    <div class={validation_style(validation.has_uppercase)}>
                        <span class="mr-2">{validation_icon(validation.has_uppercase)}</span>
                        {"Uppercase letter"}
                    </div>
                    <div class={validation_style(validation.has_lowercase)}>
                        <span class="mr-2">{validation_icon(validation.has_lowercase)}</span>
                        {"Lowercase letter"}
                    </div>
                    <div class={validation_style(validation.has_number)}>
                        <span class="mr-2">{validation_icon(validation.has_number)}</span>
                        {"Number"}
                    </div>
                    <div class={validation_style(validation.has_special)}>
                        <span class="mr-2">{validation_icon(validation.has_special)}</span>
                        {"Special character"}
                    </div>
                </div>
            }
        </div>
    }
}

#[function_component(EmailRequirements)]
pub fn email_requirements(props: &ValidationProps<EmailValidationState>) -> Html {
    let validation = &props.validation;
    html! {
        <div class="mt-2 space-y-2">
            <div class={validation_style(validation.has_at_symbol)}>
                <span class="mr-2">{validation_icon(validation.has_at_symbol)}</span>
                {"Contains @ symbol"}
            </div>
            <div class={validation_style(validation.has_domain)}>
                <span class="mr-2">{validation_icon(validation.has_domain)}</span>
                {"Has valid domain"}
            </div>
            <div class={validation_style(validation.valid_format)}>
                <span class="mr-2">{validation_icon(validation.valid_format)}</span>
                {"Valid email format"}
            </div>
        </div>
    }
}

#[function_component(UsernameRequirements)]
pub fn username_requirements(props: &ValidationProps<UsernameValidationState>) -> Html {
    let validation = &props.validation;
    html! {
        <div class="mt-2 space-y-2">
            <div class={validation_style(validation.has_min_length)}>
                <span class="mr-2">{validation_icon(validation.has_min_length)}</span>
                {"At least 3 characters"}
            </div>
            <div class={validation_style(validation.has_max_length)}>
                <span class="mr-2">{validation_icon(validation.has_max_length)}</span>
                {"At most 20 characters"}
            </div>
            <div class={validation_style(validation.valid_characters)}>
                <span class="mr-2">{validation_icon(validation.valid_characters)}</span>
                {"Only letters, numbers, and underscores"}
            </div>
        </div>
    }
}
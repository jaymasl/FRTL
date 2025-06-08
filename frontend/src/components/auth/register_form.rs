use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement, InputEvent, SubmitEvent};
use wasm_bindgen::JsValue;
use js_sys;
use yew::prelude::*;
use gloo_timers::callback::Timeout;
use gloo_timers::future::TimeoutFuture;
use crate::styles;
use crate::hooks::validation::{
    use_email_validation, use_username_validation,
    EmailRequirements, UsernameRequirements
};
use wasm_bindgen::JsCast;
use shared::profanity::ProfanityFilter;
use crate::config::get_api_base_url;

#[derive(Serialize)]
struct RegisterRequest {
    username: String,
    email: String,
    captcha_token: String,
}

#[derive(Properties, PartialEq)]
pub struct RegisterFormProps {
    pub on_success: Callback<()>,
}

fn clear_hcaptcha_token() {
    if let Some(window) = window() {
        let _ = js_sys::Reflect::set(
            &window,
            &JsValue::from_str("registerCaptchaToken"),
            &JsValue::null()
        );
    }
}

#[function_component(RegisterForm)]
pub fn register_form(props: &RegisterFormProps) -> Html {
    let username = use_state(String::new);
    let email = use_state(String::new);
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let success = use_state(|| false);
    let username_ref = use_node_ref();
    let email_ref = use_node_ref();
    
    // Validation states
    let (username_validation, validate_username) = use_username_validation();
    let (email_validation, validate_email) = use_email_validation();

    {
        use_effect_with((), move |_| {
            if let Some(window) = window() {
                let timeout = Timeout::new(100, move || {
                    if let Some(init_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("initHCaptcha"))
                        .ok()
                        .and_then(|v| v.dyn_into::<js_sys::Function>().ok()) {
                        let _ = init_fn.call0(&JsValue::NULL);
                    }
                });
                timeout.forget();
            }
            || ()
        });
    }

    let onsubmit = {
        let username = username.clone();
        let email = email.clone();
        let loading = loading.clone();
        let error = error.clone();
        let success = success.clone();
        let username_validation = username_validation.clone();
        let email_validation = email_validation.clone();
        let on_success = props.on_success.clone();
        let username_ref = username_ref.clone();
        let email_ref = email_ref.clone();
        let validate_username = validate_username.clone();
        let validate_email = validate_email.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            
            if *loading {
                return;
            }
            
            loading.set(true);
            error.set(None);
            
            let username_input = username_ref.cast::<HtmlInputElement>().unwrap();
            let email_input = email_ref.cast::<HtmlInputElement>().unwrap();
            
            let username_value = username_input.value();
            let email_value = email_input.value();
            
            // Validate inputs
            validate_username.emit(username_value.clone());
            validate_email.emit(email_value.clone());
            
            // Check if validation passes
            let username_valid = username_validation.is_valid();
            let email_valid = email_validation.is_valid();
            
            if !username_valid || !email_valid {
                error.set(Some("Please fix validation errors before submitting.".to_string()));
                loading.set(false);
                return;
            }
            
            username.set(username_value.clone());
            email.set(email_value.clone());
            
            let captcha_token = if let Some(window) = window() {
                // Try to get the token from window.registerCaptchaToken first
                let token = js_sys::Reflect::get(&window, &JsValue::from_str("registerCaptchaToken"))
                    .ok()
                    .and_then(|token| token.as_string())
                    .unwrap_or_default();
                
                // If token is empty, try the old method as fallback
                if token.is_empty() {
                    js_sys::Reflect::get(&window, &JsValue::from_str("hcaptcha"))
                        .ok()
                        .and_then(|hcaptcha| {
                            js_sys::Reflect::get(&hcaptcha, &JsValue::from_str("getResponse"))
                                .ok()
                                .and_then(|get_response| get_response.dyn_into::<js_sys::Function>().ok())
                                .and_then(|func| func.call0(&JsValue::NULL).ok())
                                .and_then(|response| response.as_string())
                        })
                        .unwrap_or_default()
                } else {
                    token
                }
            } else {
                String::new()
            };
            
            if captcha_token.is_empty() {
                error.set(Some("Please complete the captcha".to_string()));
                loading.set(false);
                return;
            }

            let request = RegisterRequest {
                username: username_value.to_lowercase(),
                email: email_value.to_lowercase(),
                captcha_token: captcha_token,
            };
            
            let mut validation_errors = Vec::new();
            
            if let Err(msg) = ProfanityFilter::validate_username(&username_value) {
                validation_errors.push(msg);
            }
            if let Err(msg) = ProfanityFilter::validate_email_local_part(&email_value) {
                validation_errors.push(msg);
            }

            if !validation_errors.is_empty() {
                error.set(Some(validation_errors.join("\n")));
                loading.set(false);
                return;
            }

            let loading = loading.clone();
            let error = error.clone();
            let success = success.clone();
            let on_success = on_success.clone();

            spawn_local(async move {
                match Request::post(&format!("{}/api/auth/register", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 201 {
                            success.set(true);
                            
                            // Clear hCaptcha token
                            clear_hcaptcha_token();
                            
                            // Wait a bit before redirecting
                            let on_success = on_success.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(12000).await;
                                on_success.emit(());
                            });
                        } else {
                            let error_text = match response.status() {
                                400 => {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(json) => {
                                            if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                                                error.to_string()
                                            } else {
                                                "Invalid registration data".to_string()
                                            }
                                        },
                                        Err(_) => "Invalid registration data".to_string()
                                    }
                                },
                                _ => "Registration failed. Please try again.".to_string()
                            };
                            error.set(Some(error_text));
                        }
                    },
                    Err(_) => {
                        error.set(Some("Network error. Please try again.".to_string()));
                    }
                }
                loading.set(false);
            });
        })
    };

    html! {
        <div class={styles::AUTH_CARD}>
            <div class={styles::AUTH_HEADER}>
                <h2 class={styles::TEXT_H2}>{"Create an Account"}</h2>
                <p class={styles::TEXT_SMALL}>
                    {"Sign up to get started"}
                </p>
            </div>

            if let Some(err) = &*error {
                <div class={classes!(styles::CARD_ERROR, "error-message")}>{err}</div>
            }

            if *success {
                <div class={styles::CARD_SUCCESS}>
                    <div class="flex items-center">
                        <svg class="w-5 h-5 mr-2 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
                        </svg>
                        <span>{"Registration request received"}</span>
                    </div>
                    <p class="mt-2">{"If your email is available, you will receive a confirmation link shortly."}</p>
                    <p class="mt-2 text-sm">{"Please check your inbox and spam folder. The email will be from frtl@jaykrown.com"}</p>
                    <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">{"FRTL will never request personal information or passwords via email. Always verify links before clicking."}</p>
                </div>
            } else {
                <form onsubmit={onsubmit} class={styles::FORM}>
                    <div>
                        <label for="username" class={styles::TEXT_LABEL}>
                            {"Username"}
                        </label>
                        <input
                            id="username"
                            type="text"
                            ref={username_ref}
                            required=true
                            disabled={*loading}
                            placeholder="Choose a username"
                            class={styles::INPUT}
                            oninput={let validate = validate_username.clone(); move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                validate.emit(input.value());
                            }}
                        />
                        <UsernameRequirements validation={(*username_validation).clone()} />
                    </div>

                    <div>
                        <label for="email" class={styles::TEXT_LABEL}>
                            {"Email"}
                        </label>
                        <input
                            id="email"
                            type="email"
                            ref={email_ref}
                            required=true
                            disabled={*loading}
                            placeholder="Enter your email"
                            class={styles::INPUT}
                            oninput={let validate = validate_email.clone(); move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                validate.emit(input.value());
                            }}
                        />
                        <EmailRequirements validation={(*email_validation).clone()} />
                    </div>

                    <div class="flex justify-center w-full my-6">
                        <div id="h-captcha-container" class="h-captcha"></div>
                    </div>

                    <div class="flex flex-col space-y-2 mt-4">
                        <button
                            type="submit"
                            disabled={*loading}
                            class={styles::AUTH_BUTTON}
                        >
                            {
                                if *loading {
                                    html! {
                                        <span class="flex items-center justify-center">
                                            <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                            </svg>
                                            {"Creating Account..."}
                                        </span>
                                    }
                                } else {
                                    html! { "Create Account" }
                                }
                            }
                        </button>
                        {
                            (*loading).then(|| html! {
                                <p class="text-xs text-center text-gray-500 mt-2">
                                    {"Please be patient. Account creation may take up to 30 seconds to complete."}
                                </p>
                            })
                        }
                    </div>
                </form>
            }
        </div>
    }
}
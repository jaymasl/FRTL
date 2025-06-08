use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement, SubmitEvent};
use wasm_bindgen::{JsValue, JsCast};
use js_sys;
use yew::prelude::*;
use gloo_timers::callback::Timeout;
use crate::styles;
use shared::validation::validate_email;
use crate::config::get_api_base_url;
use serde_json;

#[derive(Serialize)]
struct MagicLinkRequest {
    email: String,
    captcha_token: String,
}

#[derive(Properties, PartialEq)]
pub struct MagicLinkFormProps {
    pub on_success: Callback<()>,
    pub on_cancel: Callback<()>,
}

#[function_component(MagicLinkForm)]
pub fn magic_link_form(_props: &MagicLinkFormProps) -> Html {
    let email = use_state(String::new);
    let error = use_state(String::new);
    let success = use_state(String::new);
    let loading = use_state(|| false);
    let email_ref = use_node_ref();

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

    let handle_submit = {
        let email = email.clone();
        let error = error.clone();
        let success = success.clone();
        let loading = loading.clone();
        let email_ref = email_ref.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            
            if *loading {
                return;
            }
            
            loading.set(true);
            error.set(String::new());
            
            let email_input = email_ref.cast::<HtmlInputElement>().unwrap();
            let email_value = email_input.value();
            
            if let Err(_) = validate_email(&email_value) {
                error.set("Please enter a valid email address".to_string());
                loading.set(false);
                return;
            }
            
            email.set(email_value.clone());
            
            let captcha_token = if let Some(window) = window() {
                // First try to get the token from the hcaptcha.getResponse() method
                let direct_token = js_sys::Reflect::get(&window, &JsValue::from_str("hcaptcha"))
                    .ok()
                    .and_then(|hcaptcha| {
                        js_sys::Reflect::get(&hcaptcha, &JsValue::from_str("getResponse"))
                            .ok()
                            .and_then(|get_response| get_response.dyn_into::<js_sys::Function>().ok())
                            .and_then(|func| func.call0(&JsValue::NULL).ok())
                            .and_then(|response| response.as_string())
                    })
                    .unwrap_or_default();
                
                // If direct method failed, try to get the token from the window.loginCaptchaToken variable
                if direct_token.is_empty() {
                    js_sys::Reflect::get(&window, &JsValue::from_str("loginCaptchaToken"))
                        .ok()
                        .and_then(|token| token.as_string())
                        .unwrap_or_default()
                } else {
                    direct_token
                }
            } else {
                String::new()
            };
            
            if captcha_token.is_empty() {
                error.set("Please complete the captcha".to_string());
                loading.set(false);
                
                // Reset captcha to ensure it's visible
                if let Some(window) = window() {
                    if let Some(reset_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("resetCaptcha"))
                        .ok()
                        .and_then(|v| v.dyn_into::<js_sys::Function>().ok()) {
                        let _ = reset_fn.call0(&JsValue::NULL);
                    }
                }
                
                return;
            }
            
            let request = MagicLinkRequest {
                email: email_value.clone(),
                captcha_token,
            };
            
            let error_clone = error.clone();
            let loading_clone = loading.clone();
            
            // Create a consistent success message
            let success_message = "If an account exists with this email, a link has been sent. Please check your inbox and spam folder. The email will be from frtl@jaykrown.com.".to_string();
            
            // Set a fixed delay for showing the success message (5 seconds)
            let fixed_delay = 5000; // 5 seconds
            
            // Start the API request in the background
            spawn_local(async move {
                let result = Request::post(&format!("{}/api/auth/magic-link/request", get_api_base_url()))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;
                
                // Handle errors if they occur
                if let Err(_) = result {
                    error_clone.set("Failed to send request".to_string());
                    loading_clone.set(false);
                    return;
                }
                
                let response = result.unwrap();
                if response.status() != 200 {
                    let error_text = response.text().await.unwrap_or_else(|_| "Failed to send link".to_string());
                    
                    // Log the raw error text for debugging
                    web_sys::console::log_1(&format!("Raw error response: {}", &error_text).into());
                    
                    // Try to parse as JSON to extract error message
                    match serde_json::from_str::<serde_json::Value>(&error_text) {
                        Ok(error_json) => {
                            if let Some(error_msg) = error_json.get("error").and_then(|v| v.as_str()) {
                                web_sys::console::log_1(&format!("Parsed error message: {}", error_msg).into());
                                error_clone.set(error_msg.to_string());
                            } else {
                                error_clone.set("An error occurred. Please try again.".to_string());
                            }
                        },
                        Err(_) => {
                            // If it's not valid JSON, just display the text
                            error_clone.set(error_text);
                        }
                    }
                    
                    loading_clone.set(false);
                    return;
                }
                
                // If we get here, the request was successful, but we don't need to do anything
                // The success message will be shown by the timer below
            });
            
            // Set up a timer to show the success message after the fixed delay
            // This runs independently of the API request
            let success_clone = success.clone();
            let loading_clone = loading.clone();
            let timeout = gloo_timers::callback::Timeout::new(fixed_delay, move || {
                success_clone.set(success_message);
                loading_clone.set(false);
            });
            timeout.forget();
        })
    };

    html! {
        <div class={styles::AUTH_CARD}>
            <div class={styles::AUTH_HEADER}>
                <h2 class={styles::TEXT_H2}>{"Sign in with Email"}</h2>
                <p class={styles::TEXT_SMALL}>
                    {"We'll send you a link to sign in"}
                </p>
            </div>

            {
                (!(*error).is_empty()).then(|| html! {
                    <div class={classes!(styles::CARD_ERROR, "error-message")}>{&*error}</div>
                })
            }

            {
                (!(*success).is_empty()).then(|| html! {
                    <div class={classes!(styles::CARD_SUCCESS, "success-message")}>
                        <div class="flex items-center">
                            <svg class="w-5 h-5 mr-2 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
                            </svg>
                            <span>{"Request Sent"}</span>
                        </div>
                        <p class="mt-2 text-sm">{&*success}</p>
                        <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">{"FRTL will never request personal information or passwords via email. Always verify links before clicking."}</p>
                    </div>
                })
            }

            <form onsubmit={handle_submit} class={styles::FORM}>
                {
                    if (*success).is_empty() {
                        html! {
                            <>
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
                                    />
                                </div>

                                <div class="flex justify-center w-full my-6">
                                    <div id="h-captcha-container" class="h-captcha"></div>
                                </div>

                                <div class="flex flex-col space-y-2">
                                    <button
                                        type="submit"
                                        disabled={*loading}
                                        class={styles::BUTTON_PRIMARY}
                                    >
                                        {
                                            if *loading {
                                                html! {
                                                    <span class="flex items-center justify-center">
                                                        <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                        </svg>
                                                        {"Sending..."}
                                                    </span>
                                                }
                                            } else {
                                                html! { "Send Link" }
                                            }
                                        }
                                    </button>
                                    {
                                        (*loading).then(|| html! {
                                            <p class="text-xs text-center text-gray-500 mt-2">
                                                {"Please be patient. Email delivery may take up to 30 seconds to complete."}
                                            </p>
                                        })
                                    }
                                </div>
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
            </form>
        </div>
    }
} 
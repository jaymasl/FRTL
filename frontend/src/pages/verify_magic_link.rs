use yew::prelude::*;
use yew_router::prelude::*;
use web_sys::window;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use serde::{Deserialize, Serialize};
use crate::{Route, base::Base, styles};
use crate::components::GradientBackground;
use crate::config::get_api_base_url;

#[derive(Serialize)]
struct MagicLinkVerification {
    token: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    csrf_token: String,
    token: String,
    user_id: String,
    username: String,
    currency_balance: i32,
}

#[derive(Deserialize)]
struct RegistrationNeededResponse {
    email: String,
    needs_registration: bool,
}

#[function_component(VerifyMagicLink)]
pub fn verify_magic_link() -> Html {
    let navigator = use_navigator().unwrap();
    let status = use_state(|| "Verifying your magic link...".to_string());
    let error = use_state(|| String::new());
    let email = use_state(|| String::new());
    let needs_registration = use_state(|| false);
    let username = use_state(String::new);
    let username_ref = use_node_ref();
    let username_error = use_state(String::new);
    let loading = use_state(|| false);
    let success = use_state(|| String::new());

    // Get token from URL
    {
        let status = status.clone();
        let error = error.clone();
        let email = email.clone();
        let needs_registration = needs_registration.clone();
        let navigator = navigator.clone();
        
        use_effect_with((), move |_| {
            let window = window().unwrap();
            let location = window.location();
            let search = location.search().unwrap_or_default();
            let params = web_sys::UrlSearchParams::new_with_str(&search).unwrap();
            
            if let Some(token) = params.get("token") {
                let verification = MagicLinkVerification { token };
                
                spawn_local(async move {
                    let result = Request::post(&format!("{}/api/auth/magic-link/verify", get_api_base_url()))
                        .json(&verification)
                        .unwrap()
                        .send()
                        .await;
                    
                    match result {
                        Ok(response) => {
                            if response.status() == 200 {
                                // User exists, handle successful login
                                match response.json::<AuthResponse>().await {
                                    Ok(auth_response) => {
                                        // Store tokens
                                        let storage = if let Some(storage) = window.local_storage().ok().flatten() {
                                            storage
                                        } else {
                                            error.set("Failed to access local storage".to_string());
                                            return;
                                        };
                                        
                                        let _ = storage.set_item("token", &auth_response.token);
                                        let _ = storage.set_item("csrf_token", &auth_response.csrf_token);
                                        let _ = storage.set_item("user_id", &auth_response.user_id);
                                        let _ = storage.set_item("username", &auth_response.username);
                                        let _ = storage.set_item("currency", &auth_response.currency_balance.to_string());
                                        
                                        // Redirect to home immediately for login (not registration)
                                        navigator.push(&Route::Home);
                                    },
                                    Err(_) => {
                                        error.set("Failed to parse authentication response".to_string());
                                    }
                                }
                            } else if response.status() == 404 {
                                // User doesn't exist, show registration form
                                match response.json::<RegistrationNeededResponse>().await {
                                    Ok(reg_response) => {
                                        if reg_response.needs_registration {
                                            email.set(reg_response.email);
                                            needs_registration.set(true);
                                            status.set("Create your account".to_string());
                                        } else {
                                            error.set("Invalid response from server".to_string());
                                        }
                                    },
                                    Err(_) => {
                                        error.set("Failed to parse registration response".to_string());
                                    }
                                }
                            } else {
                                // Error
                                let error_text = response.text().await.unwrap_or_else(|_| "Failed to verify magic link".to_string());
                                error.set(error_text);
                            }
                        },
                        Err(_) => {
                            error.set("Failed to send verification request".to_string());
                        }
                    }
                });
            } else {
                error.set("No token found in URL".to_string());
            }
            
            || ()
        });
    }

    let handle_register = {
        let username = username.clone();
        let email = email.clone();
        let username_error = username_error.clone();
        let loading = loading.clone();
        let username_ref = username_ref.clone();
        let status = status.clone();
        let success = success.clone();
        let needs_registration = needs_registration.clone();
        
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            
            if *loading {
                return;
            }
            
            loading.set(true);
            username_error.set(String::new());
            
            let username_input = username_ref.cast::<web_sys::HtmlInputElement>().unwrap();
            let username_value = username_input.value();
            
            if username_value.len() < 3 {
                username_error.set("Username must be at least 3 characters".to_string());
                loading.set(false);
                return;
            }
            
            username.set(username_value.clone());
            
            let request = serde_json::json!({
                "username": username_value,
                "email": *email,
                "password": null,
                "captcha_token": "10000000-aaaa-bbbb-cccc-000000000001" // Dummy token for passwordless registration
            });
            
            let loading_clone = loading.clone();
            let username_error_clone = username_error.clone();
            let status_clone = status.clone();
            let success_clone = success.clone();
            let needs_registration_clone = needs_registration.clone();
            
            spawn_local(async move {
                let result = Request::post(&format!("{}/api/auth/register", get_api_base_url()))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;
                
                loading_clone.set(false);
                
                match result {
                    Ok(response) => {
                        if response.status() == 201 {
                            // Registration successful
                            match response.json::<AuthResponse>().await {
                                Ok(auth_response) => {
                                    // Store tokens
                                    let window = window().unwrap();
                                    let storage = if let Some(storage) = window.local_storage().ok().flatten() {
                                        storage
                                    } else {
                                        username_error_clone.set("Failed to access local storage".to_string());
                                        return;
                                    };
                                    
                                    let _ = storage.set_item("token", &auth_response.token);
                                    let _ = storage.set_item("csrf_token", &auth_response.csrf_token);
                                    let _ = storage.set_item("user_id", &auth_response.user_id);
                                    let _ = storage.set_item("username", &auth_response.username);
                                    let _ = storage.set_item("currency", &auth_response.currency_balance.to_string());
                                    
                                    // Show success message instead of redirecting
                                    status_clone.set("Account Created Successfully!".to_string());
                                    success_clone.set(format!("Welcome to FRTL, {}! You can now start exploring.", auth_response.username));
                                    needs_registration_clone.set(false);
                                },
                                Err(_) => {
                                    username_error_clone.set("Failed to parse authentication response".to_string());
                                }
                            }
                        } else {
                            // Error
                            let error_text = response.text().await.unwrap_or_else(|_| "Failed to register".to_string());
                            username_error_clone.set(error_text);
                        }
                    },
                    Err(_) => {
                        username_error_clone.set("Failed to send registration request".to_string());
                    }
                }
            });
        })
    };

    let go_to_home = {
        let navigator = navigator.clone();
        Callback::from(move |_| navigator.push(&Route::Home))
    };

    html! {
        <Base>
            <GradientBackground>
                <div class="min-h-screen w-full px-4 sm:px-6 lg:px-8">
                    <div class="max-w-md mx-auto px-4 sm:px-6 py-4">
                        <div class={styles::CARD}>
                            <div class={styles::AUTH_HEADER}>
                                <h2 class={styles::TEXT_H2}>{&*status}</h2>
                                {
                                    if *needs_registration {
                                        html! {
                                            <p class={styles::TEXT_SMALL}>
                                                {"Create a username to complete your registration"}
                                            </p>
                                        }
                                    } else if !(*success).is_empty() {
                                        html! {
                                            <>
                                                <p class={styles::TEXT_SMALL}>{&*success}</p>
                                                <button 
                                                    onclick={go_to_home}
                                                    class={format!("{} mt-4", styles::BUTTON_PRIMARY)}
                                                >
                                                    {"Go to Home Page"}
                                                </button>
                                            </>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                            
                            {
                                (!(*error).is_empty()).then(|| html! {
                                    <div class={classes!(styles::CARD_ERROR, "error-message")}>{&*error}</div>
                                })
                            }
                            
                            {
                                if *needs_registration {
                                    html! {
                                        <form onsubmit={handle_register} class={styles::FORM}>
                                            {
                                                (!(*username_error).is_empty()).then(|| html! {
                                                    <div class={classes!(styles::CARD_ERROR, "error-message")}>{&*username_error}</div>
                                                })
                                            }
                                            
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
                                                />
                                            </div>
                                            
                                            <div>
                                                <label for="email" class={styles::TEXT_LABEL}>
                                                    {"Email"}
                                                </label>
                                                <input
                                                    id="email"
                                                    type="email"
                                                    value={(*email).clone()}
                                                    disabled=true
                                                    class={styles::INPUT}
                                                />
                                                <p class={styles::TEXT_HINT}>{"Your email address is already verified"}</p>
                                            </div>
                                            
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
                                                                {"Creating account..."}
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
                                        </form>
                                    }
                                } else if (*error).is_empty() && (*success).is_empty() {
                                    html! {
                                        <div class="flex justify-center py-4">
                                            <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500"></div>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                    </div>
                </div>
            </GradientBackground>
        </Base>
    }
} 
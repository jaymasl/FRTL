use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, console};
use yew::prelude::*;
use yew_router::prelude::*;
use gloo_timers::future::TimeoutFuture;
use crate::styles;
use crate::Route;
use crate::base::Base;
use crate::config::get_api_base_url;
use crate::components::GradientBackground;

#[derive(Serialize)]
struct VerifyDeleteAccountRequest {
    token: String,
}

#[function_component(VerifyDeleteAccount)]
pub fn verify_delete_account() -> Html {
    let navigator = use_navigator().unwrap();
    let token = use_state(|| String::new());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let success = use_state(|| false);

    // Extract token from URL
    {
        let token = token.clone();
        use_effect_with((), move |_| {
            console::log_1(&"Extracting token from URL...".into());
            if let Some(window) = window() {
                // Try to get token from search params
                if let Ok(location) = window.location().search() {
                    console::log_1(&format!("URL search: {}", location).into());
                    let params = location.trim_start_matches('?').split('&');
                    for param in params {
                        if let Some((key, value)) = param.split_once('=') {
                            if key == "token" {
                                console::log_1(&format!("Found token in search params: {}", value).into());
                                token.set(value.to_string());
                                break;
                            }
                        }
                    }
                } else {
                    console::error_1(&"Failed to get location search".into());
                }
                
                // If token is still empty, try to get it from pathname
                if token.is_empty() {
                    if let Ok(pathname) = window.location().pathname() {
                        console::log_1(&format!("URL pathname: {}", pathname).into());
                        let parts: Vec<&str> = pathname.split('/').collect();
                        if parts.len() >= 3 && parts[1] == "verify-delete-account" {
                            let possible_token = parts[2];
                            if !possible_token.is_empty() {
                                console::log_1(&format!("Found token in pathname: {}", possible_token).into());
                                token.set(possible_token.to_string());
                            }
                        }
                    } else {
                        console::error_1(&"Failed to get location pathname".into());
                    }
                }
                
                // If token is still empty, try to get it from hash
                if token.is_empty() {
                    if let Ok(hash) = window.location().hash() {
                        console::log_1(&format!("URL hash: {}", hash).into());
                        if hash.starts_with("#token=") {
                            let possible_token = hash.trim_start_matches("#token=");
                            if !possible_token.is_empty() {
                                console::log_1(&format!("Found token in hash: {}", possible_token).into());
                                token.set(possible_token.to_string());
                            }
                        }
                    } else {
                        console::error_1(&"Failed to get location hash".into());
                    }
                }
                
                // If token is still empty, try to get the entire URL for debugging
                if token.is_empty() {
                    if let Ok(href) = window.location().href() {
                        console::log_1(&format!("Full URL: {}", href).into());
                    }
                }
            } else {
                console::error_1(&"Failed to get window".into());
            }
            || ()
        });
    }

    // Process token
    {
        let token = token.clone();
        let loading = loading.clone();
        let error = error.clone();
        let success = success.clone();
        let navigator = navigator.clone();

        use_effect_with(token.clone(), move |token| {
            let token_value = (**token).clone();
            console::log_1(&format!("Token value: '{}'", token_value).into());
            
            // Only process if token is not empty
            if !token_value.is_empty() {
                let loading = loading.clone();
                let error = error.clone();
                let success = success.clone();
                let navigator = navigator.clone();

                spawn_local(async move {
                    let request = VerifyDeleteAccountRequest {
                        token: token_value.clone(),
                    };

                    console::log_1(&format!("Sending verification request with token: {}", token_value).into());
                    let api_url = format!("{}/api/users/me/verify-delete", get_api_base_url());
                    console::log_1(&format!("API URL: {}", api_url).into());

                    match Request::post(&api_url)
                        .json(&request)
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            console::log_1(&format!("Response status: {}", response.status()).into());
                            
                            if response.status() == 200 {
                                // Parse the response to get the username
                                match response.json::<serde_json::Value>().await {
                                    Ok(data) => {
                                        if let Some(username) = data.get("username").and_then(|u| u.as_str()) {
                                            console::log_1(&format!("👤 User '{}' account deleted successfully", username).into());
                                        } else {
                                            console::log_1(&"Account deletion successful".into());
                                        }
                                    },
                                    Err(_) => {
                                        console::log_1(&"Account deletion successful".into());
                                    }
                                }
                                
                                success.set(true);
                                
                                // Clear all storage
                                if let Ok(Some(storage)) = window().unwrap().local_storage() {
                                    let _ = storage.clear();
                                }
                                if let Ok(Some(storage)) = window().unwrap().session_storage() {
                                    let _ = storage.clear();
                                }

                                // Redirect to home after a delay
                                spawn_local(async move {
                                    TimeoutFuture::new(3000).await;
                                    navigator.push(&Route::Home);
                                });
                            } else {
                                let error_text = response.text().await.unwrap_or_else(|_| 
                                    "Failed to verify account deletion".to_string()
                                );
                                console::error_1(&format!("Error response: {}", error_text).into());
                                error.set(Some(error_text));
                            }
                        }
                        Err(err) => {
                            let error_msg = format!("Failed to send verification request: {:?}", err);
                            console::error_1(&format!("{}", error_msg).into());
                            error.set(Some(error_msg));
                        }
                    }
                    loading.set(false);
                });
            } else {
                console::error_1(&"Token is empty, cannot proceed with verification".into());
                error.set(Some("Invalid or missing token. Please check your email link.".to_string()));
                loading.set(false);
            }

            || ()
        });
    }

    html! {
        <Base>
            <GradientBackground>
                <div class="min-h-screen flex items-center justify-center">
                    <div class={styles::AUTH_CARD}>
                        <h2 class={styles::TEXT_H2}>{"Account Deletion"}</h2>
                        
                        if *loading {
                            <div class="mt-8 flex justify-center">
                                <div class={styles::LOADING_SPINNER}></div>
                            </div>
                            <p class="mt-4 text-center text-gray-600 dark:text-gray-400">
                                {"Verifying your request..."}
                            </p>
                            <p class="mt-2 text-center text-xs text-gray-500 dark:text-gray-500">
                                {format!("Token: {}", *token)}
                            </p>
                        } else if *success {
                            <div class={format!("{} mt-4", styles::ALERT_SUCCESS)}>
                                <p>{"Your account has been successfully deleted."}</p>
                                <p class="mt-2">{"You will be redirected to the home page in a few seconds."}</p>
                            </div>
                        } else if let Some(err) = &*error {
                            <div class={format!("{} mt-4", styles::ALERT_ERROR)}>
                                <p>{err}</p>
                            </div>
                            <div class="mt-4">
                                <button 
                                    class={styles::BUTTON_PRIMARY}
                                    onclick={let navigator = navigator.clone(); move |_| navigator.push(&Route::Home)}
                                >
                                    {"Return to Home"}
                                </button>
                            </div>
                        }
                    </div>
                </div>
            </GradientBackground>
        </Base>
    }
} 
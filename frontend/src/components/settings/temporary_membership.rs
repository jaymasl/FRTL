use gloo_net::http::Request;
use serde::{Serialize, Deserialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;
use crate::config::get_api_base_url;
use crate::hooks::use_currency::use_currency;
use crate::base::dispatch_membership_event;
use crate::styles;
use js_sys::Date;

// Default values - matching backend defaults
const DEFAULT_COST: i32 = 1000;
const DEFAULT_DURATION_MINUTES: i32 = 10080; // 7 days

// Function to format ISO date string to a more readable format
fn format_date_time(iso_string: &str) -> String {
    let date = Date::new(&wasm_bindgen::JsValue::from_str(iso_string));
    
    // Create options for formatting
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"dateStyle".into(), &"medium".into());
    let _ = js_sys::Reflect::set(&options, &"timeStyle".into(), &"short".into());
    
    // Format the date using toLocaleString
    date.to_locale_string("en-US", &options)
        .as_string()
        .unwrap_or_else(|| iso_string.to_string())
}

#[derive(Serialize)]
struct PurchaseRequest {
    duration_minutes: Option<i32>,
}

#[derive(Deserialize, Clone, Debug)]
struct PurchaseResponse {
    new_balance: i32,
    duration_minutes: i32,
    expires_at: String,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub on_error: Callback<String>,
    pub on_success: Callback<String>,
}

#[function_component(TemporaryMembership)]
pub fn temporary_membership(props: &Props) -> Html {
    let error = use_state(String::new);
    let success = use_state(String::new);
    let loading = use_state(|| false);
    let currency = use_currency();
    
    let purchase_membership = {
        let error_state = error.clone();
        let success_state = success.clone();
        let loading_state = loading.clone();
        let on_error = props.on_error.clone();
        let on_success = props.on_success.clone();
        
        Callback::from(move |_| {
            error_state.set(String::new());
            success_state.set(String::new());
            loading_state.set(true);
            
            let error_state = error_state.clone();
            let success_state = success_state.clone();
            let loading_state = loading_state.clone();
            let on_error = on_error.clone();
            let on_success = on_success.clone();
            
            spawn_local(async move {
                // Get token from local storage
                let token = match window().and_then(|w| w.local_storage().ok()).flatten()
                    .and_then(|storage| storage.get_item("token").ok()).flatten() {
                    Some(token) => token,
                    None => {
                        error_state.set("Not authenticated".to_string());
                        on_error.emit("Not authenticated".to_string());
                        loading_state.set(false);
                        return;
                    }
                };
                
                let request = PurchaseRequest {
                    duration_minutes: Some(DEFAULT_DURATION_MINUTES),
                };
                
                match Request::post(&format!("{}/api/membership/purchase-temporary", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.status() {
                            200 => {
                                if let Ok(data) = response.json::<PurchaseResponse>().await {
                                    // Update currency in local storage
                                    if let Some(window) = window() {
                                        if let Some(storage) = window.local_storage().ok().flatten() {
                                            let _ = storage.set_item("currency", &data.new_balance.to_string());
                                        }
                                        
                                        // Dispatch currency update event
                                        let event_init = web_sys::CustomEventInit::new();
                                        event_init.set_detail(&wasm_bindgen::JsValue::from_f64(data.new_balance as f64));
                                        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                            "currencyUpdate",
                                            &event_init
                                        ) {
                                            let _ = window.dispatch_event(&event);
                                        }
                                        
                                        // Dispatch membership update event
                                        dispatch_membership_event(true);
                                    }
                                    
                                    let formatted_expiry = format_date_time(&data.expires_at);
                                    let success_message = format!(
                                        "Membership activated for {} days! Expires at {}",
                                        data.duration_minutes / 1440,
                                        formatted_expiry
                                    );
                                    success_state.set(success_message.clone());
                                    on_success.emit(success_message);
                                } else {
                                    error_state.set("Failed to parse response".to_string());
                                    on_error.emit("Failed to parse response".to_string());
                                }
                            }
                            400 => {
                                if let Ok(text) = response.text().await {
                                    let error_text = text.clone();
                                    error_state.set(error_text);
                                    on_error.emit(text);
                                } else {
                                    error_state.set("Insufficient balance".to_string());
                                    on_error.emit("Insufficient balance".to_string());
                                }
                            }
                            _ => {
                                error_state.set(format!("Error: {}", response.status()));
                                on_error.emit(format!("Error: {}", response.status()));
                            }
                        }
                    }
                    Err(e) => {
                        error_state.set(format!("Network error: {}", e));
                        on_error.emit(format!("Network error: {}", e));
                    }
                }
                
                loading_state.set(false);
            });
        })
    };
    
    let has_enough_balance = *currency >= DEFAULT_COST;
    
    html! {
        <div class="space-y-4 mt-6 p-4 border border-gray-200 dark:border-gray-700 rounded-lg">
            <div class="flex flex-col">
                <h3 class={styles::TEXT_H3}>{"Quick Membership"}</h3>
                <p class="text-gray-600 dark:text-gray-300 mt-1">
                    {format!("Purchase a temporary {} day membership for {} pax", DEFAULT_DURATION_MINUTES / 1440, DEFAULT_COST)}
                </p>
            </div>
            
            <div class="flex flex-col space-y-2">
                {
                    if !has_enough_balance {
                        html! {
                            <div class="text-red-500 text-sm">
                                {format!("Insufficient balance. You need {} pax.", DEFAULT_COST)}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                
                {
                    if !error.is_empty() {
                        html! {
                            <div class="text-red-500 text-sm">{&*error}</div>
                        }
                    } else {
                        html! {}
                    }
                }
                
                {
                    if !success.is_empty() {
                        html! {
                            <div class="text-green-500 text-sm">{&*success}</div>
                        }
                    } else {
                        html! {}
                    }
                }
                
                <button 
                    class={if has_enough_balance { styles::BUTTON_PRIMARY } else { styles::BUTTON_DANGER }}
                    disabled={!has_enough_balance || *loading}
                    onclick={purchase_membership}
                >
                    {if *loading { "Processing..." } else { "Purchase Membership" }}
                </button>
            </div>
        </div>
    }
} 
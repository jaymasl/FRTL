use yew::prelude::*;
use web_sys::{window};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::styles;
use crate::config::get_api_base_url;
use crate::base::dispatch_membership_event;
use serde::Deserialize;
use js_sys::{Date, Object, Reflect};

// Function to format ISO date string to a more readable format (copied from settings.rs)
fn format_date_time(iso_string: &str) -> String {
    let date = Date::new(&wasm_bindgen::JsValue::from_str(iso_string));
    
    // Create options for formatting
    let options = Object::new();
    let _ = Reflect::set(&options, &"dateStyle".into(), &"medium".into());
    let _ = Reflect::set(&options, &"timeStyle".into(), &"short".into());
    
    // Format the date using toLocaleString
    date.to_locale_string("en-US", &options)
        .as_string()
        .unwrap_or_else(|| iso_string.to_string())
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct PatreonStatus {
    pub is_linked: bool,
    pub patreon_email: Option<String>,
    pub patron_status: Option<String>,
    pub is_member: bool,
    pub member_until: Option<String>,
}

#[derive(Deserialize)]
struct PatreonLinkResponse {
    success: bool,
    message: String,
    is_linked: bool,
    is_member: bool,
    patron_status: Option<String>,
    member_until: Option<String>,
}

#[function_component(PatreonLink)]
pub fn patreon_link() -> Html {
    let status = use_state(|| None::<PatreonStatus>);
    let loading = use_state(|| false);
    let error = use_state(String::new);
    let success = use_state(String::new);
    
    // Check for OAuth callback parameters
    {
        let status = status.clone();
        let loading = loading.clone();
        let error = error.clone();
        let success = success.clone();
        
        use_effect_with((), move |_| {
            if let Some(window) = window() {
                let location = window.location();
                if let Ok(search) = location.search() {
                    if search.contains("code=") {
                        // We have an OAuth code, process it
                        loading.set(true);
                        error.set(String::new());
                        
                        let token = match window.local_storage().ok().flatten()
                            .and_then(|storage| storage.get_item("token").ok()).flatten() {
                            Some(token) => token,
                            None => {
                                error.set("Not authenticated".to_string());
                                loading.set(false);
                                return Box::new(|| {}) as Box<dyn FnOnce()>;
                            }
                        };
                        
                        // Extract the code
                        let params = web_sys::UrlSearchParams::new_with_str(&search).unwrap();
                        let code = params.get("code").unwrap_or_default();
                        
                        spawn_local(async move {
                            match Request::post(&format!("{}/api/settings/patreon/oauth/callback", get_api_base_url()))
                                .header("Content-Type", "application/json")
                                .header("Authorization", &format!("Bearer {}", token))
                                .json(&serde_json::json!({ "code": code }))
                                .unwrap()
                                .send()
                                .await
                            {
                                Ok(response) => {
                                    match response.json::<PatreonLinkResponse>().await {
                                        Ok(data) => {
                                            if data.success {
                                                success.set(data.message);
                                                
                                                // Update status
                                                status.set(Some(PatreonStatus {
                                                    is_linked: data.is_linked,
                                                    patreon_email: None, // Will be filled when we fetch status
                                                    patron_status: data.patron_status,
                                                    is_member: data.is_member,
                                                    member_until: data.member_until,
                                                }));
                                                
                                                // Update membership status in UI
                                                if data.is_member {
                                                    dispatch_membership_event(true);
                                                }
                                                
                                                // Clean up URL parameters
                                                if let Some(history) = window.history().ok() {
                                                    let _ = history.replace_state_with_url(
                                                        &wasm_bindgen::JsValue::NULL,
                                                        "",
                                                        Some(&location.pathname().unwrap_or_default())
                                                    );
                                                }
                                            } else {
                                                error.set(data.message);
                                            }
                                        },
                                        Err(e) => {
                                            error.set(format!("Failed to parse response: {}", e));
                                        }
                                    }
                                },
                                Err(e) => {
                                    error.set(format!("Network error: {}", e));
                                }
                            }
                            
                            loading.set(false);
                        });
                    }
                }
            }
            
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }
    
    // Fetch current Patreon status
    {
        let status = status.clone();
        let loading = loading.clone();
        let error = error.clone();
        
        use_effect_with((), move |_| {
            loading.set(true);
            error.set(String::new());
            
            let token = match window().and_then(|w| w.local_storage().ok()).flatten()
                .and_then(|storage| storage.get_item("token").ok()).flatten() {
                Some(token) => token,
                None => {
                    error.set("Not authenticated".to_string());
                    loading.set(false);
                    return Box::new(|| {}) as Box<dyn FnOnce()>;
                }
            };
            
            spawn_local(async move {
                match Request::get(&format!("{}/api/settings/patreon/status", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            match response.json::<PatreonStatus>().await {
                                Ok(data) => {
                                    status.set(Some(data));
                                },
                                Err(e) => {
                                    error.set(format!("Failed to parse response: {}", e));
                                }
                            }
                        } else {
                            error.set(format!("Server error: {}", response.status()));
                        }
                    },
                    Err(e) => {
                        error.set(format!("Network error: {}", e));
                    }
                }
                
                loading.set(false);
            });
            
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }
    
    let handle_link_patreon = {
        let loading = loading.clone();
        let error = error.clone();
        
        Callback::from(move |_: MouseEvent| {
            loading.set(true);
            error.set(String::new());
            
            let token = match window().and_then(|w| w.local_storage().ok()).flatten()
                .and_then(|storage| storage.get_item("token").ok()).flatten() {
                Some(token) => token,
                None => {
                    error.set("Not authenticated".to_string());
                    loading.set(false);
                    return;
                }
            };
            
            let error_state = error.clone();
            let loading_state = loading.clone();
            
            spawn_local(async move {
                match Request::get(&format!("{}/api/settings/patreon/oauth/url", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.json::<serde_json::Value>().await {
                            Ok(data) => {
                                if let Some(url) = data.get("url").and_then(|v| v.as_str()) {
                                    // Redirect to Patreon OAuth URL
                                    if let Some(window) = window() {
                                        let _ = window.location().set_href(url);
                                    }
                                } else {
                                    error_state.set("Failed to get OAuth URL".to_string());
                                    loading_state.set(false);
                                }
                            },
                            Err(e) => {
                                error_state.set(format!("Failed to parse response: {}", e));
                                loading_state.set(false);
                            }
                        }
                    },
                    Err(e) => {
                        error_state.set(format!("Network error: {}", e));
                        loading_state.set(false);
                    }
                }
            });
        })
    };
    
    let handle_unlink = {
        let loading = loading.clone();
        let error = error.clone();
        let success = success.clone();
        let status = status.clone();
        
        Callback::from(move |_: MouseEvent| {
            loading.set(true);
            error.set(String::new());
            success.set(String::new());
            
            let token = match window().and_then(|w| w.local_storage().ok()).flatten()
                .and_then(|storage| storage.get_item("token").ok()).flatten() {
                Some(token) => token,
                None => {
                    error.set("Not authenticated".to_string());
                    loading.set(false);
                    return;
                }
            };
            
            let error_state = error.clone();
            let success_state = success.clone();
            let loading_state = loading.clone();
            let status_state = status.clone();
            
            spawn_local(async move {
                match Request::post(&format!("{}/api/settings/patreon/unlink", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.json::<serde_json::Value>().await {
                            Ok(data) => {
                                if let Some(success_val) = data.get("success").and_then(|v| v.as_bool()) {
                                    if success_val {
                                        if let Some(message) = data.get("message").and_then(|v| v.as_str()) {
                                            success_state.set(message.to_string());
                                        } else {
                                            success_state.set("Successfully unlinked Patreon account".to_string());
                                        }
                                        
                                        // Update status
                                        if let Some(mut current_status) = (*status_state).clone() {
                                            current_status.is_linked = false;
                                            current_status.patreon_email = None;
                                            current_status.patron_status = None;
                                            status_state.set(Some(current_status));
                                        }
                                    } else if let Some(message) = data.get("message").and_then(|v| v.as_str()) {
                                        error_state.set(message.to_string());
                                    } else {
                                        error_state.set("Failed to unlink Patreon account".to_string());
                                    }
                                }
                            },
                            Err(e) => {
                                error_state.set(format!("Failed to parse response: {}", e));
                            }
                        }
                    },
                    Err(e) => {
                        error_state.set(format!("Network error: {}", e));
                    }
                }
                
                loading_state.set(false);
            });
        })
    };
    
    html! {
        <div class="mt-8">
            <div class="mb-6">
                <h2 class={styles::TEXT_H2}>{"Patreon Integration"}</h2>
                <p class={styles::TEXT_BODY}>{"Link your Patreon account to automatically receive membership benefits."}</p>
            </div>
            
            if let Some(patreon_status) = &*status {
                <div class="mb-6">
                    <div class="p-4 rounded-lg bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
                        <h3 class={styles::TEXT_H3}>{"Current Status"}</h3>
                        
                        <div class="mt-4 space-y-4">
                            <div>
                                <span class={styles::TEXT_BODY}>{"Linked to Patreon:"}</span>
                                <span class={format!("block {} {}", 
                                    styles::TEXT_BODY, 
                                    if patreon_status.is_linked { "font-semibold text-green-600 dark:text-green-400" } else { "text-gray-500 dark:text-gray-400" }
                                )}>
                                    {if patreon_status.is_linked { "Yes" } else { "No" }}
                                </span>
                            </div>
                            
                            if patreon_status.is_linked {
                                if let Some(email) = &patreon_status.patreon_email {
                                    <div>
                                        <span class={styles::TEXT_BODY}>{"Patreon Email:"}</span>
                                        <span class={format!("block {} text-gray-600 dark:text-gray-300", styles::TEXT_BODY)}>
                                            {email}
                                        </span>
                                    </div>
                                }
                                
                                <div>
                                    <span class={styles::TEXT_BODY}>{"Patron Status:"}</span>
                                    <span class={format!("block {} {}", 
                                        styles::TEXT_BODY,
                                        match patreon_status.patron_status.as_deref() {
                                            Some("active_patron") => "font-semibold text-green-600 dark:text-green-400",
                                            Some("declined_patron") => "font-semibold text-yellow-600 dark:text-yellow-400",
                                            Some("former_patron") => "font-semibold text-gray-600 dark:text-gray-400",
                                            _ => "text-gray-500 dark:text-gray-400"
                                        }
                                    )}>
                                        {match patreon_status.patron_status.as_deref() {
                                            Some("active_patron") => "Active",
                                            Some("declined_patron") => "Payment Declined",
                                            Some("former_patron") => "Former Patron",
                                            None => "Unknown",
                                            Some(other) => other,
                                        }}
                                    </span>
                                </div>
                                
                                <div>
                                    <span class={styles::TEXT_BODY}>{"Membership Status:"}</span>
                                    <span class={format!("block {} {}", 
                                        styles::TEXT_BODY,
                                        if patreon_status.is_member { "font-semibold text-green-600 dark:text-green-400" } else { "text-gray-500 dark:text-gray-400" }
                                    )}>
                                        {if patreon_status.is_member { "Active" } else { "Inactive" }}
                                    </span>
                                </div>
                                
                                if patreon_status.is_member {
                                    if let Some(until) = &patreon_status.member_until {
                                        <div>
                                            <span class={styles::TEXT_BODY}>{"Membership Until:"}</span>
                                            <span class={format!("block {} text-gray-600 dark:text-gray-300", styles::TEXT_BODY)}>
                                                {format_date_time(until)}
                                            </span>
                                        </div>
                                    }
                                }
                                
                                <div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
                                    <button 
                                        onclick={handle_unlink}
                                        disabled={*loading}
                                        class={format!("{} {}", 
                                            styles::BUTTON_DANGER,
                                            if *loading { "opacity-50 cursor-not-allowed" } else { "" }
                                        )}
                                    >
                                        {if *loading { "Processing..." } else { "Unlink Patreon Account" }}
                                    </button>
                                </div>
                            } else {
                                <div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
                                    <button 
                                        onclick={handle_link_patreon}
                                        disabled={*loading}
                                        class={format!("{} {}", 
                                            styles::BUTTON_PRIMARY,
                                            if *loading { "opacity-50 cursor-not-allowed" } else { "" }
                                        )}
                                    >
                                        {if *loading { "Processing..." } else { "Link Patreon Account" }}
                                    </button>
                                </div>
                            }
                        </div>
                    </div>
                </div>
            } else if *loading {
                <div class="flex justify-center items-center py-8">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
                </div>
            }
            
            if !error.is_empty() {
                <div class="mb-4 p-3 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/30 text-red-700 dark:text-red-400">
                    {&*error}
                </div>
            }
            
            if !success.is_empty() {
                <div class="mb-4 p-3 rounded-md bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800/30 text-green-700 dark:text-green-400">
                    {&*success}
                </div>
            }
            
            <div class="mt-8 p-4 rounded-lg bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800/30">
                <h3 class="text-lg font-medium text-blue-800 dark:text-blue-300 mb-2">{"About Patreon Integration"}</h3>
                <p class="text-sm text-blue-700 dark:text-blue-400">
                    {"Linking your Patreon account provides automatic membership benefits when you become a patron. Your membership will be automatically activated and renewed as long as you remain an active patron."}
                </p>
            </div>
        </div>
    }
} 
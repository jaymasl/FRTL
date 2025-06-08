use gloo_net::http::Request;
use serde::{Serialize, Deserialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement};
use yew::prelude::*;
use crate::styles;
use crate::config::get_api_base_url;
use gloo_timers::callback::Interval;
use js_sys::Date;
use crate::base::dispatch_membership_event;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

// Update interval in milliseconds
const UPDATE_INTERVAL: u32 = 1000;

#[derive(Serialize)]
struct RedeemCodeRequest {
    code: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
struct MembershipStatus {
    is_member: bool,
    member_until: Option<String>,
}

// Add a new struct to pass more detailed information to the parent component
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MembershipDetails {
    pub is_member: bool,
    pub member_until: Option<String>,
    pub remaining_seconds: i32,
    pub expiration_time_ms: f64,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub on_error: Callback<String>,
    #[prop_or_default]
    pub on_success: Option<Callback<String>>,
}

#[function_component(MembershipCode)]
pub fn membership_code(props: &Props) -> Html {
    let code = use_state(String::new);
    let error = use_state(String::new);
    let success = use_state(String::new);
    let loading = use_state(|| false);
    let is_initializing = use_state(|| true);
    let membership_status = use_state(|| MembershipStatus { is_member: false, member_until: None });
    let remaining_seconds = use_state(|| 0);
    let code_redeemed = use_state(|| false);
    let initialized = use_state(|| false);
    let expiration_time_ms = use_state(|| 0.0);

    // Initial fetch of membership status
    {
        let membership_status = membership_status.clone();
        let remaining_seconds = remaining_seconds.clone();
        let initialized = initialized.clone();
        let expiration_time_ms = expiration_time_ms.clone();
        let on_error = props.on_error.clone();
        let is_initializing = is_initializing.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                match fetch_membership_status().await {
                    Ok(status) => {
                        // Update membership status
                        membership_status.set(status.clone());
                        
                        // Calculate remaining seconds and expiration time
                        if status.is_member {
                            if let Some(time_str) = &status.member_until {
                                if let Ok(time) = chrono::DateTime::parse_from_rfc3339(time_str) {
                                    let time_utc = time.with_timezone(&chrono::Utc);
                                    let now = chrono::Utc::now();
                                    
                                    if time_utc > now {
                                        let diff = time_utc.signed_duration_since(now);
                                        let seconds = diff.num_seconds() as i32;
                                        remaining_seconds.set(seconds);
                                        
                                        // Store expiration time in milliseconds for timer updates
                                        let now_ms = Date::now();
                                        let expiration_ms = now_ms + (seconds as f64 * 1000.0);
                                        expiration_time_ms.set(expiration_ms);
                                    }
                                }
                            }
                        }
                        
                        initialized.set(true);
                        is_initializing.set(false);
                    },
                    Err(e) => {
                        on_error.emit(format!("Failed to fetch membership status: {}", e));
                        initialized.set(true);
                        is_initializing.set(false);
                    }
                }
            });
            || ()
        });
    }

    // Update timer based on expiration time
    {
        let remaining_seconds = remaining_seconds.clone();
        let expiration_time_ms = expiration_time_ms.clone();
        let membership_status = membership_status.clone();
        let initialized = initialized.clone();

        use_effect_with((*initialized, *expiration_time_ms), move |(initialized, _)| {
            let interval_handle = if *initialized && *expiration_time_ms > 0.0 {
                let remaining_seconds = remaining_seconds.clone();
                let expiration_time_ms = expiration_time_ms.clone();
                let membership_status = membership_status.clone();
                
                let interval = Interval::new(UPDATE_INTERVAL, move || {
                    let expiration_ms = *expiration_time_ms;
                    if expiration_ms <= 0.0 || !membership_status.is_member {
                        return;
                    }
                    
                    let now_ms = Date::now();
                    if now_ms >= expiration_ms {
                        // Membership expired, refresh status from server
                        let membership_status = membership_status.clone();
                        spawn_local(async move {
                            if let Ok(status) = fetch_membership_status().await {
                                membership_status.set(status);
                            }
                        });
                        remaining_seconds.set(0);
                    } else {
                        // Calculate remaining seconds
                        let seconds_remaining = ((expiration_ms - now_ms) / 1000.0).ceil() as i32;
                        remaining_seconds.set(seconds_remaining);
                    }
                });
                Some(interval)
            } else {
                None
            };
            
            move || {
                if let Some(interval) = interval_handle {
                    drop(interval);
                }
            }
        });
    }

    // Periodically refresh membership status from server
    {
        let membership_status = membership_status.clone();
        let remaining_seconds = remaining_seconds.clone();
        let expiration_time_ms = expiration_time_ms.clone();
        let initialized = initialized.clone();

        use_effect_with(*initialized, move |initialized| {
            let interval_handle = if *initialized {
                let membership_status = membership_status.clone();
                let remaining_seconds = remaining_seconds.clone();
                let expiration_time_ms = expiration_time_ms.clone();
                
                // Refresh every 10 seconds
                let interval = Interval::new(10000, move || {
                    let membership_status = membership_status.clone();
                    let remaining_seconds = remaining_seconds.clone();
                    let expiration_time_ms = expiration_time_ms.clone();
                    
                    spawn_local(async move {
                        if let Ok(status) = fetch_membership_status().await {
                            // Update membership status
                            membership_status.set(status.clone());
                            
                            // Update expiration time if member
                            if status.is_member {
                                if let Some(time_str) = &status.member_until {
                                    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(time_str) {
                                        let time_utc = time.with_timezone(&chrono::Utc);
                                        let now = chrono::Utc::now();
                                        
                                        if time_utc > now {
                                            let diff = time_utc.signed_duration_since(now);
                                            let seconds = diff.num_seconds() as i32;
                                            remaining_seconds.set(seconds);
                                            
                                            // Store expiration time in milliseconds for timer updates
                                            let now_ms = Date::now();
                                            let expiration_ms = now_ms + (seconds as f64 * 1000.0);
                                            expiration_time_ms.set(expiration_ms);
                                        } else {
                                            remaining_seconds.set(0);
                                            expiration_time_ms.set(0.0);
                                        }
                                    }
                                }
                            } else {
                                remaining_seconds.set(0);
                                expiration_time_ms.set(0.0);
                            }
                        }
                    });
                });
                Some(interval)
            } else {
                None
            };
            
            move || {
                if let Some(interval) = interval_handle {
                    drop(interval);
                }
            }
        });
    }

    // Helper function to fetch membership status
    async fn fetch_membership_status() -> Result<MembershipStatus, String> {
        let token = match get_token() {
            Some(token) => token,
            None => return Err("Not authenticated".to_string()),
        };

        match Request::get(&format!("{}/api/membership/status", get_api_base_url()))
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == 200 {
                    let status = response.json::<MembershipStatus>().await
                        .map_err(|e| format!("Failed to parse response: {:?}", e))?;
                    
                    // Get current status from local storage to check if it changed
                    let current_status = window()
                        .and_then(|w| w.local_storage().ok().flatten())
                        .and_then(|s| s.get_item("is_member").ok().flatten())
                        .map_or(false, |s| s == "true");
                    
                    // If status changed, dispatch event
                    if current_status != status.is_member {
                        dispatch_membership_event(status.is_member);
                    }
                    
                    Ok(status)
                } else {
                    Err(format!("Server error: {}", response.status()))
                }
            }
            Err(e) => {
                Err(format!("Network error: {:?}", e))
            }
        }
    }

    // Helper function to get token
    fn get_token() -> Option<String> {
        let window = window().unwrap();
        let local_storage = window.local_storage().unwrap().unwrap();
        let session_storage = window.session_storage().unwrap().unwrap();
        
        let local_token = local_storage.get_item("token").unwrap();
        let session_token = session_storage.get_item("token").unwrap();
        
        match (local_token, session_token) {
            (Some(token), _) | (None, Some(token)) if !token.is_empty() => Some(token),
            _ => None,
        }
    }

    let handle_submit = {
        let code = code.clone();
        let error = error.clone();
        let success = success.clone();
        let loading = loading.clone();
        let on_error = props.on_error.clone();
        let on_success = props.on_success.clone();
        let membership_status = membership_status.clone();
        let code_redeemed = code_redeemed.clone();
        let remaining_seconds = remaining_seconds.clone();
        let expiration_time_ms = expiration_time_ms.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            loading.set(true);
            error.set(String::new());
            success.set(String::new());

            let code_value = (*code).clone();
            let success_state = success.clone();
            let loading_state = loading.clone();
            let error_state = error.clone();
            let on_error = on_error.clone();
            let on_success = on_success.clone();
            let membership_status = membership_status.clone();
            let code_redeemed = code_redeemed.clone();
            let remaining_seconds = remaining_seconds.clone();
            let expiration_time_ms = expiration_time_ms.clone();

            spawn_local(async move {
                redeem_code(
                    code_value,
                    success_state,
                    loading_state,
                    error_state,
                    on_error,
                    on_success,
                    membership_status,
                    code_redeemed,
                    remaining_seconds,
                    expiration_time_ms,
                ).await;
            });
        })
    };

    // Helper function to redeem a membership code
    async fn redeem_code(
        code_value: String,
        success_state: UseStateHandle<String>,
        loading_state: UseStateHandle<bool>,
        error_state: UseStateHandle<String>,
        on_error: Callback<String>,
        on_success: Option<Callback<String>>,
        membership_status: UseStateHandle<MembershipStatus>,
        code_redeemed: UseStateHandle<bool>,
        remaining_seconds: UseStateHandle<i32>,
        expiration_time_ms: UseStateHandle<f64>,
    ) {
        let token = match get_token() {
            Some(token) => token,
            None => {
                error_state.set("Not authenticated".to_string());
                on_error.emit("Not authenticated".to_string());
                loading_state.set(false);
                return;
            }
        };

        let request = RedeemCodeRequest {
            code: code_value,
        };

        match Request::post(&format!("{}/api/membership/redeem", get_api_base_url()))
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .json(&request)
            .unwrap()
            .send()
            .await 
        {
            Ok(response) => {
                if response.status() == 200 {
                    let success_message = "Membership code redeemed successfully!".to_string();
                    success_state.set(success_message.clone());
                    code_redeemed.set(true);
                    
                    // Refresh membership status immediately
                    if let Ok(status) = fetch_membership_status().await {
                        membership_status.set(status.clone());
                        
                        let mut updated_expiry_ms = 0.0;
                        
                        // If we have a member_until date, update the expiration time
                        if let Some(member_until) = &status.member_until {
                            if let Some(window) = web_sys::window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    let js_value = wasm_bindgen::JsValue::from_str(member_until);
                                    let date = Date::new(&js_value);
                                    let expiry_ms = date.get_time();
                                    let _ = storage.set_item("membership_expiry", &expiry_ms.to_string());
                                    updated_expiry_ms = expiry_ms;
                                    
                                    // Immediately update the remaining seconds
                                    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(member_until) {
                                        let time_utc = time.with_timezone(&chrono::Utc);
                                        let now = chrono::Utc::now();
                                        
                                        if time_utc > now {
                                            let diff = time_utc.signed_duration_since(now);
                                            let seconds = diff.num_seconds() as i32;
                                            remaining_seconds.set(seconds);
                                            
                                            // Update expiration time for timer updates
                                            let now_ms = Date::now();
                                            let expiration_ms = now_ms + (seconds as f64 * 1000.0);
                                            expiration_time_ms.set(expiration_ms);
                                            updated_expiry_ms = expiration_ms;
                                            
                                            // Immediately update local storage with the new expiry time
                                            if let Some(window) = web_sys::window() {
                                                if let Ok(Some(storage)) = window.local_storage() {
                                                    let _ = storage.set_item("membership_expiry", &expiration_ms.to_string());
                                                    
                                                    // Also set a flag to indicate we just redeemed a code
                                                    let _ = storage.set_item("code_just_redeemed", "true");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Call the on_success callback if provided with enhanced information
                        if let Some(callback) = on_success {
                            // First call with the standard success message
                            callback.emit(success_message);
                            
                            // Force a refresh of the parent component's timer
                            if status.is_member {
                                // Use a small timeout to ensure the parent component has processed the first callback
                                let window = web_sys::window().unwrap();
                                let callback = Callback::from(move |_| {
                                    if let Some(window) = web_sys::window() {
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            // Update the membership_expiry in local storage again to ensure it's picked up
                                            if updated_expiry_ms > 0.0 {
                                                let _ = storage.set_item("membership_expiry", &updated_expiry_ms.to_string());
                                            }
                                            
                                            // Dispatch a custom event to force a refresh of the parent component
                                            let init = web_sys::CustomEventInit::new();
                                            init.set_bubbles(true);
                                            let event = web_sys::CustomEvent::new_with_event_init_dict(
                                                "membershipUpdated",
                                                &init,
                                            ).unwrap();
                                            window.dispatch_event(&event).unwrap();
                                            
                                            // Dispatch another event after a short delay to ensure the timer is updated
                                            let window_clone = window.clone();
                                            let timeout_callback = Callback::from(move |_| {
                                                let init = web_sys::CustomEventInit::new();
                                                init.set_bubbles(true);
                                                let event = web_sys::CustomEvent::new_with_event_init_dict(
                                                    "membershipUpdated",
                                                    &init,
                                                ).unwrap();
                                                window_clone.dispatch_event(&event).unwrap();
                                            });
                                            
                                            let closure = Closure::wrap(Box::new(move || {
                                                timeout_callback.emit(());
                                            }) as Box<dyn FnMut()>);
                                            
                                            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                                                closure.as_ref().unchecked_ref(),
                                                500,
                                            );
                                            closure.forget();
                                        }
                                    }
                                });
                                
                                let closure = Closure::wrap(Box::new(move || {
                                    callback.emit(());
                                }) as Box<dyn FnMut()>);
                                
                                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                                    closure.as_ref().unchecked_ref(),
                                    100,
                                );
                                closure.forget();
                            }
                        }
                        
                        // Dispatch event to update UI components across the app
                        dispatch_membership_event(status.is_member);
                    }
                } else {
                    let error_text = response.text().await.unwrap_or_else(|_| 
                        "Failed to redeem membership code".to_string()
                    );
                    error_state.set(error_text.clone());
                    on_error.emit(error_text);
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to send request: {:?}", e);
                error_state.set(error_msg.clone());
                on_error.emit(error_msg);
            }
        }
        loading_state.set(false);
    }

    html! {
        <div class="space-y-4">
            if *is_initializing {
                <div class="animate-pulse">
                    <div class="h-8 bg-gray-200 dark:bg-gray-700 rounded mb-2 w-3/4"></div>
                    <div class="h-10 bg-gray-200 dark:bg-gray-700 rounded"></div>
                </div>
            } else {
                // Only show the form if user is not a member and hasn't just redeemed a code
                if !membership_status.is_member && !*code_redeemed {
                    <form onsubmit={handle_submit} class="space-y-4">
                        <div>
                            <label class={styles::TEXT_LABEL}>{"Membership Code"}</label>
                            <input
                                type="text"
                                required=true
                                placeholder="Enter your membership code"
                                class={styles::INPUT}
                                onchange={let code = code.clone(); move |e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    code.set(input.value());
                                }}
                            />
                        </div>
                        <div>
                            if *loading {
                                <div class={styles::LOADING_SPINNER}></div>
                            } else {
                                <button type="submit" class={styles::BUTTON_PRIMARY}>{"Redeem Code"}</button>
                            }
                        </div>
                    </form>
                }

                if !(*error).is_empty() {
                    <div class={styles::CARD_ERROR}>
                        <p>{&*error}</p>
                    </div>
                }
                if !(*success).is_empty() {
                    <div class={styles::CARD_SUCCESS}>
                        <p>{&*success}</p>
                    </div>
                }
            }
        </div>
    }
} 
use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use serde::Deserialize;
use crate::config::get_api_base_url;
use web_sys::window;
use gloo_timers::callback::Interval;
use crate::base::dispatch_membership_event;

const UPDATE_INTERVAL: u32 = 1000; // Check every 1 second

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct MembershipStatus {
    pub is_member: bool,
    pub member_until: Option<String>,
    #[serde(default)]
    pub requires_membership: bool,
}

#[derive(Clone, PartialEq)]
pub struct MembershipInfo {
    pub is_member: bool,
    pub remaining_seconds: i32,
    pub loading: bool,
    pub member_until: Option<String>,
}

// Add a new struct to hold the result of a refresh operation
#[derive(Clone, Debug)]
pub struct RefreshResult {
    pub is_member: bool,
    pub remaining_seconds: i32,
    pub member_until: Option<String>,
}

impl MembershipInfo {
    // Add a method to refresh the membership status
    pub async fn refresh(&self) -> Result<RefreshResult, String> {
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
                    if let Ok(status) = response.json::<MembershipStatus>().await {
                        // Calculate remaining seconds
                        let mut remaining_secs = 0;
                        
                        if status.is_member && status.member_until.is_some() {
                            if let Ok(time) = chrono::DateTime::parse_from_rfc3339(&status.member_until.as_ref().unwrap()) {
                                let time_utc = time.with_timezone(&chrono::Utc);
                                let now = chrono::Utc::now();
                                
                                if time_utc > now {
                                    let diff = time_utc.signed_duration_since(now);
                                    remaining_secs = diff.num_seconds() as i32;
                                }
                            }
                        }
                        
                        // Update local storage with membership status
                        if let Some(window) = window() {
                            if let Ok(Some(storage)) = window.local_storage() {
                                let _ = storage.set_item("is_member", if status.is_member { "true" } else { "false" });
                                
                                // If we have a member_until date, update the expiration time
                                if let Some(member_until) = &status.member_until {
                                    let js_value = wasm_bindgen::JsValue::from_str(member_until);
                                    let date = js_sys::Date::new(&js_value);
                                    let expiry_ms = date.get_time();
                                    let _ = storage.set_item("membership_expiry", &expiry_ms.to_string());
                                }
                            }
                        }
                        
                        // Dispatch event to update UI components
                        dispatch_membership_event(status.is_member);
                        
                        return Ok(RefreshResult {
                            is_member: status.is_member,
                            remaining_seconds: remaining_secs,
                            member_until: status.member_until,
                        });
                    }
                }
                Err(format!("Failed to refresh membership status: {}", response.status()))
            },
            Err(e) => Err(format!("Network error: {:?}", e))
        }
    }
}

#[hook]
pub fn use_membership() -> MembershipInfo {
    let is_member = use_state(|| false);
    let remaining_seconds = use_state(|| 0);
    let loading = use_state(|| true);
    let member_until = use_state(|| None::<String>);
    
    // Initial fetch and periodic refresh
    {
        let is_member_clone = is_member.clone();
        let remaining_seconds_clone = remaining_seconds.clone();
        let loading_clone = loading.clone();
        let member_until_clone = member_until.clone();
        
        use_effect_with((), move |_| {
            // Initial fetch
            fetch_membership_status(is_member_clone.clone(), remaining_seconds_clone.clone(), loading_clone.clone(), member_until_clone.clone());
            
            // Set up interval for periodic refresh
            let interval = Interval::new(UPDATE_INTERVAL, move || {
                fetch_membership_status(is_member_clone.clone(), remaining_seconds_clone.clone(), loading_clone.clone(), member_until_clone.clone());
            });
            
            move || { drop(interval); }
        });
    }
    
    MembershipInfo {
        is_member: *is_member,
        remaining_seconds: *remaining_seconds,
        loading: *loading,
        member_until: (*member_until).clone(),
    }
}

fn fetch_membership_status(
    is_member: UseStateHandle<bool>,
    remaining_seconds: UseStateHandle<i32>,
    loading: UseStateHandle<bool>,
    member_until: UseStateHandle<Option<String>>
) {
    spawn_local(async move {
        let token = match get_token() {
            Some(token) => token,
            None => {
                loading.set(false);
                return;
            },
        };

        match Request::get(&format!("{}/api/membership/status", get_api_base_url()))
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == 200 {
                    if let Ok(status) = response.json::<MembershipStatus>().await {
                        // Calculate remaining seconds first to determine if membership has expired
                        let mut has_expired = false;
                        let mut remaining_secs = 0;
                        
                        if status.is_member && status.member_until.is_some() {
                            // Set the member_until value
                            member_until.set(status.member_until.clone());
                            
                            if let Ok(time) = chrono::DateTime::parse_from_rfc3339(&status.member_until.as_ref().unwrap()) {
                                let time_utc = time.with_timezone(&chrono::Utc);
                                let now = chrono::Utc::now();
                                
                                if time_utc > now {
                                    let diff = time_utc.signed_duration_since(now);
                                    remaining_secs = diff.num_seconds() as i32;
                                } else {
                                    // Membership has expired
                                    remaining_secs = 0;
                                    has_expired = true;
                                }
                            }
                        } else {
                            remaining_seconds.set(0);
                            member_until.set(None);
                        }
                        
                        // Update remaining seconds
                        remaining_seconds.set(remaining_secs);
                        
                        // Handle membership status update
                        let should_be_member = status.is_member && !has_expired;
                        
                        // Only update and dispatch event if the status has changed
                        if *is_member != should_be_member {
                            is_member.set(should_be_member);
                            // Dispatch event to update UI components
                            dispatch_membership_event(should_be_member);
                            
                            // Update local storage with membership status
                            if let Some(window) = window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    let _ = storage.set_item("is_member", if should_be_member { "true" } else { "false" });
                                    
                                    // If membership expired, clear the expiration time
                                    if has_expired {
                                        let _ = storage.set_item("membership_expiry", "0");
                                    }
                                }
                            }
                        } else {
                            is_member.set(should_be_member);
                        }
                        
                        // If membership is active, ensure local storage is updated with expiry time
                        if should_be_member && status.member_until.is_some() {
                            if let Some(window) = window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    if let Some(member_until_str) = &status.member_until {
                                        let js_value = wasm_bindgen::JsValue::from_str(member_until_str);
                                        let date = js_sys::Date::new(&js_value);
                                        let expiry_ms = date.get_time();
                                        let _ = storage.set_item("membership_expiry", &expiry_ms.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                loading.set(false);
            },
            Err(_) => {
                loading.set(false);
            }
        }
    });
}

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
use yew::prelude::*;
use serde::Deserialize;
use web_sys::{window, CustomEvent, CustomEventInit};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use gloo_timers::callback::Interval;
use gloo_timers::future::TimeoutFuture;
use std::sync::atomic::{AtomicI64, Ordering};
use crate::config::get_api_base_url;
use crate::hooks::use_membership::use_membership;
use crate::components::membership_required::MembershipRequired;

pub const CURRENCY_UPDATE_EVENT: &str = "currencyUpdate";
const UPDATE_INTERVAL: u32 = 1000;
const STREAK_RESET_WINDOW: f64 = 169600.0; // Changed from 86400.0 (24 hours) to 169600.0 (47 hours)
const EXPIRY_BUFFER: f64 = 0.5; // 0.5 second buffer for edge cases
const DAILY_CLAIM_AMOUNT: i32 = 10; // Base reward amount
const DAILY_CLAIM_COOLDOWN: f32 = 82800.0; // 23 hours (was 10 seconds)
const SCROLL_REWARD_DAY: i32 = 7; // Award scroll every 7th day

#[derive(Deserialize)]
struct ClaimResponse {
    success: bool,
    new_balance: i32,
    remaining_cooldown: i32,
    claim_streak: i32,
    message: Option<String>,
    #[serde(default)]
    scroll_reward: bool,
}

#[derive(Deserialize)]
struct ClaimStatusResponse {
    remaining_cooldown: i64,
    claim_streak: i32,
    last_claim_time: Option<i64>,
}

fn dispatch_currency_event(amount: i32) {
    if let Some(window) = window() {
        let event_init = CustomEventInit::new();
        event_init.set_detail(&JsValue::from_f64(amount as f64));
        let event = CustomEvent::new_with_event_init_dict(
            CURRENCY_UPDATE_EVENT,
            &event_init
        ).unwrap();
        window.dispatch_event(&event).unwrap();
    }
}

fn format_time(seconds: i32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

#[derive(Properties, PartialEq)]
pub struct ClaimButtonProps {
    pub on_success: Callback<String>,
    pub on_error: Callback<String>,
}

#[function_component(ClaimButton)]
pub fn claim_button(props: &ClaimButtonProps) -> Html {
    let loading = use_state(|| true);
    let remaining_cooldown = use_state(|| 0);
    let claim_streak = use_state(|| 0);
    let can_claim = use_state(|| false);
    let last_claim_time = use_state(|| 0.0);
    let streak_expiry = use_state(|| 0);
    let initialized = use_state(|| false);
    let server_confirmed = use_state(|| false);
    let pending_server_response = use_state(|| true);
    let scroll_awarded = use_state(|| false); // Track when a scroll was just awarded
    
    // Add membership check
    let membership = use_membership();
    
    // Fetch claim status on component mount
    {
        let loading = loading.clone();
        let can_claim = can_claim.clone();
        let remaining_cooldown = remaining_cooldown.clone();
        let initialized = initialized.clone();
        let server_confirmed = server_confirmed.clone();
        let pending_server_response = pending_server_response.clone();
        let on_error = props.on_error.clone();
        let claim_streak = claim_streak.clone();
        let streak_expiry = streak_expiry.clone();
        let last_claim_time = last_claim_time.clone();

        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                pending_server_response.set(true);
                match fetch_claim_status().await {
                    Ok(response) => {
                        let cooldown = response.remaining_cooldown;
                        remaining_cooldown.set(cooldown as i32);
                        can_claim.set(cooldown <= 0);
                        server_confirmed.set(true);
                        pending_server_response.set(false);
                        
                        // Update claim streak from server response
                        claim_streak.set(response.claim_streak);
                        
                        // Update last claim time if available
                        if let Some(last_time) = response.last_claim_time {
                            let last_time_ms = (last_time * 1000) as f64;
                            last_claim_time.set(last_time_ms);
                            
                            // Calculate streak expiry if applicable
                            if response.claim_streak > 0 {
                                let now = js_sys::Date::now();
                                let expiry_time = last_time_ms + (STREAK_RESET_WINDOW * 1000.0);
                                if now < expiry_time {
                                    streak_expiry.set(((expiry_time - now) / 1000.0).ceil() as i32);
                                }
                            }
                        }
                        
                        // Force a minimum loading time to ensure the user sees the loading state
                        let remaining_cooldown_clone = remaining_cooldown.clone();
                        let can_claim_clone = can_claim.clone();
                        let initialized_clone = initialized.clone();
                        let loading_clone = loading.clone();
                        
                        // Safety check: if remaining_cooldown > 0, ensure can_claim is false
                        if *remaining_cooldown_clone > 0 && *can_claim_clone {
                            can_claim_clone.set(false);
                        }
                        
                        // Set a timeout to ensure the loading state is visible for at least 800ms
                        spawn_local(async move {
                            TimeoutFuture::new(800).await;
                            initialized_clone.set(true);
                            loading_clone.set(false);
                        });
                    }
                    Err(err) => {
                        on_error.emit(format!("Failed to fetch claim status: {}", err));
                        pending_server_response.set(false);
                        initialized.set(true);
                        loading.set(false);
                    }
                }
            });
            || ()
        });
    }

    // Update cooldown timer - only run if we're initialized
    {
        let remaining_cooldown = remaining_cooldown.clone();
        let can_claim = can_claim.clone();
        let initialized = initialized.clone();
        
        // Create a ref to store the interval
        let interval_ref = std::rc::Rc::new(std::cell::RefCell::new(None));
        let interval_ref_clone = interval_ref.clone();
        
        use_effect_with((*initialized, *remaining_cooldown), move |(initialized, _)| {
            if !initialized {
                return Box::new(|| ()) as Box<dyn FnOnce()>;
            }
            
            // Only create the interval if there's a cooldown
            if *remaining_cooldown <= 0 {
                // Clear any existing interval
                if let Some(interval) = interval_ref_clone.borrow_mut().take() {
                    drop(interval);
                }
                return Box::new(|| ()) as Box<dyn FnOnce()>;
            }
            
            // Create the interval directly and store it in the ref
            let interval = Interval::new(1000, move || {
                // Decrement the remaining cooldown by 1 second each interval
                let current = *remaining_cooldown;
                if current > 0 {
                    remaining_cooldown.set(current - 1);
                    // Only update can_claim when we reach zero
                    if current == 1 {
                        can_claim.set(true);
                    }
                }
            });
            
            // Store the interval in the ref
            *interval_ref_clone.borrow_mut() = Some(interval);
            
            // Return a cleanup function that drops the interval
            Box::new(move || {
                if let Some(interval) = interval_ref_clone.borrow_mut().take() {
                    drop(interval);
                }
            }) as Box<dyn FnOnce()>
        });
    }

    // Update streak expiry timer - only run if we're initialized
    {
        let streak_expiry = streak_expiry.clone();
        let last_claim_time = last_claim_time.clone();
        let claim_streak = claim_streak.clone();
        let initialized = initialized.clone();
        
        use_effect_with((*initialized, *last_claim_time), move |(initialized, _)| {
            if !*initialized {
                return Box::new(|| ()) as Box<dyn FnOnce()>;
            }
            
            // Create an Option<Interval> that we can clear when needed
            let interval_handle = std::rc::Rc::new(std::cell::RefCell::new(None));
            let interval_handle_clone = interval_handle.clone();

            let interval = Interval::new(UPDATE_INTERVAL, move || {
                let last = *last_claim_time;
                if last <= 0.0 {
                    // Clear interval if last_claim_time is 0 (reset state)
                    if let Some(interval) = interval_handle_clone.borrow_mut().take() {
                        drop(interval);
                    }
                    return;
                }
                
                let now = js_sys::Date::now();
                let expiry_time = last + ((STREAK_RESET_WINDOW + EXPIRY_BUFFER) * 1000.0);
                
                if now >= expiry_time && *claim_streak > 0 {
                    // Use static to track last reset time
                    static LAST_RESET: AtomicI64 = AtomicI64::new(0);
                    
                    let current_time = (js_sys::Date::now() / 1000.0) as i64;
                    let last_reset = LAST_RESET.load(Ordering::Relaxed);
                    
                    // Only reset if:
                    // 1. More than 5 seconds since last reset
                    // 2. We have a valid streak
                    // 3. We're actually past expiry time
                    if current_time - last_reset > 5 && *claim_streak > 0 && now >= expiry_time {
                        LAST_RESET.store(current_time, Ordering::Relaxed);
                        
                        // Get token for backend request
                        let token = {
                            let window = window().unwrap();
                            let local_token = window.local_storage().unwrap().unwrap()
                                .get_item("token").unwrap();
                            let session_token = window.session_storage().unwrap().unwrap()
                                .get_item("token").unwrap();
                            
                            match (local_token, session_token) {
                                (Some(token), _) | (None, Some(token)) if !token.is_empty() => token,
                                _ => String::new()
                            }
                        };

                        if !token.is_empty() {
                            let claim_streak = claim_streak.clone();
                            let streak_expiry = streak_expiry.clone();
                            let interval_handle = interval_handle_clone.clone();
                            
                            wasm_bindgen_futures::spawn_local(async move {
                                match Request::post(&format!("{}/api/daily-claim/reset-streak", get_api_base_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await {
                                        Ok(response) => {
                                            if response.ok() {
                                                web_sys::console::log_1(&"Successfully reset streak".into());
                                                // Only update UI state after successful backend update
                                                if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                                                    storage.set_item("claim_streak", "0").ok();
                                                    storage.set_item("last_claim_time", "0").ok();
                                                }
                                                claim_streak.set(0);
                                                streak_expiry.set(0);
                                                
                                                // Clear the interval after successful reset
                                                if let Some(interval) = interval_handle.borrow_mut().take() {
                                                    drop(interval);
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            web_sys::console::log_1(&format!("Error resetting streak: {}", e).into());
                                        }
                                };
                            });
                        }
                    }
                } else if now < expiry_time && *claim_streak > 0 {
                    let remaining = ((expiry_time - now) / 1000.0).ceil() as i32;
                    streak_expiry.set(remaining);
                }
            });

            // Store the interval
            *interval_handle.borrow_mut() = Some(interval);

            Box::new(move || {
                // Cleanup interval on unmount
                if let Some(interval) = interval_handle.borrow_mut().take() {
                    drop(interval);
                }
            }) as Box<dyn FnOnce()>
        });
    }

    // Add a check at component mount to verify streak validity
    {
        let claim_streak = claim_streak.clone();
        let last_claim_time = last_claim_time.clone();
        
        use_effect_with((), move |_| {
            let now = js_sys::Date::now();
            let last = *last_claim_time;
            
            if last > 0.0 && now - last >= (STREAK_RESET_WINDOW * 1000.0) && *claim_streak > 0 {
                // If we're already expired at mount, reset immediately
                claim_streak.set(0);
                if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                    storage.set_item("claim_streak", "0").ok();
                }
            }
            || ()
        });
    }

    let onclick = {
        let loading = loading.clone();
        let remaining_cooldown = remaining_cooldown.clone();
        let claim_streak_state = claim_streak.clone();
        let can_claim = can_claim.clone();
        let on_success = props.on_success.clone();
        let on_error = props.on_error.clone();
        let last_claim_time = last_claim_time.clone();
        let _streak_expiry = streak_expiry.clone();
        let initialized = initialized.clone();
        let pending_server_response = pending_server_response.clone();
        let scroll_awarded = scroll_awarded.clone();

        Callback::from(move |_: MouseEvent| {
            // Double-check that we're initialized and can claim
            if !*initialized || !*can_claim || *loading || *pending_server_response {
                return;
            }
            
            // Extra safety check - make sure there's no remaining cooldown
            if *remaining_cooldown > 0 {
                web_sys::console::warn_1(&format!("Prevented claim attempt with {} seconds cooldown remaining", *remaining_cooldown).into());
                return;
            }
            
            loading.set(true);
            // Set can_claim to false immediately to prevent double-clicks
            can_claim.set(false);
            on_error.emit(String::new());
            on_success.emit(String::new());

            let token = {
                let window = window().unwrap();
                let local_token = window.local_storage().unwrap().unwrap()
                    .get_item("token").unwrap();
                let session_token = window.session_storage().unwrap().unwrap()
                    .get_item("token").unwrap();
                
                match (local_token, session_token) {
                    (Some(token), _) | (None, Some(token)) if !token.is_empty() => token,
                    _ => String::new()
                }
            };

            if token.is_empty() {
                loading.set(false);
                on_error.emit("Please log in again".to_string());
                return;
            }

            let loading_async = loading.clone();
            let remaining_cooldown_async = remaining_cooldown.clone();
            let claim_streak_state_async = claim_streak_state.clone();
            let can_claim_async = can_claim.clone();
            let on_success_async = on_success.clone();
            let on_error_async = on_error.clone();
            let last_claim_time_async = last_claim_time.clone();
            let scroll_awarded_async = scroll_awarded.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match Request::post(&format!("{}/api/daily-claim", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.status() {
                            200 => {
                                if let Ok(json) = response.json::<ClaimResponse>().await {
                                    if json.success {
                                        // Update the claim streak
                                        claim_streak_state_async.set(json.claim_streak);
                                        
                                        // Update remaining cooldown
                                        remaining_cooldown_async.set(json.remaining_cooldown);
                                        
                                        // Update last_claim_time to current time
                                        last_claim_time_async.set(js_sys::Date::now());
                                        
                                        // If we just claimed on day 7, reset to day 1 of the next week
                                        if get_day_in_week(json.claim_streak - 1) == SCROLL_REWARD_DAY {
                                            web_sys::console::log_1(&"Claimed on day 7, starting new week".into());
                                        }
                                        
                                        // Update scroll awarded state immediately if a scroll was awarded
                                        if json.scroll_reward {
                                            scroll_awarded_async.set(true);
                                            
                                            // Set a timeout to reset the scroll awarded state after 6 seconds
                                            let scroll_awarded_clone = scroll_awarded_async.clone();
                                            spawn_local(async move {
                                                TimeoutFuture::new(6000).await;
                                                scroll_awarded_clone.set(false);
                                            });
                                        }
                                        
                                        // Dispatch currency update event
                                        dispatch_currency_event(json.new_balance);
                                        
                                        // Show success message
                                        if let Some(msg) = json.message {
                                            on_success_async.emit(msg);
                                        }
                                        
                                        // If a scroll was awarded, dispatch scroll update event
                                        if json.scroll_reward {
                                            if let Some(window) = window() {
                                                let event_init = CustomEventInit::new();
                                                let event = CustomEvent::new_with_event_init_dict(
                                                    "scrollUpdate",
                                                    &event_init
                                                ).unwrap();
                                                window.dispatch_event(&event).unwrap();
                                            }
                                        }
                                    } else if let Some(msg) = json.message {
                                        on_error_async.emit(msg);
                                        // Reset can_claim if the claim failed
                                        can_claim_async.set(true);
                                    }
                                }
                            },
                            status => {
                                web_sys::console::error_1(&format!("Error status: {}", status).into());
                                match status {
                                    401 => on_error_async.emit("You must be logged in to claim rewards".to_string()),
                                    429 => on_error_async.emit("Too many requests. Please try again later".to_string()),
                                    500 => on_error_async.emit("Server error. Please try again later".to_string()),
                                    _ => on_error_async.emit("Failed to claim reward. Please try again".to_string()),
                                }
                                // Reset can_claim if the claim failed
                                can_claim_async.set(true);
                            }
                        }
                        loading_async.set(false);
                    },
                    Err(e) => {
                        web_sys::console::error_1(&format!("Network error: {:?}", e).into());
                        loading_async.set(false);
                        on_error_async.emit("Network error. Please check your connection and try again".to_string());
                        // Reset can_claim if the claim failed
                        can_claim_async.set(true);
                    }
                }
            });
        })
    };

    html! {
        <div class="flex flex-col items-center p-6 bg-gradient-to-br from-gray-50 to-gray-100 dark:from-gray-800 dark:to-gray-900 rounded-xl shadow-lg max-w-md w-full mx-auto border border-gray-200 dark:border-gray-700">
            // Header with streak counter
            <div class="w-full mb-6 text-center">
                <div class="text-2xl font-extrabold text-transparent bg-clip-text bg-gradient-to-r from-blue-500 to-purple-600 dark:from-blue-400 dark:to-purple-500">
                    { format!("Streak: {}", *claim_streak) }
                </div>
            </div>
            
            // Scroll award animation
            if *scroll_awarded {
                <div class="fixed inset-0 flex items-center justify-center pointer-events-none z-50">
                    <div class="relative">
                        /* Background glow effect */
                        <div class="absolute inset-0 bg-gradient-to-r from-orange-500/20 to-yellow-500/20 blur-xl rounded-full scale-150 animate-pulse"></div>
                        
                        /* Main container with glass effect */
                        <div class="relative bg-white/10 dark:bg-gray-900/30 backdrop-blur-md rounded-2xl p-8 shadow-2xl border border-white/20 dark:border-gray-700/30 overflow-hidden">
                            /* Content */
                            <div class="relative z-10 text-center">
                                /* Scroll icon with rays */
                                <div class="relative inline-block mb-4">
                                    /* Scroll icon */
                                    <div class="relative z-10 w-24 h-24 rounded-full bg-gradient-to-br from-orange-400 to-yellow-500 flex items-center justify-center shadow-lg animate-bounce">
                                        <svg class="w-14 h-14 text-white drop-shadow-md" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                            <path fill-rule="evenodd" d="M5 2a1 1 0 011 1v1h1a1 1 0 010 2H6v1a1 1 0 01-2 0V6H3a1 1 0 010-2h1V3a1 1 0 011-1zm0 10a1 1 0 011 1v1h1a1 1 0 110 2H6v1a1 1 0 11-2 0v-1H3a1 1 0 110-2h1v-1a1 1 0 011-1zM12 2a1 1 0 01.967.744L14.146 7.2 17.5 9.134a1 1 0 010 1.732l-3.354 1.935-1.18 4.455a1 1 0 01-1.933 0L9.854 12.8 6.5 10.866a1 1 0 010-1.732l3.354-1.935 1.18-4.455A1 1 0 0112 2z" clip-rule="evenodd"></path>
                                        </svg>
                                    </div>
                                </div>
                                
                                /* Text with animation */
                                <div class="mt-4 space-y-2">
                                    <h3 class="text-2xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-orange-500 to-yellow-500 animate-pulse">
                                        {"Scroll Awarded!"}
                                    </h3>
                                    <p class="text-gray-700 dark:text-gray-300">
                                        {"Use it to summon a new creature"}
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            }
            
            // Reward info card
            <div class="w-full p-4 mb-6 bg-white dark:bg-gray-800 rounded-lg shadow-md border border-gray-200 dark:border-gray-700">
                <div class="flex items-center justify-between">
                    <div class="flex items-center">
                        <svg class="w-6 h-6 text-yellow-500 mr-2" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                            <path d="M10 2a6 6 0 00-6 6v3.586l-.707.707A1 1 0 004 14h12a1 1 0 00.707-1.707L16 11.586V8a6 6 0 00-6-6zM10 18a3 3 0 01-3-3h6a3 3 0 01-3 3z"></path>
                        </svg>
                        <span class="font-medium text-gray-800 dark:text-white">{"Daily Reward"}</span>
                    </div>
                    <div class="text-right">
                        <div class="text-sm font-semibold text-gray-900 dark:text-white">
                            {format!("{} pax", calculate_reward(*claim_streak))}
                        </div>
                        <div class="text-xs text-gray-500 dark:text-gray-400">
                            {format!("Week {} reward", get_week_number(*claim_streak))}
                        </div>
                    </div>
                </div>
                
                // 7-day progress indicator
                <div class="mt-4 mb-4">
                    <div class="flex justify-between items-center mb-2">
                        <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{"Weekly Progress"}</span>
                        <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                            {format!("Day {}/7", get_day_in_week(*claim_streak))}
                        </span>
                    </div>
                    <div class="flex items-center justify-between space-x-2">
                        {
                            for (1..=7).map(|day| {
                                let current_day = get_day_in_week(*claim_streak);
                                let is_scroll_day = day == 7;
                                let is_current = day == current_day;
                                
                                html! {
                                    <div class="flex flex-col items-center">
                                        <div 
                                            class={classes!(
                                                "w-8", "h-8", "rounded-full", "flex", "items-center", "justify-center",
                                                "transition-all", "duration-300", "mb-1",
                                                "border-2",
                                                if is_current {
                                                    "border-blue-500 dark:border-blue-400"
                                                } else if day == current_day + 1 {
                                                    "border-blue-300 dark:border-blue-700"
                                                } else {
                                                    "border-transparent"
                                                },
                                                if is_scroll_day {
                                                    if day <= current_day {
                                                        "bg-gradient-to-br from-orange-400 to-orange-600 text-white shadow-lg"
                                                    } else if day == current_day + 1 {
                                                        "bg-gray-200 dark:bg-gray-700 text-orange-500 dark:text-orange-400"
                                                    } else {
                                                        "bg-gray-200 dark:bg-gray-700 text-gray-400 dark:text-gray-500"
                                                    }
                                                } else if day <= current_day {
                                                    "bg-gradient-to-br from-blue-400 to-blue-600 text-white shadow-md"
                                                } else if day == current_day + 1 {
                                                    "bg-gray-200 dark:bg-gray-700 text-blue-500 dark:text-blue-400"
                                                } else {
                                                    "bg-gray-200 dark:bg-gray-700 text-gray-400 dark:text-gray-500"
                                                }
                                            )}
                                        >
                                            {
                                                if is_scroll_day {
                                                    html! {
                                                        <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                                            <path fill-rule="evenodd" d="M5 2a1 1 0 011 1v1h1a1 1 0 010 2H6v1a1 1 0 01-2 0V6H3a1 1 0 010-2h1V3a1 1 0 011-1zm0 10a1 1 0 011 1v1h1a1 1 0 110 2H6v1a1 1 0 11-2 0v-1H3a1 1 0 110-2h1v-1a1 1 0 011-1zM12 2a1 1 0 01.967.744L14.146 7.2 17.5 9.134a1 1 0 010 1.732l-3.354 1.935-1.18 4.455a1 1 0 01-1.933 0L9.854 12.8 6.5 10.866a1 1 0 010-1.732l3.354-1.935 1.18-4.455A1 1 0 0112 2z" clip-rule="evenodd"></path>
                                                        </svg>
                                                    }
                                                } else {
                                                    html! { {day} }
                                                }
                                            }
                                        </div>
                                        <div class={classes!(
                                            "h-0.5", "w-6", 
                                            if day < 7 {
                                                if day <= current_day {
                                                    "bg-blue-500 dark:bg-blue-400"
                                                } else {
                                                    "bg-gray-200 dark:bg-gray-700"
                                                }
                                            } else {
                                                "bg-transparent"
                                            }
                                        )}/>
                                    </div>
                                }
                            })
                        }
                    </div>
                </div>
                
                <div class="mt-3 text-sm text-gray-600 dark:text-gray-400">
                    {"Your reward increases by 1 pax each week (every 7 days)"}
                </div>
                
                {
                    if *claim_streak > 0 {
                        html! {
                            <div class="mt-3 flex items-center">
                                <svg class="w-4 h-4 text-blue-500 mr-1" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"></path>
                                </svg>
                                <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {
                                        if *can_claim {
                                            format!("Streak expires in: {}", format_time(*streak_expiry))
                                        } else {
                                            "Maintain your streak by claiming daily".to_string()
                                        }
                                    }
                                </span>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                
                // Show next scroll reward info
                <div class="mt-3 flex items-center">
                    <svg class="w-4 h-4 text-orange-500 mr-1" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                        <path fill-rule="evenodd" d="M5 2a1 1 0 011 1v1h1a1 1 0 010 2H6v1a1 1 0 01-2 0V6H3a1 1 0 010-2h1V3a1 1 0 011-1zm0 10a1 1 0 011 1v1h1a1 1 0 110 2H6v1a1 1 0 11-2 0v-1H3a1 1 0 110-2h1v-1a1 1 0 011-1zM12 2a1 1 0 01.967.744L14.146 7.2 17.5 9.134a1 1 0 010 1.732l-3.354 1.935-1.18 4.455a1 1 0 01-1.933 0L9.854 12.8 6.5 10.866a1 1 0 010-1.732l3.354-1.935 1.18-4.455A1 1 0 0112 2z" clip-rule="evenodd"></path>
                    </svg>
                    <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                    {
                        if get_day_in_week(*claim_streak) == 6 {
                            "Claim again to receive a bonus scroll!".to_string()
                        } else if get_day_in_week(*claim_streak) == SCROLL_REWARD_DAY {
                            "Claim again to start a new week!".to_string()
                        } else {
                            let days_left = SCROLL_REWARD_DAY - get_day_in_week(*claim_streak);
                            format!("Receive a scroll on day 7 ({} days left)", days_left)
                        }
                    }
                    </span>
                </div>
            </div>
            
            // Claim button or status
            {
                if membership.loading {
                    html! {
                        <div class="w-full flex items-center justify-center p-4 bg-gray-100 dark:bg-gray-800 rounded-lg animate-pulse">
                            <svg class="animate-spin mr-3 h-5 w-5 text-blue-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                            <span class="text-gray-700 dark:text-gray-300 font-medium">{"Loading..."}</span>
                        </div>
                    }
                } else if !membership.is_member {
                    html! {
                        <MembershipRequired feature_name="Daily Claim" />
                    }
                } else {
                    html! {
                        <div class="w-full">
                            {
                                if *remaining_cooldown > 0 {
                                    html! {
                                        <div class="w-full">
                                            <div class="mb-2 flex justify-between items-center">
                                                <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{"Next claim available in:"}</span>
                                                <span class="text-sm font-bold text-blue-600 dark:text-blue-400">{format_time(*remaining_cooldown)}</span>
                                            </div>
                                            <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2.5">
                                                <div class="bg-gradient-to-r from-blue-500 to-purple-600 h-2.5 rounded-full transition-all duration-500" 
                                                     style={format!("width: {}%", (1.0 - (*remaining_cooldown as f32 / DAILY_CLAIM_COOLDOWN as f32)) * 100.0)}>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            
                            <button
                                onclick={onclick}
                                disabled={*loading || *remaining_cooldown > 0 || !*can_claim || !*initialized || *pending_server_response}
                                class={classes!(
                                    "w-full", "mt-2", "py-3", "px-4", "rounded-lg", "font-medium", "text-base", 
                                    "transition-all", "duration-300", "transform", "shadow-md",
                                    "flex", "items-center", "justify-center", "space-x-2",
                                    if *loading || *remaining_cooldown > 0 || !*can_claim || !*initialized || *pending_server_response {
                                        "bg-gray-300 dark:bg-gray-700 text-gray-600 dark:text-gray-400 cursor-not-allowed"
                                    } else {
                                        "bg-gradient-to-r from-blue-500 to-purple-600 hover:from-blue-600 hover:to-purple-700 text-white hover:shadow-lg active:scale-95"
                                    }
                                )}
                            >
                                {
                                    if !*initialized || *pending_server_response {
                                        html! {
                                            <>
                                                <svg class="animate-spin h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                </svg>
                                                <span>{"Loading claim status..."}</span>
                                            </>
                                        }
                                    } else if *loading {
                                        html! { 
                                            <>
                                                <svg class="animate-spin h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                </svg>
                                                <span>{"Claiming..."}</span>
                                            </> 
                                        }
                                    } else if *remaining_cooldown > 0 {
                                        html! { 
                                            <>
                                                <svg class="h-5 w-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
                                                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"></path>
                                                </svg>
                                                <span>{format!("Next claim in {}", format_time(*remaining_cooldown))}</span>
                                            </> 
                                        }
                                    } else if *can_claim && *remaining_cooldown == 0 && *server_confirmed {
                                        html! { 
                                            <>
                                                <span>{"Claim Daily Reward"}</span>
                                            </> 
                                        }
                                    } else {
                                        html! { 
                                            <>
                                                <svg class="animate-spin h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                </svg>
                                                <span>{"Checking claim status..."}</span>
                                            </> 
                                        }
                                    }
                                }
                            </button>
                        </div>
                    }
                }
            }
        </div>
    }
}

async fn fetch_claim_status() -> Result<ClaimStatusResponse, String> {
    // Get token for backend request
    let token = {
        let window = window().unwrap();
        let local_token = window.local_storage().unwrap().unwrap()
            .get_item("token").unwrap();
        let session_token = window.session_storage().unwrap().unwrap()
            .get_item("token").unwrap();
        
        match (local_token, session_token) {
            (Some(token), _) | (None, Some(token)) if !token.is_empty() => token,
            _ => String::new()
        }
    };

    if token.is_empty() {
        return Err("No authentication token found".to_string());
    }

    match Request::get(&format!("{}/api/daily-claim/status", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await {
        Ok(response) => {
            if response.ok() {
                match response.json::<ClaimStatusResponse>().await {
                    Ok(status) => Ok(status),
                    Err(e) => Err(format!("Error parsing status response: {:?}", e))
                }
            } else {
                Err(format!("Error status: {}", response.status()))
            }
        },
        Err(e) => Err(format!("Network error: {:?}", e))
    }
}

// Helper function to calculate the week number from streak
fn get_week_number(streak: i32) -> i32 {
    ((streak - 1) / SCROLL_REWARD_DAY) + 1
}

// Helper function to calculate the current day in the week (1-7)
fn get_day_in_week(streak: i32) -> i32 {
    let day = streak % SCROLL_REWARD_DAY;
    if day == 0 && streak > 0 {
        SCROLL_REWARD_DAY
    } else {
        day
    }
}

// Helper function to calculate the reward amount based on streak
fn calculate_reward(streak: i32) -> i32 {
    let week_number = get_week_number(streak);
    DAILY_CLAIM_AMOUNT + (week_number - 1)
}
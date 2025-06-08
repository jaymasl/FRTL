use yew::prelude::*;
use web_sys::window;
use crate::hooks::{auth_state::use_auth_check, form_state::use_form_state, use_membership::use_membership};
use js_sys::Date;
use gloo_timers::callback::Timeout;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsCast;
use crate::components::settings::{
    AccountManagement, MembershipCode, TemporaryMembership, PatreonLink
};

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

// Function to format seconds into a human-readable duration
fn format_duration(seconds: i32) -> String {
    if seconds <= 0 {
        return "expired".to_string();
    }
    
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    format!("{}d {}h {}m {}s", days, hours, minutes, secs)
}

#[function_component(Settings)]
pub fn settings() -> Html {
    use_auth_check();
    let form_state = use_form_state();
    let membership = use_membership();
    let just_purchased = use_state(|| false);
    let is_loading = use_state(|| true);
    let is_activating = use_state(|| false);
    let force_refresh = use_state(|| 0);
    
    // Store the expiry timestamp (not the remaining seconds)
    let expiry_time = use_state(|| {
        if let Some(member_until) = &membership.member_until {
            let js_value = wasm_bindgen::JsValue::from_str(member_until);
            let date = Date::new(&js_value);
            return date.get_time();
        }
        
        // If no valid member_until time, check local storage
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(expiry_str)) = storage.get_item("membership_expiry") {
                    if let Ok(expiry_time) = expiry_str.parse::<f64>() {
                        let now = Date::new_0().get_time();
                        if expiry_time > now {
                            return expiry_time;
                        }
                    }
                }
            }
        }
        
        // If no valid expiry time in storage, use remaining seconds from membership
        let now = Date::new_0().get_time();
        now + (membership.remaining_seconds.max(0) as f64 * 1000.0)
    });
    
    // Calculate remaining time based on membership data
    let remaining_time = use_state(|| membership.remaining_seconds);

    // Set loading and initialization state once membership data is available
    {
        let is_loading = is_loading.clone();
        
        use_effect_with(membership.clone(), move |membership| {
            // Only set loading to false when we're sure the membership data has been loaded
            if !membership.loading {
                // Small delay to ensure all data is processed before showing content
                let is_loading_clone = is_loading.clone();
                
                // Use a short timeout to ensure smooth transition
                let timeout = Timeout::new(300, move || {
                    is_loading_clone.set(false);
                });
                
                // Return a cleanup function
                return Box::new(move || {
                    drop(timeout);
                }) as Box<dyn FnOnce()>;
            }
            
            // No cleanup needed if we didn't set a timer
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }
    
    // Add an event listener for the custom membershipUpdated event
    {
        let force_refresh = force_refresh.clone();
        let expiry_time = expiry_time.clone();
        
        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            
            let force_refresh_clone = force_refresh.clone();
            let expiry_time_clone = expiry_time.clone();
            
            let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::Event| {
                // Increment the force_refresh counter to trigger a re-render
                force_refresh_clone.set(*force_refresh_clone + 1);
                
                // Check for updated expiry time in local storage
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(expiry_str)) = storage.get_item("membership_expiry") {
                            if let Ok(stored_expiry) = expiry_str.parse::<f64>() {
                                let now = Date::new_0().get_time();
                                if stored_expiry > now {
                                    expiry_time_clone.set(stored_expiry);
                                }
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "membershipUpdated",
                closure.as_ref().unchecked_ref(),
            ).unwrap();
            
            move || {
                document.remove_event_listener_with_callback(
                    "membershipUpdated",
                    closure.as_ref().unchecked_ref(),
                ).unwrap();
                drop(closure);
            }
        });
    }
    
    // Update remaining_time when membership.remaining_seconds changes
    // Also check if membership has expired when timer reaches zero
    {
        let remaining_time = remaining_time.clone();
        let membership_clone = membership.clone();
        let previous_remaining_time = use_state(|| membership.remaining_seconds);
        
        use_effect_with((membership.remaining_seconds, *remaining_time), move |(current_remaining, displayed_remaining)| {
            // Update the displayed remaining time from membership data
            remaining_time.set(*current_remaining);
            previous_remaining_time.set(*displayed_remaining);
            
            // If the timer just reached zero, trigger a refresh of the membership status
            if *displayed_remaining > 0 && *current_remaining <= 0 {
                // Membership just expired while user was viewing
                let membership_for_refresh = membership_clone.clone();
                spawn_local(async move {
                    if let Ok(_) = membership_for_refresh.refresh().await {
                        // The refresh will update the UI through the use_membership hook
                        log::info!("Membership refreshed after expiration");
                    }
                });
            }
            || ()
        });
    }
    
    // Update expiry_time when membership.remaining_seconds or member_until changes
    {
        let expiry_time = expiry_time.clone();
        let membership_clone = membership.clone();
        let force_refresh = force_refresh.clone();
        
        use_effect_with((membership.remaining_seconds, membership.member_until.clone(), *force_refresh), move |_| {
            // Check if we have a valid member_until date
            if let Some(member_until) = &membership_clone.member_until {
                let js_value = wasm_bindgen::JsValue::from_str(member_until);
                let date = Date::new(&js_value);
                let new_expiry = date.get_time();
                expiry_time.set(new_expiry);
                
                // Update the stored expiry time
                if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.set_item("membership_expiry", &new_expiry.to_string());
                    }
                }
            } else if membership_clone.remaining_seconds > 0 {
                // If no member_until but we have remaining seconds, use that
                let now = Date::new_0().get_time();
                let new_expiry = now + (membership_clone.remaining_seconds as f64 * 1000.0);
                expiry_time.set(new_expiry);
                
                // Update the stored expiry time
                if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.set_item("membership_expiry", &new_expiry.to_string());
                    }
                }
            }
            || ()
        });
    }
    
    // Update expiry_time when a temporary membership is purchased
    {
        let expiry_time = expiry_time.clone();
        let just_purchased_value = *just_purchased;
        let membership_clone = membership.clone();
        
        use_effect_with((just_purchased_value, membership_clone.member_until.clone()), move |_| {
            if just_purchased_value {
                // First check if we have a valid member_until date from the membership status
                if let Some(member_until) = &membership_clone.member_until {
                    let js_value = wasm_bindgen::JsValue::from_str(member_until);
                    let date = Date::new(&js_value);
                    expiry_time.set(date.get_time());
                } else if membership_clone.remaining_seconds > 0 {
                    // If no member_until but we have remaining seconds, use that
                    let now = Date::new_0().get_time();
                    let new_expiry = now + (membership_clone.remaining_seconds as f64 * 1000.0);
                    expiry_time.set(new_expiry);
                }
                
                // Update the stored expiry time
                if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.set_item("membership_expiry", &expiry_time.to_string());
                    }
                }
            }
            || ()
        });
    }
    
    // Check for code_just_redeemed flag on component mount
    {
        let just_purchased = just_purchased.clone();
        
        use_effect_with((), move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(flag)) = storage.get_item("code_just_redeemed") {
                        if flag == "true" {
                            // Clear the flag
                            let _ = storage.set_item("code_just_redeemed", "false");
                            
                            // Set just_purchased to true without showing the activation overlay
                            // since we're just loading the page after a redirect
                            just_purchased.set(true);
                        }
                    }
                }
            }
            || ()
        });
    }
    
    // Set up a timer to check if the membership has expired
    {
        let membership_clone = membership.clone();
        let expiry_time_value = *expiry_time;
        
        use_effect_with(expiry_time_value, move |_| {
            let now = Date::new_0().get_time();
            let time_until_expiry = expiry_time_value - now;
            
            // Only set up a timer if expiry is in the future and within the next hour
            if time_until_expiry > 0.0 && time_until_expiry < 3600000.0 {
                // Calculate milliseconds until expiry (plus a small buffer)
                let ms_until_expiry = time_until_expiry.max(0.0) as u32 + 1000;
                
                // Set a one-time timer that will fire when membership expires
                let timeout = Timeout::new(ms_until_expiry, move || {
                    // When the timer fires, refresh the membership status
                    let membership_for_refresh = membership_clone.clone();
                    spawn_local(async move {
                        if let Ok(_) = membership_for_refresh.refresh().await {
                            log::info!("Membership refreshed after expiration timer");
                        }
                    });
                });
                
                // Return a cleanup function
                return Box::new(move || {
                    drop(timeout);
                }) as Box<dyn FnOnce()>;
            }
            
            // No cleanup needed if we didn't set a timer
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }
    
    html! {
        <crate::base::Base>
            <crate::components::GradientBackground>
                <style>
                    {r#"
                    @keyframes fadeIn {
                        from { opacity: 0; }
                        to { opacity: 1; }
                    }
                    @keyframes fadeOut {
                        from { opacity: 1; }
                        to { opacity: 0; }
                    }
                    "#}
                </style>
                <div class="min-h-screen w-full px-4 sm:px-6 lg:px-8">
                    <div class="max-w-md mx-auto px-4 sm:px-6 py-4">
                        <div class={crate::styles::CARD}>
                            <div class="flex items-center space-x-4 mb-8">
                                <div class={crate::styles::ICON_WRAPPER_BLUE}>
                                    <svg class={crate::styles::ICON} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                                            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                                    </svg>
                                </div>
                                <h2 class={crate::styles::TEXT_H2}>{"Account Settings"}</h2>
                            </div>

                            if !form_state.error.is_empty() {
                                <div class={format!("{} mb-8", crate::styles::CARD_ERROR)}>
                                    <div class="flex items-center">
                                        <svg class="h-5 w-5 text-red-400 dark:text-red-300 mr-2" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
                                            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
                                        </svg>
                                        <span class="text-red-800 dark:text-red-200">{&form_state.error}</span>
                                    </div>
                                </div>
                            }
                            if !form_state.success.is_empty() {
                                <div class={format!("{} mb-8", crate::styles::ALERT_SUCCESS)}>
                                    {
                                        if form_state.success.contains("temporary membership") {
                                            let expiry_date = Date::new_0();
                                            expiry_date.set_time(*expiry_time);
                                            let expiry_str = format_date_time(&expiry_date.to_iso_string().as_string().unwrap_or_default());
                                            html! {
                                                <>
                                                    {&form_state.success}
                                                    <p class="mt-1">{"Expires at: "}{expiry_str}</p>
                                                </>
                                            }
                                        } else {
                                            html! { {&form_state.success} }
                                        }
                                    }
                                </div>
                            }

                            <div class="space-y-8">
                                if *is_loading || *is_activating {
                                    // Show a simple centered spinner for both loading and activation
                                    <div class="flex justify-center items-center py-16">
                                        <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500" />
                                    </div>
                                } else {
                                    <div style="animation: fadeIn 0.5s ease-in-out;">
                                        // Show membership status if user is a member or just purchased
                                        if membership.is_member || (*just_purchased && *remaining_time > 0) {
                                            <div class="p-4 bg-green-50 dark:bg-green-900/20 rounded-lg border border-green-200 dark:border-green-800">
                                                <div class="flex items-center">
                                                    <svg class="h-5 w-5 text-green-500 mr-2" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
                                                        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                                                    </svg>
                                                    <span class="text-green-800 dark:text-green-200 font-medium">{"Thank you for being a member!"}</span>
                                                </div>
                                                <div class="mt-2 text-sm text-green-700 dark:text-green-300">
                                                    <p>{"Your membership is active."}</p>
                                                    
                                                    // Always show the countdown timer
                                                    <p class="mt-1">
                                                        <span class="font-medium">{"Time remaining: "}</span>
                                                        <span class="font-mono bg-green-100 dark:bg-green-800/50 px-2 py-1 rounded">
                                                            {format_duration(*remaining_time)}
                                                        </span>
                                                    </p>
                                                    
                                                    // Show expiry time based on the stored timestamp
                                                    <p class="mt-1">
                                                        <span class="font-medium">{"Expires at: "}</span>
                                                        {{
                                                            // Prioritize using the raw member_until string if available
                                                            if let Some(until_str) = &membership.member_until {
                                                                format_date_time(until_str)
                                                            } else {
                                                                // Fallback to using the expiry_time state if member_until is None
                                                                let expiry_date = Date::new_0();
                                                                expiry_date.set_time(*expiry_time);
                                                                format_date_time(&expiry_date.to_iso_string().as_string().unwrap_or_default())
                                                            }
                                                        }}
                                                    </p>
                                                </div>
                                            </div>
                                        }
                                        
                                        // Only show membership code input if not a member
                                        if !membership.is_member && (!*just_purchased || *remaining_time <= 0) {
                                            <MembershipCode
                                                on_error={form_state.handle_error.clone()}
                                                on_success={
                                                    let handle_success = form_state.handle_success.clone();
                                                    let just_purchased = just_purchased.clone();
                                                    let is_activating = is_activating.clone();
                                                    
                                                    Callback::from(move |message: String| {
                                                        // Set activating state to show spinner
                                                        is_activating.set(true);
                                                        
                                                        // Set the code_just_redeemed flag to trigger a refresh on next load
                                                        if let Some(window) = window() {
                                                            if let Ok(Some(storage)) = window.local_storage() {
                                                                let _ = storage.set_item("code_just_redeemed", "true");
                                                            }
                                                        }
                                                        
                                                        // Use a delay to show the activation spinner
                                                        let just_purchased_clone = just_purchased.clone();
                                                        let handle_success_clone = handle_success.clone();
                                                        let is_activating_clone = is_activating.clone();
                                                        let message_clone = message.clone();
                                                        
                                                        // Use a delay to ensure the activation spinner is visible
                                                        let timeout = Timeout::new(800, move || {
                                                            // Update UI state
                                                            just_purchased_clone.set(true);
                                                            handle_success_clone.emit(message_clone);
                                                            is_activating_clone.set(false);
                                                        });
                                                        
                                                        // Keep the timeout alive
                                                        std::mem::forget(timeout);
                                                    })
                                                }
                                            />
                                        }
                                        
                                        // Only show temporary membership if not a member
                                        if !membership.is_member && (!*just_purchased || *remaining_time <= 0) {
                                            <TemporaryMembership
                                                on_error={form_state.handle_error.clone()}
                                                on_success={
                                                    let handle_success = form_state.handle_success.clone();
                                                    let just_purchased = just_purchased.clone();
                                                    let is_activating = is_activating.clone();
                                                    
                                                    Callback::from(move |message: String| {
                                                        // Set activating state to show spinner
                                                        is_activating.set(true);
                                                        
                                                        // Set the code_just_redeemed flag to trigger a refresh on next load
                                                        if let Some(window) = window() {
                                                            if let Ok(Some(storage)) = window.local_storage() {
                                                                let _ = storage.set_item("code_just_redeemed", "true");
                                                            }
                                                        }
                                                        
                                                        // Use a delay to show the activation spinner
                                                        let just_purchased_clone = just_purchased.clone();
                                                        let handle_success_clone = handle_success.clone();
                                                        let is_activating_clone = is_activating.clone();
                                                        let message_clone = message.clone();
                                                        
                                                        // Use a delay to ensure the activation spinner is visible
                                                        let timeout = Timeout::new(800, move || {
                                                            // Update UI state
                                                            just_purchased_clone.set(true);
                                                            handle_success_clone.emit(message_clone);
                                                            is_activating_clone.set(false);
                                                        });
                                                        
                                                        // Keep the timeout alive
                                                        std::mem::forget(timeout);
                                                    })
                                                }
                                            />
                                        }
                                    </div>
                                }
                                
                                <PatreonLink />
                                
                                <AccountManagement
                                    on_success={Some(Callback::from(move |_| {
                                        if let Some(window) = window() {
                                            for storage in [window.local_storage().unwrap().unwrap(), window.session_storage().unwrap().unwrap()].iter() {
                                                storage.remove_item("token").ok();
                                                storage.remove_item("username").ok();
                                                storage.remove_item("csrf_token").ok();
                                                storage.remove_item("currency").ok();
                                                storage.remove_item("user_id").ok();
                                            }
                                            window.location().set_href("/login").ok();
                                        }
                                    }))}
                                    on_error={form_state.handle_error.clone()}
                                />
                            </div>
                        </div>
                    </div>
                </div>
            </crate::components::GradientBackground>
        </crate::base::Base>
    }
}
mod wheel_canvas;
mod wheel_utils;

use yew::prelude::*;
use gloo::net::http::Request;
use wasm_bindgen_futures::spawn_local;
use shared::shared_wheel_game::*;
use web_sys::{window, CustomEvent, CustomEventInit};
use wasm_bindgen::JsValue;
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use crate::hooks::use_currency::use_currency;
use crate::config::get_api_base_url;
use crate::hooks::use_membership::use_membership;
use crate::components::MembershipRequired;
use crate::styles;
use serde::Deserialize;
use gloo_timers::future::TimeoutFuture;
use gloo_timers::callback::Interval;
use serde_json;

// Define the response structure for wheel status
#[derive(Deserialize, Debug)]
struct WheelStatusResponse {
    cooldown_seconds: i64,
}

// Add custom CSS for animations
const CUSTOM_CSS: &str = r#"
@keyframes pulse-subtle {
    0% {
        transform: scale(1);
        box-shadow: 0 0 0 0 rgba(255, 215, 0, 0.4);
    }
    70% {
        transform: scale(1.02);
        box-shadow: 0 0 0 10px rgba(255, 215, 0, 0);
    }
    100% {
        transform: scale(1);
        box-shadow: 0 0 0 0 rgba(255, 215, 0, 0);
    }
}

.animate-pulse-subtle {
    animation: pulse-subtle 2s infinite;
}
"#;

// Import components and utilities from our modules
use wheel_canvas::{WheelCanvas, ease_out_cubic};
use wheel_utils::{
    get_auth_token,
    ResultDisplay, SpinButton, format_time
};

// Add a constant for the wheel cooldown to match the backend
const WHEEL_SPIN_COOLDOWN: f64 = 82800.0; // 23 hours (was 30 seconds)

// Function to fetch wheel status from server
async fn fetch_wheel_status() -> Result<WheelStatusResponse, String> {
    let token = match get_auth_token() {
        Some(token) => token,
        None => return Err("No authentication token found".to_string()),
    };

    match Request::get(&format!("{}/wheel/cooldown", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await {
        Ok(response) => {
            if response.ok() {
                match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        // Extract the cooldown_seconds value
                        if let Some(cooldown) = data.get("cooldown_seconds").and_then(|v| v.as_i64()) {
                            Ok(WheelStatusResponse { cooldown_seconds: cooldown })
                        } else {
                            Err("Missing cooldown_seconds in response".to_string())
                        }
                    },
                    Err(e) => Err(format!("Error parsing status response: {:?}", e))
                }
            } else {
                Err(format!("Error status: {}", response.status()))
            }
        },
        Err(e) => Err(format!("Network error: {:?}", e))
    }
}

#[function_component(FrontendWheelGame)]
pub fn frontend_wheel_game() -> Html {
    // Apply custom CSS
    {
        use_effect_with((), move |_| {
            let style_element = if let Some(window) = window() {
                if let Some(document) = window.document() {
                    let head = document.head().expect("Document should have a head");
                    let style = document.create_element("style").expect("Should be able to create style element");
                    style.set_text_content(Some(CUSTOM_CSS));
                    let _ = head.append_child(&style);
                    
                    // Return the style element for cleanup
                    Some(style)
                } else {
                    None
                }
            } else {
                None
            };
            
            // Return cleanup function
            move || {
                if let Some(style) = style_element {
                    if let Some(parent) = style.parent_node() {
                        let _ = parent.remove_child(&style);
                    }
                }
            }
        });
    }

    // Game state
    let game_state = use_state(|| None::<WheelGame>);
    let is_spinning = use_state(|| false);
    let result_number = use_state(|| None::<f64>);
    let error_message = use_state(String::new);
    let rotation = use_state(|| 90.0); // Start 90 degrees clockwise
    let will_win = use_state(|| false);
    let show_result = use_state(|| false);
    
    // Get current balance using the use_currency hook
    let _current_balance = use_currency();
    
    // Loading states similar to claim button
    let loading = use_state(|| true);
    let cooldown_seconds = use_state(|| 0i32);
    let is_on_cooldown = use_state(|| false);
    let can_spin = use_state(|| false);
    let initialized = use_state(|| false);
    let server_confirmed = use_state(|| false);
    let pending_server_response = use_state(|| true);

    // Add membership check
    let membership = use_membership();

    // Fetch cooldown status on component mount
    {
        let loading = loading.clone();
        let can_spin = can_spin.clone();
        let cooldown_seconds = cooldown_seconds.clone();
        let is_on_cooldown = is_on_cooldown.clone();
        let initialized = initialized.clone();
        let server_confirmed = server_confirmed.clone();
        let pending_server_response = pending_server_response.clone();
        let error_message = error_message.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                pending_server_response.set(true);
                match fetch_wheel_status().await {
                    Ok(response) => {
                        let cooldown = response.cooldown_seconds;
                        cooldown_seconds.set(cooldown as i32);
                        is_on_cooldown.set(cooldown > 0);
                        can_spin.set(cooldown <= 0);
                        server_confirmed.set(true);
                        pending_server_response.set(false);
                        
                        // Store in local storage for persistence
                        if cooldown > 0 {
                            if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                                let cooldown_end = js_sys::Date::now() + (cooldown as f64 * 1000.0);
                                let _ = storage.set_item("wheel_cooldown_end", &cooldown_end.to_string());
                            }
                        }
                        
                        // Force a minimum loading time
                        let cooldown_seconds_clone = cooldown_seconds.clone();
                        let can_spin_clone = can_spin.clone();
                        let initialized_clone = initialized.clone();
                        let loading_clone = loading.clone();
                        
                        // Safety check: if remaining_cooldown > 0, ensure can_spin is false
                        if *cooldown_seconds_clone > 0 && *can_spin_clone {
                            can_spin_clone.set(false);
                        }
                        
                        // Set a timeout to ensure the loading state is visible for at least 800ms
                        spawn_local(async move {
                            TimeoutFuture::new(800).await;
                            initialized_clone.set(true);
                            loading_clone.set(false);
                        });
                    },
                    Err(err) => {
                        // If server call fails, try to use local storage as a fallback
                        if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                            if let Ok(Some(cooldown_end_str)) = storage.get_item("wheel_cooldown_end") {
                                if let Ok(cooldown_end) = cooldown_end_str.parse::<f64>() {
                                    let now = js_sys::Date::now();
                                    if now < cooldown_end {
                                        // Calculate remaining seconds
                                        let remaining = ((cooldown_end - now) / 1000.0).ceil() as i32;
                                        cooldown_seconds.set(remaining);
                                        is_on_cooldown.set(remaining > 0);
                                        can_spin.set(remaining <= 0);
                                    } else {
                                        // Cooldown has expired
                                        cooldown_seconds.set(0);
                                        is_on_cooldown.set(false);
                                        can_spin.set(true);
                                        let _ = storage.remove_item("wheel_cooldown_end");
                                    }
                                }
                            }
                        }
                        
                        error_message.set(format!("Failed to fetch wheel status: {}", err));
                        pending_server_response.set(false);
                        initialized.set(true);
                        loading.set(false);
                        server_confirmed.set(true); // Mark as confirmed even though it failed, as we're using local data
                    }
                }
            });
            || ()
        });
    }

    // Update cooldown timer - only run if we're initialized
    {
        let cooldown_seconds = cooldown_seconds.clone();
        let is_on_cooldown = is_on_cooldown.clone();
        let can_spin = can_spin.clone();
        let initialized = initialized.clone();
        
        // Create a ref to store the interval
        let interval_ref = std::rc::Rc::new(std::cell::RefCell::new(None));
        let interval_ref_clone = interval_ref.clone();
        
        use_effect_with((*initialized, *cooldown_seconds), move |(initialized, _)| {
            if !initialized {
                return Box::new(|| ()) as Box<dyn FnOnce()>;
            }
            
            // Only create the interval if there's a cooldown
            if *cooldown_seconds <= 0 {
                // Clear any existing interval
                if let Some(interval) = interval_ref_clone.borrow_mut().take() {
                    drop(interval);
                }
                return Box::new(|| ()) as Box<dyn FnOnce()>;
            }
            
            // Create the interval directly and store it in the ref
            let interval = Interval::new(1000, move || {
                // Decrement the remaining cooldown by 1 second each interval
                let current = *cooldown_seconds;
                if current > 1 {
                    cooldown_seconds.set(current - 1);
                    // Only update can_spin when we reach zero
                    if current == 1 {
                        can_spin.set(true);
                        is_on_cooldown.set(false);
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

    let start_spin = {
        let game_state = game_state.clone();
        let is_spinning = is_spinning.clone();
        let result_number = result_number.clone();
        let error_message = error_message.clone();
        let rotation = rotation.clone();
        let will_win = will_win.clone();
        let show_result = show_result.clone();
        let cooldown_seconds = cooldown_seconds.clone();
        let is_on_cooldown = is_on_cooldown.clone();
        let loading = loading.clone();
        let can_spin = can_spin.clone();
        let initialized = initialized.clone();
        let pending_server_response = pending_server_response.clone();

        Callback::from(move |_| {
            // Double-check that we're initialized and can spin
            if !*initialized || !*can_spin || *loading || *pending_server_response || *is_spinning || *is_on_cooldown {
                return;
            }
            
            // Extra safety check - make sure there's no remaining cooldown
            if *cooldown_seconds > 0 {
                web_sys::console::warn_1(&format!("Prevented spin attempt with {} seconds cooldown remaining", *cooldown_seconds).into());
                return;
            }

            loading.set(true);
            // Set can_spin to false immediately to prevent double-clicks
            can_spin.set(false);
            error_message.set(String::new());

            let game_state = game_state.clone();
            let is_spinning = is_spinning.clone();
            let result_number = result_number.clone();
            let error_message = error_message.clone();
            let rotation = rotation.clone();
            let will_win = will_win.clone();
            let show_result = show_result.clone();
            let cooldown_seconds = cooldown_seconds.clone();
            let is_on_cooldown = is_on_cooldown.clone();
            let loading = loading.clone();

            spawn_local(async move {
                is_spinning.set(true);
                error_message.set(String::new());
                result_number.set(None);
                show_result.set(false);

                let token = get_auth_token();
                
                if token.is_none() {
                    loading.set(false);
                    error_message.set("Please log in again".to_string());
                    return;
                }

                let timestamp = js_sys::Date::now() as u64;
                
                let spin_req = WheelSpinRequest {
                    timestamp,
                };

                // Set cooldown immediately to prevent multiple spins
                cooldown_seconds.set(WHEEL_SPIN_COOLDOWN as i32);
                is_on_cooldown.set(true);

                // Store cooldown end time in local storage
                if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                    let cooldown_end = js_sys::Date::now() + (WHEEL_SPIN_COOLDOWN * 1000.0);
                    let _ = storage.set_item("wheel_cooldown_end", &cooldown_end.to_string());
                }

                match Request::post(&format!("{}/wheel/spin", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                    .json(&spin_req)
                    .expect("Failed to build request")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if resp.status() == 200 {
                            if let Ok(spin_resp) = resp.json::<WheelSpinResponse>().await {
                                // Extract values we need from spin_resp
                                let success = spin_resp.success;
                                let is_win = spin_resp.is_win;
                                let new_balance = spin_resp.new_balance;
                                let message_opt = spin_resp.message;
                                let backend_number_opt = spin_resp.result_number;
                                
                                // Check if the spin was successful
                                if !success {
                                    // If not successful, show the error message and stop spinning
                                    is_spinning.set(false);
                                    loading.set(false);
                                    if let Some(msg) = message_opt {
                                        // Check if this is a cooldown message
                                        if msg.contains("Please wait") {
                                            // Extract the seconds from the message
                                            if let Some(seconds_str) = msg.split_whitespace()
                                                .nth(2)
                                                .and_then(|s| s.parse::<i32>().ok()) 
                                            {
                                                cooldown_seconds.set(seconds_str);
                                                is_on_cooldown.set(true);
                                                
                                                // Store cooldown end time in local storage
                                                if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                                                    let cooldown_end = js_sys::Date::now() + (seconds_str as f64 * 1000.0);
                                                    let _ = storage.set_item("wheel_cooldown_end", &cooldown_end.to_string());
                                                }
                                                
                                                error_message.set(String::new()); // Clear error message since we show on button
                                            } else {
                                                error_message.set(msg);
                                            }
                                        } else {
                                            error_message.set(msg);
                                            // If it's not a cooldown error, reset the cooldown we set earlier
                                            cooldown_seconds.set(0);
                                            is_on_cooldown.set(false);
                                        }
                                        show_result.set(true);
                                    } else {
                                        error_message.set("An error occurred while spinning the wheel.".to_string());
                                        show_result.set(true);
                                        // If it's not a cooldown error, reset the cooldown we set earlier
                                        cooldown_seconds.set(0);
                                        is_on_cooldown.set(false);
                                    }
                                    return;
                                }
                                
                                // Process the result
                                if let Some(backend_number) = backend_number_opt {
                                    result_number.set(Some(backend_number));
                                    
                                    // Determine the outcome based on the result number
                                    let is_scroll_win = backend_number >= 60.0 && backend_number < 85.0;
                                    let is_big_pax_win = backend_number >= 85.0;
                                    let is_small_pax_win = backend_number >= 35.0 && backend_number < 60.0;
                                    let _is_tiny_pax_win = backend_number < 35.0;
                                    will_win.set(is_win);
                                    
                                    // Calculate final rotation to ensure the wheel lands on the correct segment
                                    // Start with current rotation
                                    let current_rotation = *rotation;
                                    
                                    // Add minimum spins (at least 3 full rotations for effect)
                                    let min_spins = 8.0 * 360.0; // Increased from 5.0 to 8.0 rotations for faster initial spin
                                    
                                    // Determine the target position based on the outcome
                                    let target_position = if is_scroll_win {
                                        // For orange segment (0-90°)
                                        let segment_pos = 15.0 + (backend_number - 60.0) * (60.0 / 25.0);
                                        (270.0 - segment_pos) % 360.0 + 360.0 * min_spins
                                    } else if is_big_pax_win {
                                        // For blue segment (90-144°)
                                        let segment_pos = 100.0 + (backend_number - 85.0) * (34.0 / 15.0);
                                        (270.0 - segment_pos) % 360.0 + 360.0 * min_spins
                                    } else if is_small_pax_win {
                                        // For violet segment (144-234°)
                                        let segment_pos = 154.0 + (backend_number - 35.0) * (70.0 / 25.0);
                                        (270.0 - segment_pos) % 360.0 + 360.0 * min_spins
                                    } else {
                                        // For pink segment (234-360°)
                                        let segment_pos = 244.0 + (backend_number) * (106.0 / 35.0);
                                        (270.0 - segment_pos) % 360.0 + 360.0 * min_spins
                                    };
                                    
                                    // Calculate final rotation (current + min_spins + adjustment to land on target)
                                    let normalized_current = current_rotation % 360.0;
                                    let adjustment = (target_position - normalized_current + 360.0) % 360.0;
                                    let final_rotation = current_rotation + min_spins + adjustment;
                                    
                                    // Animate the wheel spinning
                                    let start_time = js_sys::Date::now();
                                    let duration = 6000.0; // 6 seconds spin
                                    let start_rotation = current_rotation;
                                    let rotation_change = final_rotation - start_rotation;
                                    
                                    // Create a reference to the animation frame callback
                                    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
                                    let g = f.clone();
                                    
                                    // Clone message_opt to avoid FnOnce issue
                                    let message_opt_clone = message_opt.clone();
                                    
                                    // Define the animation function using the extracted values
                                    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                                        let current_time = js_sys::Date::now();
                                        let elapsed = current_time - start_time;
                                        let progress = (elapsed / duration).min(1.0);
                                        
                                        // Easing function for smooth deceleration
                                        let eased_progress = ease_out_cubic(progress);
                                        let current_rotation = start_rotation + rotation_change * eased_progress;
                                        rotation.set(current_rotation);
                                        
                                        if elapsed < duration {
                                            // Request next frame
                                            if let Some(window) = web_sys::window() {
                                                let _ = window.request_animation_frame(
                                                    f.borrow().as_ref().unwrap().as_ref().unchecked_ref()
                                                );
                                            }
                                        } else {
                                            // Animation complete
                                            rotation.set(final_rotation);
                                            
                                            // Update game state with result
                                            is_spinning.set(false);
                                            loading.set(false);
                                            
                                            // Update currency balance
                                            if let Some(window) = window() {
                                                let event_init = CustomEventInit::new();
                                                event_init.set_detail(&JsValue::from_f64(new_balance as f64));
                                                if let Ok(event) = CustomEvent::new_with_event_init_dict(
                                                    "currencyUpdate",
                                                    &event_init,
                                                ) {
                                                    let _ = window.dispatch_event(&event);
                                                }
                                            }
                                            
                                            // Update game state with result
                                            game_state.set(Some(WheelGame {
                                                is_spinning: false,
                                                last_result: Some(WheelResult {
                                                    is_win,
                                                    reward_type: if is_scroll_win {
                                                        Some(RewardType::Scroll)
                                                    } else if is_big_pax_win {
                                                        Some(RewardType::BigPax)
                                                    } else if is_small_pax_win {
                                                        Some(RewardType::SmallPax)
                                                    } else {
                                                        Some(RewardType::TinyPax)
                                                    },
                                                    new_balance,
                                                }),
                                                cost_to_spin: 0, // Updated to 0 since it's free
                                            }));
                                            
                                            // Use the cloned message_opt to avoid FnOnce issue
                                            if let Some(_msg) = &message_opt_clone {
                                                // Don't display any of the detailed messages from the backend
                                                error_message.set(String::new());
                                            }
                                            
                                            show_result.set(true);
                                        }
                                    }) as Box<dyn FnMut()>));
                                    
                                    // Start the animation
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.request_animation_frame(
                                            g.borrow().as_ref().unwrap().as_ref().unchecked_ref()
                                        );
                                    }
                                }
                            }
                        } else {
                            is_spinning.set(false);
                            loading.set(false);
                            error_message.set("Failed to process spin".to_string());
                        }
                    }
                    Err(_) => {
                        is_spinning.set(false);
                        loading.set(false);
                        error_message.set("Network error".to_string());
                    }
                }
            });
        })
    };

    html! {
        <div class="container mx-auto px-4 py-8">
            <h1 class="text-3xl font-bold mb-6 text-center text-gray-900 dark:text-white">
                <span class="bg-clip-text text-transparent bg-gradient-to-r from-yellow-400 to-orange-500">{"Daily Wheel"}</span>
            </h1>
            
            if membership.loading {
                <div class="flex justify-center">
                    <div class={styles::LOADING_SPINNER}></div>
                </div>
            } else {
                <div class="bg-white dark:bg-gray-800 p-6 sm:p-8 rounded-2xl shadow-xl dark:shadow-[0_8px_30px_-12px_rgba(255,255,255,0.1)] max-w-2xl mx-auto border border-gray-100 dark:border-gray-700 backdrop-blur-sm">
                    <div class="relative mx-auto mb-8 flex justify-center items-center">
                        <div class="w-full max-w-[450px] mx-auto">
                            <WheelCanvas rotation={*rotation} is_spinning={*is_spinning} will_win={*will_win} />
                        </div>
                    </div>

                    if !(*error_message).is_empty() && membership.is_member {
                        <div class="mb-6 text-center">
                            if (*error_message).contains("Congratulations") {
                                <p class="text-green-500 bg-green-50 dark:bg-green-900/20 p-3 rounded-lg">{&*error_message}</p>
                            } else {
                                <p class="text-red-500 bg-red-50 dark:bg-red-900/20 p-3 rounded-lg">{&*error_message}</p>
                            }
                        </div>
                    }

                    if !membership.is_member {
                        <div class="mt-6">
                            <MembershipRequired feature_name="Wheel Game" />
                        </div>
                    } else {
                        <div class="flex justify-center mt-4">
                            {
                                if *loading {
                                    html! {
                                        <div class="w-full max-w-[300px]">
                                            <div class="w-full flex items-center justify-center py-4 px-8 rounded-full bg-gray-300 dark:bg-gray-700 animate-pulse mb-2">
                                                <svg class="animate-spin mr-3 h-5 w-5 text-gray-500 dark:text-gray-400" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                </svg>
                                                <span class="text-gray-700 dark:text-gray-300 font-medium">{"Loading..."}</span>
                                            </div>
                                        </div>
                                    }
                                } else if *is_on_cooldown && *cooldown_seconds > 0 {
                                    html! {
                                        <div class="w-full max-w-[300px]">
                                            <div class="mb-2 flex justify-between items-center">
                                                <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{"Next spin available in:"}</span>
                                                <span class="text-sm font-bold text-blue-600 dark:text-blue-400">{format_time(*cooldown_seconds)}</span>
                                            </div>
                                            <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2.5 mb-4">
                                                <div class="bg-gradient-to-r from-blue-500 to-purple-600 h-2.5 rounded-full transition-all duration-500" 
                                                    style={format!("width: {}%", (1.0 - (*cooldown_seconds as f32 / WHEEL_SPIN_COOLDOWN as f32)) * 100.0)}>
                                                </div>
                                            </div>
                                            <SpinButton 
                                                is_spinning={*is_spinning}
                                                is_on_cooldown={*is_on_cooldown}
                                                cooldown_seconds={*cooldown_seconds}
                                                has_enough_balance={true}
                                                onclick={start_spin}
                                            />
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <div class="w-full max-w-[300px]">
                                            <SpinButton 
                                                is_spinning={*is_spinning}
                                                is_on_cooldown={*is_on_cooldown}
                                                cooldown_seconds={*cooldown_seconds}
                                                has_enough_balance={true}
                                                onclick={start_spin}
                                            />
                                        </div>
                                    }
                                }
                            }
                        </div>
                        
                        <ResultDisplay 
                            reward_type={
                                if let Some(game) = &*game_state {
                                    if let Some(result) = &game.last_result {
                                        result.reward_type.clone()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            show_result={*show_result}
                            result_number={*result_number}
                        />
                        
                        // Add game instructions
                        <div class="mt-8 text-center bg-gray-50 dark:bg-gray-700/30 p-6 rounded-xl shadow-sm">
                            <h3 class="font-bold text-lg mb-3 text-gray-800 dark:text-gray-200 flex items-center justify-center">
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 mr-2" viewBox="0 0 20 20" fill="currentColor">
                                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                                </svg>
                                {"How to Play"}
                            </h3>
                            <p class="mb-4 text-gray-700 dark:text-gray-300">{"Spin the magical wheel for a chance to win pax or a scroll! Spinning is completely free."}</p>
                            
                            <div class="grid grid-cols-1 md:grid-cols-2 gap-3 mt-4">
                                <div class="bg-white dark:bg-gray-800 p-3 rounded-lg shadow-sm flex items-center border-l-4 border-orange-500">
                                    <div class="w-4 h-4 rounded-full bg-orange-500 mr-3 flex-shrink-0"></div>
                                    <div class="text-left">
                                        <div class="font-medium text-gray-900 dark:text-white">{"Scroll"}</div>
                                        <div class="text-xs text-gray-500 dark:text-gray-400">{"25% chance"}</div>
                                    </div>
                                </div>
                                <div class="bg-white dark:bg-gray-800 p-3 rounded-lg shadow-sm flex items-center border-l-4 border-cyan-500">
                                    <div class="w-4 h-4 rounded-full bg-cyan-500 mr-3 flex-shrink-0"></div>
                                    <div class="text-left">
                                        <div class="font-medium text-gray-900 dark:text-white">{"50 pax"}</div>
                                        <div class="text-xs text-gray-500 dark:text-gray-400">{"15% chance"}</div>
                                    </div>
                                </div>
                                <div class="bg-white dark:bg-gray-800 p-3 rounded-lg shadow-sm flex items-center border-l-4 border-violet-500">
                                    <div class="w-4 h-4 rounded-full bg-violet-500 mr-3 flex-shrink-0"></div>
                                    <div class="text-left">
                                        <div class="font-medium text-gray-900 dark:text-white">{"20 pax"}</div>
                                        <div class="text-xs text-gray-500 dark:text-gray-400">{"25% chance"}</div>
                                    </div>
                                </div>
                                <div class="bg-white dark:bg-gray-800 p-3 rounded-lg shadow-sm flex items-center border-l-4 border-pink-500">
                                    <div class="w-4 h-4 rounded-full bg-pink-500 mr-3 flex-shrink-0"></div>
                                    <div class="text-left">
                                        <div class="font-medium text-gray-900 dark:text-white">{"10 pax"}</div>
                                        <div class="text-xs text-gray-500 dark:text-gray-400">{"35% chance"}</div>
                                    </div>
                                </div>
                            </div>
                            
                            <div class="mt-4 text-xs text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 p-2 rounded-md inline-block">
                                {"23-hour cooldown between spins"}
                            </div>
                        </div>
                    }
                </div>
            }
        </div>
    }
} 
use yew::prelude::*;
use web_sys::{window, MouseEvent, CustomEvent, CustomEventInit};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::callback::Interval;
use uuid::Uuid;
use js_sys::Date;
use wasm_bindgen::{JsValue, JsCast};
use wasm_bindgen::closure::Closure;
use crate::models::Creature;
use crate::pages::inventory::handlers::CreatureResponse;
use serde::Deserialize;
use serde_json;
use gloo_timers::future::sleep;
use gloo_utils::format::JsValueSerdeExt;
use crate::config::get_api_base_url;
use std::rc::Rc;

const CURRENCY_UPDATE_EVENT: &str = "currencyUpdate";
const ENERGY_RECHARGE_EVENT: &str = "energyRecharge";

fn dispatch_currency_event(amount: i64) {
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

// Helper function to dispatch energy recharge event
fn dispatch_energy_recharge_event(creature_id: &Uuid, recharging: bool) {
    if let Some(window) = window() {
        let event_init = CustomEventInit::new();
        let detail_json = serde_json::json!({
            "creatureId": creature_id.to_string(),
            "recharging": recharging
        });
        let detail = match JsValue::from_serde(&detail_json) {
            Ok(val) => val,
            Err(e) => {
                log::error!("Failed to serialize event detail: {:?}", e);
                JsValue::NULL
            }
        };
        
        event_init.set_detail(&detail);
        let event = CustomEvent::new_with_event_init_dict(
            ENERGY_RECHARGE_EVENT,
            &event_init
        ).unwrap();
        window.dispatch_event(&event).unwrap();
    }
}

#[derive(Debug, Deserialize)]
struct EnergyResponse {
    success: bool,
    error: Option<String>,
    creature_id: Uuid,
    energy_full: bool,
    pax_balance: i64,
    energy_recharge_complete_at: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct EnergyManagerProps {
    pub creature: Creature,
    pub updated_creature: Option<CreatureResponse>,
    pub fetch_data: Option<Callback<()>>,
    #[prop_or_default]
    pub on_energy_update: Option<Callback<(Uuid, bool)>>,
    #[prop_or_default]
    pub on_recharge_start: Callback<Uuid>,
}

// Helper function to calculate energy recharge cost based on rarity
fn get_energy_recharge_cost(rarity: Option<&str>) -> i32 {
    match rarity {
        Some("Rare") => 10,
        Some("Epic") => 20,
        Some("Legendary") => 30,
        Some("Mythical") => 40,
        _ => 5 // Default for Uncommon and others
    }
}

#[function_component(EnergyManager)]
pub fn energy_manager(props: &EnergyManagerProps) -> Html {
    let error = use_state(|| String::new());
    let loading = use_state(|| false);
    let max_seconds = use_state(|| 21600); // Energy recharge takes 6 hours (21600 seconds) now (was 60)
    let chaos_remaining_seconds = use_state(|| None::<i32>);
    // Add state to track when recharge is being initiated (before API response)
    let is_initiating_recharge = use_state(|| false);
    let charge_locked = use_state(|| false);
    // Add a direct override for energy display to always show zero
    let force_zero_energy = use_state(|| false);
    // Add state to track the effective energy full state that combines props and local state
    let effective_energy_full = use_state(|| props.creature.energy_full);

    // Initialize recharge time and remaining seconds
    let recharge_time = props.updated_creature.as_ref()
        .and_then(|c| c.energy_recharge_complete_at.clone())
        .or_else(|| props.creature.energy_recharge_complete_at.clone());

    let initial_remaining_seconds = if let Some(recharge_time) = &recharge_time {
        let finish_date = Date::new(&JsValue::from_str(recharge_time));
        let now = Date::new_0();
        ((finish_date.get_time() - now.get_time()) / 1000.0).ceil() as i32
    } else {
        0
    };

    let remaining_seconds = use_state(|| initial_remaining_seconds.max(0));
    let pax_balance = use_state(|| 0);
    let local_recharge_time = use_state(|| recharge_time);

    // Calculate energy cost based on creature rarity
    let energy_cost = get_energy_recharge_cost(props.creature.rarity.as_deref());
    
    // Add a state to specifically track chaos realm status based on props
    let is_in_chaos_realm_prop = use_state(|| props.creature.in_chaos_realm);
    
    // Effect: Update internal chaos realm state when props change
    {
        let is_in_chaos_realm_prop = is_in_chaos_realm_prop.clone();
        let chaos_remaining_seconds = chaos_remaining_seconds.clone();
        use_effect_with(props.creature.in_chaos_realm, move |in_chaos_realm| {
            is_in_chaos_realm_prop.set(*in_chaos_realm);
            
            // If entering chaos realm, immediately set a default value for remaining time
            // This ensures the purple styling appears without waiting for the next check
            if *in_chaos_realm {
                chaos_remaining_seconds.set(Some(82800)); // Set initial 23 hours (in seconds)
            }
            
            || ()
        });
    }
    
    // Effect: Set initial state on component mount
    {
        let effective_energy_full = effective_energy_full.clone();
        let force_zero_energy = force_zero_energy.clone();
        let charge_locked = charge_locked.clone();
        let is_initiating_recharge = is_initiating_recharge.clone();
        let initial_energy_full = props.creature.energy_full; // Clone this value
        
        use_effect_with((), move |_| {
            // On component mount, ensure we start with the correct energy state
            effective_energy_full.set(initial_energy_full);
            
            // Reset forced states on component unmount
            move || {
                force_zero_energy.set(false);
                charge_locked.set(false);
                is_initiating_recharge.set(false);
            }
        });
    }
    
    // Effect: Update effective_energy_full when props change
    {
        let effective_energy_full = effective_energy_full.clone();
        let is_initiating_recharge = is_initiating_recharge.clone();
        let charge_locked = charge_locked.clone();
        
        use_effect_with((props.creature.clone(), props.updated_creature.clone()), move |(creature, updated_creature)| {
            // Only update if not currently initiating a recharge and not locked
            if !*is_initiating_recharge && !*charge_locked {
                let energy_full = updated_creature.as_ref()
                    .map(|c| c.energy_full)
                    .unwrap_or(creature.energy_full);
                effective_energy_full.set(energy_full);
            }
            || ()
        });
    }

    // Effect: Update local_recharge_time when updated_creature changes
    {
        let local_recharge_time = local_recharge_time.clone();
        let remaining_seconds = remaining_seconds.clone();
        let effective_energy_full = effective_energy_full.clone();
        let force_zero_energy = force_zero_energy.clone();
        
        use_effect_with(props.updated_creature.clone(), move |updated_creature| {
            if let Some(creature) = updated_creature {
                if let Some(recharge_time) = &creature.energy_recharge_complete_at {
                    let finish_date = Date::new(&JsValue::from_str(recharge_time));
                    let now = Date::new_0();
                    let remaining = ((finish_date.get_time() - now.get_time()) / 1000.0).ceil() as i32;
                    remaining_seconds.set(remaining.max(0));
                    local_recharge_time.set(Some(recharge_time.clone()));
                    
                    // Only update effective_energy_full if force_zero_energy is false
                    // This prevents API updates from causing flickering
                    if !*force_zero_energy {
                        effective_energy_full.set(creature.energy_full);
                    }
                    
                    // If we get an updated creature with recharge time, keep force_zero_energy true
                    // to prevent any flashing to 100%
                    force_zero_energy.set(remaining > 0);
                } else {
                    remaining_seconds.set(0);
                    local_recharge_time.set(None);
                    
                    // Only update effective_energy_full if we're not in a forced state
                    if !*force_zero_energy {
                        effective_energy_full.set(creature.energy_full);
                    }
                    
                    // If no recharge time and creature is full energy, we can reset force_zero_energy
                    if creature.energy_full {
                        force_zero_energy.set(false);
                    }
                }
            }
            || ()
        });
    }

    // Effect: Calculate remaining seconds when local_recharge_time changes
    {
        let remaining_seconds = remaining_seconds.clone();
        let recharge_time = (*local_recharge_time).clone();
        use_effect_with(recharge_time, move |recharge_time_option| {
            if let Some(recharge_time) = recharge_time_option.as_ref() {
                let finish_date = Date::new(&JsValue::from_str(recharge_time));
                let now = Date::new_0();
                let remaining = ((finish_date.get_time() - now.get_time()) / 1000.0).ceil() as i32;
                remaining_seconds.set(remaining.max(0));

                // Set up interval to update remaining time
                let interval = {
                    let remaining_seconds = remaining_seconds.clone();
                    Interval::new(1000, move || {
                        let now = Date::new_0();
                        let remaining = ((finish_date.get_time() - now.get_time()) / 1000.0).ceil() as i32;
                        remaining_seconds.set(remaining.max(0));
                    })
                };

                Box::new(move || drop(interval)) as Box<dyn FnOnce()>
            } else {
                remaining_seconds.set(0);
                Box::new(|| {}) as Box<dyn FnOnce()>
            }
        });
    }

    // Effect: Check if energy_recharge_complete_at is in the past when component loads
    {
        let creature = props.creature.clone();
        let fetch_data = props.fetch_data.clone();
        let on_energy_update = props.on_energy_update.clone();
        let remaining_seconds = remaining_seconds.clone();
        
        use_effect_with((), move |_| {
            // If creature has a recharge time but energy is not full
            if !creature.energy_full && creature.energy_recharge_complete_at.is_some() {
                let recharge_time = creature.energy_recharge_complete_at.as_ref().unwrap();
                let finish_date = Date::new(&JsValue::from_str(recharge_time));
                let now = Date::new_0();
                let remaining = ((finish_date.get_time() - now.get_time()) / 1000.0).ceil() as i32;
                
                // If the recharge time is in the past (remaining <= 0)
                if remaining <= 0 {
                    log::info!("Energy recharge time is in the past but energy_full is false. Updating UI to show energy as full.");
                    
                    // Trigger a refresh if callback exists
                    if let Some(callback) = fetch_data.as_ref() {
                        callback.emit(());
                    }
                    
                    // Notify parent that energy is now full
                    if let Some(callback) = on_energy_update.as_ref() {
                        callback.emit((creature.id, true));
                    }
                    
                    // Update remaining seconds to 0
                    remaining_seconds.set(0);
                }
            }
            || ()
        });
    }

    // Effect: Update chaos realm remaining time
    {
        let chaos_remaining_seconds = chaos_remaining_seconds.clone();
        use_effect_with(props.creature.clone(), move |creature| {
            let mut interval = None;
            
            if creature.in_chaos_realm {
                if let Some(entry_time) = &creature.chaos_realm_entry_at {
                    let entry_date = Date::new(&JsValue::from_str(entry_time));
                    let total_seconds = 82800; // Chaos realm duration is 23 hours (82800 seconds)
                    
                    let update_remaining = {
                        let chaos_remaining_seconds = chaos_remaining_seconds.clone();
                        move || {
                            let now = Date::new_0();
                            let elapsed_ms = now.get_time() - entry_date.get_time();
                            let elapsed_seconds = (elapsed_ms / 1000.0).ceil() as i32;
                            let remaining = (total_seconds - elapsed_seconds).max(0);
                            chaos_remaining_seconds.set(Some(remaining));
                        }
                    };
                    
                    // Initial update
                    update_remaining();
                    
                    // Set up interval for updates
                    interval = Some(Interval::new(100, update_remaining));
                }
            }
            
            move || {
                if let Some(i) = interval {
                    i.cancel();
                }
            }
        });
    }

    // Calculate progress percentage
    let progress_percent = if *is_initiating_recharge {
        0  // If initiating recharge, always show 0%
    } else if *effective_energy_full && !*is_initiating_recharge {
        100
    } else if *is_in_chaos_realm_prop {
        if let Some(entry_time) = &props.creature.chaos_realm_entry_at {
            let entry_date = Date::new(&JsValue::from_str(entry_time));
            let now = Date::new_0();
            let elapsed_ms = now.get_time() - entry_date.get_time();
            let elapsed_seconds = (elapsed_ms / 1000.0).ceil() as i32;
            let total_seconds = 82800; // Chaos realm duration is 23 hours (82800 seconds)
            let remaining_seconds = total_seconds - elapsed_seconds;
            ((remaining_seconds as f64 / total_seconds as f64) * 100.0).max(0.0).min(100.0) as i32
        } else {
            100
        }
    } else if let Some(_) = &*local_recharge_time {
        let total_seconds = *max_seconds;
        let remaining = *remaining_seconds;
        ((total_seconds - remaining) as f64 / total_seconds as f64 * 100.0) as i32
    } else {
        0
    };

    // Effect: When remaining seconds reaches 0, trigger a refresh and clear local_recharge_time after a delay
    {
        let remaining_seconds = remaining_seconds.clone();
        let local_recharge_time = local_recharge_time.clone();
        let fetch_data = props.fetch_data.clone();
        let on_energy_update = props.on_energy_update.clone();
        let creature_id = props.creature.id;
        let effective_energy_full = effective_energy_full.clone();
        let force_zero_energy = force_zero_energy.clone();
        let _is_initiating_recharge = is_initiating_recharge.clone();
        let _charge_locked = charge_locked.clone();
        
        use_effect_with(*remaining_seconds, move |&seconds| {
            if seconds <= 0 && local_recharge_time.is_some() {
                // Recharge time is up, trigger a refresh if callback exists
                if let Some(callback) = fetch_data.as_ref() {
                    callback.emit(());
                }
                
                // Notify parent that energy is now full
                if let Some(callback) = on_energy_update.as_ref() {
                    callback.emit((creature_id, true));
                }
                
                // Update effective energy full state
                effective_energy_full.set(true);
                
                // Reset forced zero energy state after recharge completes
                force_zero_energy.set(false);
                
                // Delay clearing the local recharge time by 500ms to prevent a flash
                let local_recharge_time_clone = local_recharge_time.clone();
                spawn_local(async move {
                    sleep(std::time::Duration::from_millis(500)).await;
                    local_recharge_time_clone.set(None);
                });
            }
            || ()
        });
    }

    let recharge_energy = {
        // Clone all the necessary values from props before creating the callback
        let creature_id = props.creature.id;
        let creature_in_chaos_realm = props.creature.in_chaos_realm;
        let on_recharge_start = props.on_recharge_start.clone();
        let fetch_data = props.fetch_data.clone();
        let on_energy_update = props.on_energy_update.clone();
        let api_base_url = get_api_base_url();
        
        // Clone state variables
        let error = error.clone();
        let loading = loading.clone();
        let pax_balance = pax_balance.clone();
        let local_recharge_time = local_recharge_time.clone();
        let is_initiating_recharge = is_initiating_recharge.clone();
        let effective_energy_full = effective_energy_full.clone();
        // Use Rc to share the charge_locked state
        let charge_locked_rc = Rc::new(charge_locked.clone());
        let force_zero_energy = force_zero_energy.clone();

        Callback::from(move |e: MouseEvent| {
            // Clone api_base_url here to avoid moving it
            let api_base_url = api_base_url.clone();
            // Clone charge_locked reference for this closure
            let charge_locked = charge_locked_rc.clone();
            
            // Prevent event propagation to ensure nothing else triggers
            e.stop_propagation();
            
            if *loading || *effective_energy_full || creature_in_chaos_realm || local_recharge_time.is_some() {
                return;
            }
            
            // *** Emit recharge start signal IMMEDIATELY ***
            on_recharge_start.emit(creature_id);
            
            // Set a flag to prepare for the animation before forcing zero energy
            is_initiating_recharge.set(true);
            
            // Use requestAnimationFrame to ensure the DOM has updated before changing other states
            if let Some(window) = window() {
                let force_zero_energy_clone = force_zero_energy.clone();
                let charge_locked_clone = charge_locked.clone();
                let effective_energy_full_clone = effective_energy_full.clone();
                
                let closure = Closure::wrap(Box::new(move || {
                    // Now force zero energy display after the animation has had a chance to prepare
                    force_zero_energy_clone.set(true);
                    charge_locked_clone.set(true);
                    effective_energy_full_clone.set(false);
                }) as Box<dyn FnMut()>);
                
                let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
                closure.forget();
            } else {
                // Fallback if window is not available
                force_zero_energy.set(true);
                charge_locked.set(true);
                effective_energy_full.set(false);
            }
            
            // Don't let user click again until animation completes
            let e_target = e.target();
            if let Some(target) = e_target {
                if let Ok(element) = target.dyn_into::<web_sys::Element>() {
                    let _ = element.set_attribute("disabled", "disabled");
                }
            }
            
            // Clone variables for the async block
            let error = error.clone();
            let loading = loading.clone();
            let fetch_data = fetch_data.clone();
            let pax_balance = pax_balance.clone();
            let on_energy_update = on_energy_update.clone();
            let local_recharge_time = local_recharge_time.clone();
            let is_initiating_recharge = is_initiating_recharge.clone();
            let charge_locked = charge_locked.clone();
            let effective_energy_full = effective_energy_full.clone();
            let force_zero_energy = force_zero_energy.clone();
            
            spawn_local(async move {
                // Small delay to allow animation to complete
                sleep(std::time::Duration::from_millis(100)).await;
                
                loading.set(true);
                
                let token = window()
                    .and_then(|w| w.local_storage().ok().flatten())
                    .and_then(|s| s.get_item("token").ok().flatten())
                    .or_else(|| window()
                        .and_then(|w| w.session_storage().ok().flatten())
                        .and_then(|s| s.get_item("token").ok().flatten()))
                    .unwrap_or_default();
                if token.is_empty() {
                    error.set("Please log in to recharge energy".to_string());
                    loading.set(false);
                    is_initiating_recharge.set(false);
                    charge_locked.set(false);
                    force_zero_energy.set(false);
                    return;
                }
                let response_result = Request::post(&format!("{}/api/creatures/{}/energy_recharge", api_base_url, creature_id))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await;

                match response_result {
                    Ok(response) => {
                        if response.status() != 200 {
                            let text = response.text().await.unwrap_or_default();
                            if text.is_empty() {
                                error.set("Cannot recharge while listed on the market".to_string());
                            } else {
                                error.set(text);
                            }
                            loading.set(false);
                            is_initiating_recharge.set(false);
                            charge_locked.set(false);
                            force_zero_energy.set(false);
                            return;
                        }

                        let response_text = response.text().await;
                        match response_text {
                            Ok(text) => {
                                match serde_json::from_str::<EnergyResponse>(&text) {
                                    Ok(data) => {
                                        if data.success {
                                            pax_balance.set(data.pax_balance);
                                            local_recharge_time.set(data.energy_recharge_complete_at.clone());
                                            dispatch_currency_event(data.pax_balance);
                                            // Dispatch energy recharge event to notify other components
                                            dispatch_energy_recharge_event(&data.creature_id, true);
                                            if let Some(callback) = fetch_data {
                                                callback.emit(());
                                            }
                                            if let Some(callback) = on_energy_update {
                                                callback.emit((data.creature_id, data.energy_full));
                                            }
                                            effective_energy_full.set(data.energy_full);
                                            
                                            // Don't reset force_zero_energy right away, let the recharge_time handle visuals
                                            // Only reset if somehow energy_full is true (should not happen)
                                            if data.energy_full {
                                                force_zero_energy.set(false);
                                            }
                                        } else {
                                            if let Some(err_str) = data.error {
                                                error.set(err_str);
                                            } else {
                                                error.set("Failed to recharge energy".to_string());
                                            }
                                            force_zero_energy.set(false);
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse response: {:?}", e);
                                        error.set("Failed to parse server response".to_string());
                                        force_zero_energy.set(false);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to read response text: {:?}", e);
                                error.set("Failed to read response from server".to_string());
                                force_zero_energy.set(false);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Network error: {:?}", e);
                        error.set("Network error - please try again".to_string());
                        force_zero_energy.set(false);
                    }
                }
                loading.set(false);
                is_initiating_recharge.set(false);
                charge_locked.set(false);
                // Don't reset force_zero_energy here, as it should persist until local_recharge_time
                // is updated with the API response. This ensures the UI stays at 0% during the transition.
            });
        })
    };

    // Display logic - explicitly check for force_zero_energy
    let display_energy_full = *effective_energy_full && !*is_initiating_recharge && !*force_zero_energy;
    // Track if charge animation is active to show particle effects
    let is_charge_animation_active = *is_initiating_recharge || *force_zero_energy;
    
    html! {
        <div class="space-y-1">
            <div class="flex justify-between items-center">
                <span class="text-sm text-gray-600 dark:text-gray-400">{"Energy"}</span>
                <span class="text-sm text-gray-900 dark:text-white font-medium">
                    {if *is_in_chaos_realm_prop {
                        "".to_string()
                    } else if *force_zero_energy || *is_initiating_recharge {
                        "0%".to_string() // Explicitly show 0% during recharge animation
                    } else if display_energy_full {
                        "100%".to_string()
                    } else if local_recharge_time.is_some() {
                        format!("{}%", progress_percent)
                    } else {
                        "0%".to_string()
                    }}
                </span>
            </div>
            <div class="relative">
                <div class="relative h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                    <div 
                        class={classes!(
                            "absolute",
                            "top-0",
                            "left-0",
                            "h-full",
                            "transform-gpu", // Add GPU acceleration for smoother animations
                            if !*is_initiating_recharge && !*force_zero_energy {
                                "transition-all duration-500 ease-in-out"
                            } else {
                                "" // No transition during animation to prevent flickering
                            },
                            if *is_initiating_recharge {
                                "animate-energy-drain" // Add animation class when initiating recharge
                            } else {
                                ""
                            },
                            if *is_in_chaos_realm_prop { // Use state derived directly from prop
                                "bg-gradient-to-r from-purple-500 to-fuchsia-600 animate-chaos-pulse"
                            } else {
                                "bg-gradient-to-r from-teal-500 to-cyan-500"
                            }
                        )}
                        style={format!("width: {}%", 
                            if *force_zero_energy || *is_initiating_recharge {
                                0 // Force empty width when initiating recharge or forcing zero
                            } else if *is_in_chaos_realm_prop { // Use state derived directly from prop
                                // For chaos realm, use a percentage based on remaining time (100% to 0%)
                                if let Some(remaining) = *chaos_remaining_seconds {
                                    if remaining <= 0 {
                                        100
                                    } else {
                                        // Calculate progress based on total chaos realm duration (23 hours = 82800 seconds)
                                        let total_seconds = 82800;
                                        ((remaining as f64 / total_seconds as f64) * 100.0) as i32
                                    }
                                } else {
                                    100
                                }
                            } else if display_energy_full {
                                100
                            } else {
                                progress_percent
                            }
                        )}
                    >
                        {if *force_zero_energy || *is_initiating_recharge {
                            // Add a subtle visual indicator during energy drain
                            html! {
                                <div class="absolute inset-0 bg-gradient-to-r from-teal-500/20 to-cyan-500/20 animate-pulse"></div>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                    
                    // Visual effects for charging animation
                    {if is_charge_animation_active {
                        html! {
                            <>
                                // Pulse effect around the bar
                                <div class="absolute inset-0 rounded-full animate-energy-charge-pulse bg-teal-400/20"></div>
                                
                                // Energy particles
                                <div class="absolute -top-4 left-1/4 w-1 h-1 bg-cyan-400 rounded-full animate-energy-particles" style="--particle-delay: 0ms;"></div>
                                <div class="absolute -top-4 left-1/2 w-1.5 h-1.5 bg-teal-400 rounded-full animate-energy-particles animation-delay-150" style="--particle-delay: 150ms;"></div>
                                <div class="absolute -top-4 left-3/4 w-1 h-1 bg-cyan-500 rounded-full animate-energy-particles animation-delay-300" style="--particle-delay: 300ms;"></div>
                                <div class="absolute -top-4 left-1/3 w-2 h-2 bg-teal-300 rounded-full animate-energy-particles animation-delay-450" style="--particle-delay: 450ms;"></div>
                                <div class="absolute -top-4 left-2/3 w-1.5 h-1.5 bg-cyan-300 rounded-full animate-energy-particles animation-delay-600" style="--particle-delay: 600ms;"></div>
                            </>
                        }
                    } else {
                        html! {}
                    }}
                    
                    {if *is_in_chaos_realm_prop { // Use state derived directly from prop
                        if let Some(remaining) = *chaos_remaining_seconds {
                            if remaining <= 0 {
                                html! {
                                    <div class="absolute inset-0 bg-gradient-to-r from-green-700 to-emerald-700 animate-pulse" />
                                }
                            } else {
                                html! {
                                    <>
                                        // Base glow effect inside the bar
                                        <div class="absolute inset-0 bg-gradient-to-r from-purple-500/20 to-fuchsia-600/20 animate-pulse" />
                                        
                                        // Random pulse effects with varied positions and timings
                                        <div class="absolute inset-0 overflow-hidden">
                                            // Horizontal pulses
                                            <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-purple-500/50 to-transparent left-[7%] animate-chaos-pulse-1" />
                                            <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-fuchsia-500/50 to-transparent left-[28%] animate-chaos-pulse-2" />
                                            <div class="absolute h-full w-20 bg-gradient-to-r from-transparent via-purple-400/50 to-transparent left-[46%] animate-chaos-pulse-3" />
                                            <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-fuchsia-400/50 to-transparent left-[67%] animate-chaos-pulse-4" />
                                            <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-purple-500/50 to-transparent left-[88%] animate-chaos-pulse-5" />
                                            
                                            // Offset pulses with delays
                                            <div class="absolute h-full w-20 bg-gradient-to-r from-transparent via-purple-500/40 to-transparent left-[15%] animate-chaos-pulse-2 delay-[350ms]" />
                                            <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-fuchsia-500/40 to-transparent left-[38%] animate-chaos-pulse-3 delay-[650ms]" />
                                            <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-purple-400/40 to-transparent left-[58%] animate-chaos-pulse-4 delay-[150ms]" />
                                            <div class="absolute h-full w-20 bg-gradient-to-r from-transparent via-fuchsia-400/40 to-transparent left-[78%] animate-chaos-pulse-5 delay-[450ms]" />
                                            
                                            // Additional offset pulses for more randomness
                                            <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-purple-500/30 to-transparent left-[22%] animate-chaos-pulse-1 delay-[800ms]" />
                                            <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-fuchsia-500/30 to-transparent left-[52%] animate-chaos-pulse-2 delay-[950ms]" />
                                            <div class="absolute h-full w-20 bg-gradient-to-r from-transparent via-purple-400/30 to-transparent left-[82%] animate-chaos-pulse-3 delay-[250ms]" />
                                        </div>
                                    </>
                                }
                            }
                        } else {
                            html! {
                                <>
                                    <div class="absolute inset-0 bg-gradient-to-r from-purple-500/20 to-fuchsia-600/20 animate-pulse" />
                                    <div class="absolute inset-0 overflow-hidden">
                                        <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-purple-500/50 to-transparent left-[7%] animate-chaos-pulse-1" />
                                        <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-fuchsia-500/50 to-transparent left-[28%] animate-chaos-pulse-2" />
                                    </div>
                                </>
                            }
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
            <div class="flex justify-between items-center py-2">
                <button
                    onclick={recharge_energy}
                    disabled={*loading || *is_in_chaos_realm_prop || display_energy_full || local_recharge_time.is_some() || *force_zero_energy}
                    class={classes!(
                        "px-2",
                        "py-2",
                        "text-xs",
                        "font-medium",
                        "rounded-lg",
                        "transition-all",
                        "duration-300",
                        "relative", // Added for absolute positioning of effects
                        "overflow-hidden", // Keep effects inside button
                        if *loading || *is_in_chaos_realm_prop || display_energy_full || local_recharge_time.is_some() || *force_zero_energy {
                            if *force_zero_energy || *is_initiating_recharge {
                                // Special energized styling during charging animation
                                "bg-gradient-to-r from-teal-600 to-cyan-600 text-white cursor-not-allowed relative"
                            } else {
                                "bg-gray-400 dark:bg-gray-600 cursor-not-allowed"
                            }
                        } else {
                            "bg-gradient-to-r from-teal-500 to-cyan-500 hover:from-teal-600 hover:to-cyan-600 text-white shadow-md hover:shadow-lg"
                        }
                    )}
                >
                    { if *loading {
                        html! { 
                            <>
                                {"Charging..."}
                                // Subtle shimmer effect during charging
                                <div class="absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent animate-[shimmer_1.5s_infinite]" style="background-size: 200% 100%;"></div>
                            </>
                        }
                    } else if *is_in_chaos_realm_prop { // Use state derived directly from prop
                        html! { "In Chaos Realm" }
                    } else if *force_zero_energy {
                        html! { 
                            <>
                                {"Charging..."}
                                // Add charging visual effects
                                <div class="absolute inset-0 flex justify-center items-center">
                                    <div class="absolute w-full h-full bg-gradient-to-r from-transparent via-white/10 to-transparent animate-[shimmer_1.5s_infinite]" style="background-size: 200% 100%;"></div>
                                </div>
                            </>
                        }
                    } else if display_energy_full {
                        html! { 
                            "Energy Full"
                        }
                    } else if local_recharge_time.is_some() {
                        html! { 
                            <>
                                {"Charging..."}
                                // Add charging visual effects
                                <div class="absolute inset-0 flex justify-center items-center">
                                    <div class="absolute w-full h-full bg-gradient-to-r from-transparent via-white/10 to-transparent animate-[shimmer_1.5s_infinite]" style="background-size: 200% 100%;"></div>
                                </div>
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <div class="flex items-center space-x-1">
                                    <span>{"Charge"}</span>
                                    <span class="text-xs opacity-90">{format!("({} pax)", energy_cost)}</span>
                                </div>
                            </>
                        }
                    } }
                </button>
            </div>
            if !(*error).is_empty() {
                <div class="text-sm text-red-400">{&*error}</div>
            }
        </div>
    }
} 
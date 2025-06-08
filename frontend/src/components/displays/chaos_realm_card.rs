use yew::prelude::*;
use crate::models::{Creature, ChaosRealmResponse, ChaosRealmStatusResponse};
use wasm_bindgen_futures::spawn_local;
use gloo_timers::callback::Interval;
use reqwest::Client;
use web_sys::{window, CustomEventInit, CustomEvent};
use wasm_bindgen::JsValue;
use js_sys::Object;
use crate::config::get_api_base_url;
use js_sys::Date;

// Helper function to get auth token
async fn get_auth_token() -> String {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
        .unwrap_or_default()
}

pub async fn enter_chaos_realm(creature_id: &str) -> Result<ChaosRealmResponse, reqwest::Error> {
    let client = Client::new();
    let token = get_auth_token().await;
    let api_base = get_api_base_url();
    
    let url = if api_base.is_empty() {
        let origin = window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:3000".to_string());
        
        format!("{}/api/creatures/{}/chaos-realm/enter", origin, creature_id)
    } else {
        format!("{}/api/creatures/{}/chaos-realm/enter", api_base, creature_id)
    };
    
    client.post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json::<ChaosRealmResponse>()
        .await
}

pub async fn claim_chaos_realm_reward(creature_id: &str) -> Result<ChaosRealmResponse, reqwest::Error> {
    let client = Client::new();
    let token = get_auth_token().await;
    let api_base = get_api_base_url();
    
    let url = if api_base.is_empty() {
        let origin = window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:3000".to_string());
        
        format!("{}/api/creatures/{}/chaos-realm/claim", origin, creature_id)
    } else {
        format!("{}/api/creatures/{}/chaos-realm/claim", api_base, creature_id)
    };
    
    client.post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json::<ChaosRealmResponse>()
        .await
}

pub async fn get_chaos_realm_status(creature_id: &str) -> Result<ChaosRealmStatusResponse, reqwest::Error> {
    let client = Client::new();
    let token = get_auth_token().await;
    let api_base = get_api_base_url();
    
    let url = if api_base.is_empty() {
        let origin = window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:3000".to_string());
        
        format!("{}/api/creatures/{}/chaos-realm/status", origin, creature_id)
    } else {
        format!("{}/api/creatures/{}/chaos-realm/status", api_base, creature_id)
    };
    
    client.get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json::<ChaosRealmStatusResponse>()
        .await
}

#[derive(Properties, PartialEq)]
pub struct ChaosRealmCardProps {
    pub creature: Creature,
    pub loading_chaos: bool,
    pub error: String,
    pub fetch_data: Option<Callback<()>>,
    #[prop_or_default]
    pub is_recharging: bool,
}

#[function_component(ChaosRealmCard)]
pub fn chaos_realm_card(props: &ChaosRealmCardProps) -> Html {
    let remaining_time = use_state(|| None::<i64>);
    let loading_chaos = use_state(|| props.loading_chaos);
    let error = use_state(|| props.error.clone());
    let status_check_interval = use_state(|| None::<Interval>);
    // Add a visual state for transition effects
    let visually_in_realm = use_state(|| props.creature.in_chaos_realm);
    
    {
        let remaining_time = remaining_time.clone();
        let in_chaos_realm = props.creature.in_chaos_realm;
        let creature_id = props.creature.id.to_string();
        let status_check_interval = status_check_interval.clone();
        let error_state = error.clone();
        
        use_effect_with(in_chaos_realm, move |in_chaos_realm| {
            if *in_chaos_realm {
                // Instead of calculating locally, fetch status from backend every second
                let creature_id_clone = creature_id.clone();
                let remaining_time_clone = remaining_time.clone();
                let error_state_clone = error_state.clone();
                
                let interval = Interval::new(1000, move || {
                    let creature_id_inner = creature_id_clone.clone();
                    let remaining_time_inner = remaining_time_clone.clone();
                    let error_state_inner = error_state_clone.clone();
                    
                    spawn_local(async move {
                        match get_chaos_realm_status(&creature_id_inner).await {
                            Ok(status) => {
                                if let Some(seconds) = status.remaining_seconds {
                                    remaining_time_inner.set(Some(seconds));
                                }
                            },
                            Err(e) => {
                                error_state_inner.set(format!("Failed to check status: {}", e));
                            }
                        }
                    });
                });
                
                status_check_interval.set(Some(interval));
                
                // Initial status check
                let creature_id_init = creature_id.clone();
                let remaining_time_init = remaining_time.clone();
                let error_state_init = error_state.clone();
                
                spawn_local(async move {
                    match get_chaos_realm_status(&creature_id_init).await {
                        Ok(status) => {
                            if let Some(seconds) = status.remaining_seconds {
                                remaining_time_init.set(Some(seconds));
                            }
                        },
                        Err(e) => {
                            error_state_init.set(format!("Failed to check status: {}", e));
                        }
                    }
                });
            }
            
            // Cleanup function
            let status_check_interval_clone = status_check_interval.clone();
            move || {
                // We need to take ownership of the interval to cancel it
                if let Some(_) = *status_check_interval_clone {
                    // Set to None first to avoid borrowing issues
                    status_check_interval_clone.set(None);
                }
            }
        });
    }

    let on_claim = {
        let creature_id = props.creature.id.to_string();
        let loading_chaos = loading_chaos.clone();
        let error = error.clone();
        let fetch_data = props.fetch_data.clone();
        let visually_in_realm = visually_in_realm.clone();
        
        Callback::from(move |_| {
            let creature_id = creature_id.clone();
            let loading_chaos = loading_chaos.clone();
            let error_setter = error.clone();
            let fetch_data = fetch_data.clone();
            let visually_in_realm = visually_in_realm.clone();
            
            loading_chaos.set(true);
            error_setter.set(String::new());
            
            spawn_local(async move {
                match claim_chaos_realm_reward(&creature_id).await {
                    Ok(response) => {
                        if response.success {
                            // Update visual state to match the backend state
                            visually_in_realm.set(false);
                            
                            // Update currency in local storage
                            if let Some(window) = window() {
                                if let Some(storage) = window.local_storage().ok().flatten() {
                                    if let Ok(currency_str) = storage.get_item("currency") {
                                        if let Some(currency_str) = currency_str {
                                            if let Ok(current_currency) = currency_str.parse::<i32>() {
                                                let _ = storage.set_item("currency", &(current_currency + response.new_balance).to_string());
                                            }
                                        }
                                    }
                                }
                                
                                // Dispatch currency update event
                                let event_init = CustomEventInit::new();
                                event_init.set_detail(&JsValue::from_f64(response.new_balance as f64));
                                if let Ok(event) = CustomEvent::new_with_event_init_dict(
                                    "currencyUpdate",
                                    &event_init
                                ) {
                                    let _ = window.dispatch_event(&event);
                                }
                                
                                // Show notification about the reward
                                let notification_init = CustomEventInit::new();
                                notification_init.set_detail(&JsValue::from_str(&format!("Received {} Pax from Chaos Realm!", response.reward_amount)));
                                if let Ok(notify_event) = CustomEvent::new_with_event_init_dict(
                                    "notification",
                                    &notification_init
                                ) {
                                    let _ = window.dispatch_event(&notify_event);
                                }
                                
                                // Dispatch creature update event with energy_full set to false
                                let creature_event_init = CustomEventInit::new();
                                let creature_data = Object::new();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("id"), &JsValue::from_str(&creature_id)).unwrap();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("chaos_realm_reward_claimed"), &JsValue::from_bool(true)).unwrap();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("in_chaos_realm"), &JsValue::from_bool(false)).unwrap();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("energy_full"), &JsValue::from_bool(false)).unwrap();
                                creature_event_init.set_detail(&creature_data);
                                let creature_event = CustomEvent::new_with_event_init_dict(
                                    "creatureUpdate",
                                    &creature_event_init
                                ).unwrap();
                                window.dispatch_event(&creature_event).unwrap();
                            }
                            
                            // After claiming, check status once more to ensure UI is in sync
                            match get_chaos_realm_status(&creature_id).await {
                                Ok(_) => {
                                    // Status updated, will be reflected in the UI through the interval
                                },
                                Err(e) => {
                                    log::warn!("Failed to update status after claim: {}", e);
                                }
                            }
                            
                            if let Some(fetch_data) = fetch_data {
                                fetch_data.emit(());
                            }
                        } else if let Some(err) = response.error {
                            error_setter.set(err);
                        }
                    }
                    Err(e) => {
                        error_setter.set(e.to_string());
                    }
                }
                loading_chaos.set(false);
            });
        })
    };

    let on_enter = {
        let creature_id = props.creature.id.to_string();
        let error = error.clone();
        let fetch_data = props.fetch_data.clone();
        let remaining_time_state = remaining_time.clone();
        let loading_chaos = loading_chaos.clone();
        let visually_in_realm = visually_in_realm.clone();
        
        Callback::from(move |_| {
            let creature_id = creature_id.clone();
            let error_setter = error.clone();
            let fetch_data = fetch_data.clone();
            let remaining_time = remaining_time_state.clone();
            let loading_chaos = loading_chaos.clone();
            let visually_in_realm = visually_in_realm.clone();
            
            // Clear any previous errors
            error_setter.set(String::new());
            // Start loading/transition state immediately
            loading_chaos.set(true);
            
            spawn_local(async move {
                match enter_chaos_realm(&creature_id).await {
                    Ok(response) => {
                        if response.success {
                            // Update visual state first for smooth transition
                            visually_in_realm.set(true);
                            
                            if let Some(window) = window() {
                                // No need to update currency since entry is now free
                                
                                let creature_event_init = CustomEventInit::new();
                                let creature_data = Object::new();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("id"), &JsValue::from_str(&creature_id)).unwrap();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("energy"), &JsValue::from_f64(0.0)).unwrap();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("in_chaos_realm"), &JsValue::from_bool(true)).unwrap();
                                
                                // Include entry time to ensure styling updates properly
                                let now = Date::new_0();
                                let iso_string = now.to_iso_string();
                                js_sys::Reflect::set(&creature_data, &JsValue::from_str("chaos_realm_entry_at"), &iso_string).unwrap();
                                
                                creature_event_init.set_detail(&creature_data);
                                let creature_event = CustomEvent::new_with_event_init_dict(
                                    "creatureUpdate",
                                    &creature_event_init
                                ).unwrap();
                                window.dispatch_event(&creature_event).unwrap();
                            }
                            
                            // After entering, check status immediately to get accurate timer
                            match get_chaos_realm_status(&creature_id).await {
                                Ok(status) => {
                                    // Status updated, will be reflected in the UI through the interval
                                    if let Some(seconds) = status.remaining_seconds {
                                        remaining_time.set(Some(seconds));
                                    }
                                },
                                Err(e) => {
                                    log::warn!("Failed to update status after entering: {}", e);
                                }
                            }
                            
                            if let Some(fetch_data) = fetch_data {
                                fetch_data.emit(());
                            }
                        } else if let Some(err) = response.error {
                            error_setter.set(err);
                        }
                    }
                    Err(e) => {
                        error_setter.set(e.to_string());
                    }
                }
                // End loading state after everything is processed
                loading_chaos.set(false);
            });
        })
    };

    // Button text for entering Chaos Realm
    let button_text = "Enter Chaos Realm".to_string();

    html! {
        if *visually_in_realm {
            <button 
                class="w-full py-3 px-4 bg-gradient-to-r from-purple-500 to-purple-600 text-white rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed hover:from-purple-600 hover:to-purple-700 transition-all duration-500 ease-in-out animate-fadeIn"
                onclick={on_claim}
                disabled={*loading_chaos || remaining_time.map_or(true, |t| t > 0)}
            >
                { claim_button_text(remaining_time.clone(), loading_chaos.clone()) }
            </button>
        } else {
            <button 
                class="w-full py-3 px-4 bg-gradient-to-r from-purple-500 to-purple-600 text-white rounded-lg font-medium hover:from-purple-600 hover:to-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-500 ease-in-out animate-fadeIn"
                onclick={on_enter}
                disabled={!props.creature.energy_full || *loading_chaos || props.is_recharging}
                title="Requires full energy to enter"
            >
                {
                    if !props.creature.energy_full || props.is_recharging {
                        "Requires Full Energy".to_string()
                    } else {
                        button_text
                    }
                }
            </button>
        }
    }
}

// Helper function to determine claim button text
fn claim_button_text(remaining_time: UseStateHandle<Option<i64>>, loading_chaos: UseStateHandle<bool>) -> String {
    if let Some(time) = *remaining_time {
        if time <= 0 {
            "Claim Reward".to_string()
        } else if *loading_chaos {
            "Claiming...".to_string()
        } else {
            "In Chaos Realm".to_string()
        }
    } else if *loading_chaos {
        "Claiming...".to_string()
    } else {
        "In Chaos Realm".to_string()
    }
}

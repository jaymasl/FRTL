use crate::components::displays::DisplayItem;
use crate::models::{Egg, Creature, Scroll};
use gloo_net::http::Request;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use web_sys::window;
use super::state::HatchState;
use serde_json::Value;
use serde::{Deserialize, Serialize};
use js_sys::Date;
use wasm_bindgen::JsValue;
use std::collections::HashMap;
use crate::config::get_api_base_url;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct CreatureResponse {
    pub id: Uuid,
    pub display_name: String,
    pub original_egg_created_at: Option<String>,
    pub hatched_at: Option<String>,
    pub energy_full: bool,
    pub energy_recharge_complete_at: Option<String>,
    pub stats: HashMap<String, Value>,
    pub rarity: String,
    pub streak: i32,
    pub soul: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BindResponse {
    pub success: bool,
    pub creature: Option<CreatureResponse>,
    pub error: Option<String>,
    pub new_balance: Option<i32>,
}

#[derive(Debug, Serialize)]
struct BindRequest {
    sacrifice_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct GenerateEggResponse {
    new_balance: i32,
    egg: Egg,
}

pub fn handle_item_click(selected_item: UseStateHandle<Option<DisplayItem>>) -> Callback<DisplayItem> {
    Callback::from(move |item: DisplayItem| {
        selected_item.set(Some(item));
    })
}

pub fn handle_bind_select(selected: UseStateHandle<Option<Uuid>>) -> Callback<Uuid> {
    Callback::from(move |id: Uuid| {
        selected.set(Some(id));
    })
}

pub fn handle_bind(
    selected: UseStateHandle<Option<Uuid>>,
    target: Creature,
    _creatures: UseStateHandle<Vec<Creature>>,
    on_success: Callback<Option<CreatureResponse>>,
    on_error: Callback<String>,
    on_close: Callback<()>,
    fetch_data: Callback<()>,
) -> Callback<()> {
    Callback::from(move |_| {
        let selected = selected.clone();
        let target = target.clone();
        let on_success = on_success.clone();
        let on_error = on_error.clone();
        let on_close = on_close.clone();
        let fetch_data = fetch_data.clone();

        if let Some(sacrifice_id) = *selected {
            let token = window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item("token").ok().flatten())
                .or_else(|| {
                    window()
                        .and_then(|w| w.session_storage().ok().flatten())
                        .and_then(|s| s.get_item("token").ok().flatten())
                });

            let token = match token {
                Some(t) if !t.is_empty() => t,
                _ => {
                    on_error.emit("No authentication token found. Please log in again.".to_string());
                    return;
                }
            };

            spawn_local(async move {
                match Request::post(&format!("{}/api/creatures/{}/bind", get_api_base_url(), target.id))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&BindRequest { sacrifice_id })
                    .unwrap()
                    .send()
                    .await 
                {
                    Ok(response) => {
                        let status = response.status();
                        log::debug!("Received bind response");
                        
                        match status {
                            200 => {
                                if let Ok(bind_response) = response.json::<BindResponse>().await {
                                    // Update currency if available
                                    if let Some(new_balance) = bind_response.new_balance {
                                        if let Some(window) = window() {
                                            if let Some(storage) = window.local_storage().ok().flatten() {
                                                let _ = storage.set_item("currency", &new_balance.to_string());
                                            }
                                            
                                            let event_init = web_sys::CustomEventInit::new();
                                            event_init.set_detail(&JsValue::from_f64(new_balance as f64));
                                            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                                "currencyUpdate",
                                                &event_init
                                            ) {
                                                let _ = window.dispatch_event(&event);
                                            }
                                        }
                                    }

                                    if bind_response.success {
                                        on_success.emit(bind_response.creature);
                                        fetch_data.emit(());
                                        on_close.emit(());
                                    } else if let Some(error) = bind_response.error {
                                        log::error!("Failed to parse response");
                                        on_error.emit(error);
                                    }
                                } else {
                                    log::error!("Failed to parse bind response");
                                    on_error.emit("Failed to parse server response".to_string());
                                }
                            }
                            401 => {
                                on_error.emit("Session expired. Please log in again.".to_string());
                            }
                            402 => {
                                on_error.emit("Not enough Pax (requires 55)".to_string());
                            }
                            429 => {
                                on_error.emit("Too Many Requests".to_string());
                                // Clear the error after 5 seconds to match the backend cooldown
                                let on_error = on_error.clone();
                                spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(5_000).await;
                                    on_error.emit(String::new());
                                });
                            }
                            _ => {
                                log::error!("Network error occurred");
                                if let Ok(response_text) = response.text().await {
                                    log::error!("Response body: {}", response_text);
                                    on_error.emit(response_text);
                                } else {
                                    on_error.emit("Failed to bind creatures".to_string());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Network error: {:?}", e);
                        on_error.emit("Network error occurred".to_string());
                    }
                }
            });
        }
    })
}

pub fn handle_close(
    selected_item: UseStateHandle<Option<DisplayItem>>,
    hatch_state: UseStateHandle<HatchState>
) -> Callback<()> {
    Callback::from(move |_| {
        selected_item.set(None);
        hatch_state.set(HatchState::default());
    })
}

pub fn handle_hatch(
    loading: UseStateHandle<bool>,
    eggs: UseStateHandle<Vec<Egg>>,
    creatures: UseStateHandle<Vec<Creature>>,
    token: String,
    hatch_state: UseStateHandle<HatchState>,
    selected_item: UseStateHandle<Option<DisplayItem>>,
) -> Callback<Uuid> {
    Callback::from(move |egg_id| {
        let loading = loading.clone();
        let eggs = eggs.clone();
        let creatures = creatures.clone();
        let token = token.clone();
        let hatch_state = hatch_state.clone();
        let selected_item = selected_item.clone();

        // Only check if we're already loading
        if !*loading {
            loading.set(true);
            hatch_state.set(HatchState {
                egg_id: Some(egg_id),
                error: String::new(),
                last_attempt: Some(Date::now()),
            });

            spawn_local(async move {
                log::info!("Initiating egg hatch...");

                match Request::post(&format!("{}/api/eggs/{}/generate-creature", get_api_base_url(), egg_id))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.status() {
                            200 => {
                                if let Ok(response_text) = response.text().await {
                                    match serde_json::from_str::<Creature>(&response_text) {
                                        Ok(creature) => {
                                            log::debug!("Received creature response");
                                            log::info!("Successfully hatched egg");
                                            // Update local states immediately
                                            eggs.set((*eggs).iter().filter(|e| e.id != egg_id).cloned().collect());
                                            creatures.set(std::iter::once(creature.clone()).chain((*creatures).clone()).collect());
                                            selected_item.set(Some(DisplayItem::Creature(creature)));
                                            hatch_state.set(HatchState::default());
                                        }
                                        Err(e) => {
                                            log::error!("Failed to parse creature response: {:?}", e);
                                            log::error!("Response text: {}", response_text);
                                            hatch_state.set(HatchState {
                                                egg_id: Some(egg_id),
                                                error: format!("Failed to parse server response: {}", e),
                                                last_attempt: Some(Date::now()),
                                            });
                                        }
                                    }
                                } else {
                                    log::error!("Failed to get response text");
                                    hatch_state.set(HatchState {
                                        egg_id: Some(egg_id),
                                        error: "Failed to read server response".to_string(),
                                        last_attempt: Some(Date::now()),
                                    });
                                }
                            }
                            429 => {
                                hatch_state.set(HatchState {
                                    egg_id: Some(egg_id),
                                    error: "Too Many Requests".to_string(),
                                    last_attempt: Some(Date::now()),
                                });
                                // Clear the error after 5 seconds to match the backend cooldown
                                let hatch_state = hatch_state.clone();
                                spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(5_000).await;
                                    hatch_state.set(HatchState {
                                        egg_id: Some(egg_id),
                                        error: String::new(),
                                        last_attempt: Some(Date::now()),
                                    });
                                });
                            }
                            500 => {
                                if let Ok(error_response) = response.json::<serde_json::Value>().await {
                                    if let Some(message) = error_response.get("error").and_then(|e| e.as_str()) {
                                        hatch_state.set(HatchState {
                                            egg_id: Some(egg_id),
                                            error: message.to_string(),
                                            last_attempt: Some(Date::now()),
                                        });
                                    }
                                }
                            }
                            _ => {
                                hatch_state.set(HatchState {
                                    egg_id: Some(egg_id),
                                    error: "Failed to hatch egg".to_string(),
                                    last_attempt: Some(Date::now()),
                                });
                            }
                        }
                    }
                    Err(_) => {
                        hatch_state.set(HatchState {
                            egg_id: Some(egg_id),
                            error: "Network error".to_string(),
                            last_attempt: Some(Date::now()),
                        });
                    }
                }
                loading.set(false);
            });
        }
    })
}

/// Handles summoning a new egg from a scroll. Similar to handle_hatch, it sends a request to generate a new egg
/// and then updates the UI instantly.
pub fn handle_summon(
    scroll: Scroll,
    eggs: UseStateHandle<Vec<Egg>>,
    token: String,
    selected_item: UseStateHandle<Option<DisplayItem>>,
    on_error: Callback<String>,
    on_close: Callback<()>,
    fetch_data: Callback<()>,
) -> Callback<()> {
    Callback::from(move |_| {
        let token = token.clone();
        let scroll_id = scroll.id;
        let on_error = on_error.clone();
        let on_close = on_close.clone();
        let fetch_data = fetch_data.clone();
        let selected_item = selected_item.clone();
        let eggs_state = eggs.clone();

        spawn_local(async move {
            match Request::post(&format!("{}/api/generator/generate-egg", get_api_base_url()))
                .header("Authorization", &format!("Bearer {}", token))
                .json(&serde_json::json!({ "scroll_id": scroll_id }))
                .unwrap()
                .send()
                .await
            {
                Ok(response) => {
                    let status = response.status();
                    match status {
                        200 => {
                            if let Ok(data) = response.json::<GenerateEggResponse>().await {
                                // Update currency in local storage
                                if let Some(window) = web_sys::window() {
                                    if let Some(storage) = window.local_storage().ok().flatten() {
                                        let _ = storage.set_item("currency", &data.new_balance.to_string());
                                    }
                                    let event_init = web_sys::CustomEventInit::new();
                                    event_init.set_detail(&wasm_bindgen::JsValue::from_f64(data.new_balance as f64));
                                    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("currencyUpdate", &event_init) {
                                        let _ = window.dispatch_event(&event);
                                    }
                                }

                                // Inline UI update: prepend new egg and set focus
                                let mut new_eggs = vec![data.egg.clone()];
                                new_eggs.extend((*eggs_state).clone());
                                eggs_state.set(new_eggs);
                                selected_item.set(Some(DisplayItem::Egg(data.egg.clone())));

                                on_close.emit(());
                                gloo_timers::future::TimeoutFuture::new(200).await;
                                fetch_data.emit(());
                            } else {
                                on_error.emit("Failed to parse egg generation response.".to_string());
                            }
                        },
                        402 => on_error.emit("Not enough currency.".to_string()),
                        401 => on_error.emit("Please log in again.".to_string()),
                        429 => on_error.emit("Too many requests. Please try again later.".to_string()),
                        _   => on_error.emit("Failed to generate egg.".to_string()),
                    }
                },
                Err(e) => {
                    log::error!("Network error: {:?}", e);
                    on_error.emit("Network error occurred".to_string());
                }
            }
        });
    })
}

/// Handles updating a creature's energy. Similar to handle_summon and handle_hatch,
/// it updates the UI immediately and then refreshes in the background.
pub fn handle_energy(
    creature_id: Uuid,
    energy_full: bool,
    creatures: UseStateHandle<Vec<Creature>>,
    selected_item: UseStateHandle<Option<DisplayItem>>,
    fetch_data: Callback<()>,
) -> Callback<()> {
    Callback::from(move |_| {
        let creatures = creatures.clone();
        let selected_item = selected_item.clone();
        let fetch_data = fetch_data.clone();

        let token = window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten())
            .or_else(|| {
                window()
                    .and_then(|w| w.session_storage().ok().flatten())
                    .and_then(|s| s.get_item("token").ok().flatten())
            });

        if let Some(token) = token {
            // First update the UI optimistically
            let mut updated_creatures = (*creatures).clone();
            if let Some(creature) = updated_creatures.iter_mut().find(|c| c.id == creature_id) {
                creature.energy_full = energy_full;
                creatures.set(updated_creatures.clone());
                
                // Update the selected item if it's the same creature
                if let Some(DisplayItem::Creature(ref selected_creature)) = *selected_item {
                    if selected_creature.id == creature_id {
                        selected_item.set(Some(DisplayItem::Creature(Creature {
                            energy_full,
                            ..selected_creature.clone()
                        })));
                    }
                }
            }

            // Only make the API call if we're starting a recharge (energy_full is false)
            // If energy_full is true, it means the energy just completed charging, so no need for API call
            if !energy_full {
                // Check if the creature is already recharging to avoid duplicate requests
                let already_recharging = if let Some(DisplayItem::Creature(ref creature)) = *selected_item {
                    if creature.id == creature_id {
                        creature.energy_recharge_complete_at.is_some()
                    } else {
                        false
                    }
                } else {
                    // Check in the full creatures list
                    updated_creatures.iter()
                        .find(|c| c.id == creature_id)
                        .map(|c| c.energy_recharge_complete_at.is_some())
                        .unwrap_or(false)
                };

                // Only proceed with API call if not already recharging
                if !already_recharging {
                    // Then make the API call to recharge energy
                    spawn_local(async move {
                        match Request::post(&format!("{}/api/creatures/{}/energy_recharge", get_api_base_url(), creature_id))
                            .header("Authorization", &format!("Bearer {}", token))
                            .send()
                            .await 
                        {
                            Ok(response) => {
                                let status = response.status();
                                
                                if status == 200 {
                                    if let Ok(data) = response.json::<Value>().await {
                                        // Update currency if available
                                        if let Some(new_balance) = data["pax_balance"].as_i64() {
                                            if let Some(window) = window() {
                                                if let Some(storage) = window.local_storage().ok().flatten() {
                                                    let _ = storage.set_item("currency", &new_balance.to_string());
                                                }
                                                
                                                let event_init = web_sys::CustomEventInit::new();
                                                event_init.set_detail(&JsValue::from_f64(new_balance as f64));
                                                if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                                    "currencyUpdate",
                                                    &event_init
                                                ) {
                                                    let _ = window.dispatch_event(&event);
                                                }
                                            }
                                        }

                                        // If we have a creature in the response, update with complete data
                                        if let Ok(updated_creature) = serde_json::from_value::<Creature>(data["creature"].clone()) {
                                            let mut updated_creatures = (*creatures).clone();
                                            if let Some(creature) = updated_creatures.iter_mut().find(|c| c.id == creature_id) {
                                                *creature = updated_creature.clone();
                                                creatures.set(updated_creatures);
                                                
                                                // Update the selected item if it's the same creature
                                                if let Some(DisplayItem::Creature(ref selected_creature)) = *selected_item {
                                                    if selected_creature.id == creature_id {
                                                        selected_item.set(Some(DisplayItem::Creature(updated_creature)));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if status == 400 {
                                    // If the backend rejected the request because energy is already full,
                                    // we should update our local state to reflect that
                                    let mut updated_creatures = (*creatures).clone();
                                    if let Some(creature) = updated_creatures.iter_mut().find(|c| c.id == creature_id) {
                                        creature.energy_full = true;
                                        creature.energy_recharge_complete_at = None;
                                        creatures.set(updated_creatures.clone());
                                        
                                        // Update the selected item if it's the same creature
                                        if let Some(DisplayItem::Creature(ref selected_creature)) = *selected_item {
                                            if selected_creature.id == creature_id {
                                                selected_item.set(Some(DisplayItem::Creature(Creature {
                                                    energy_full: true,
                                                    energy_recharge_complete_at: None,
                                                    ..selected_creature.clone()
                                                })));
                                            }
                                        }
                                    }
                                    
                                    // Remove warning log for energy recharge rejection
                                    if let Ok(_error_text) = response.text().await {
                                        // log removed
                                    }
                                }
                                
                                // Refresh data regardless of response
                                fetch_data.emit(());
                            },
                            Err(e) => {
                                // Log the error
                                log::error!("Energy recharge network error: {:?}", e);
                                
                                // On error, still refresh to get the latest state
                                fetch_data.emit(());
                            }
                        }
                    });
                }
            } else {
                // If energy is full, just refresh the data to ensure UI is up to date
                fetch_data.emit(());
            }
        }
    })
}
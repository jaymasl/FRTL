use gloo_net::http::Request;
use wasm_bindgen::JsValue;
use web_sys::window;
use yew::prelude::*;
use crate::components::displays::DisplayItem;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use serde::Serialize;
use crate::config::get_api_base_url;

#[derive(Debug, Serialize)]
pub struct CreateListingRequest {
    pub item_id: Uuid,
    pub item_type: String,
    pub price: i32,
    pub quantity: i32,
}

pub fn handle_buy(
    token: String,
    listings: UseStateHandle<Vec<super::MarketListing>>,
    listing_states: UseStateHandle<std::collections::HashMap<Uuid, super::ListingState>>,
) -> Callback<Uuid> {
    Callback::from(move |listing_id: Uuid| {
        let token = token.clone();
        let listings = listings.clone();
        let listing_states = listing_states.clone();

        // Update loading state
        {
            let mut states = (*listing_states).clone();
            states.insert(listing_id, super::ListingState {
                loading: true,
                error: String::new(),
            });
            listing_states.set(states);
        }

        spawn_local(async move {
            match Request::post(&format!("{}/api/market/listings/{}/buy", get_api_base_url(), listing_id))
                .header("Authorization", &format!("Bearer {}", token))
                .send()
                .await
            {
                Ok(response) => {
                    match response.status() {
                        200 => {
                            if let Ok(json) = response.json::<serde_json::Value>().await {
                                if json.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    if let Some(new_balance) = json.get("data").and_then(|d| d.as_i64()) {
                                        // Update currency in local storage and dispatch event
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

                                    // Remove only the purchased listing
                                    let mut current_listings = (*listings).clone();
                                    current_listings.retain(|listing| listing.id != listing_id);
                                    listings.set(current_listings);
                                    
                                    // Clear the listing state
                                    let mut states = (*listing_states).clone();
                                    states.remove(&listing_id);
                                    listing_states.set(states);

                                    // Trigger a profile refresh to update currency
                                    if let Some(window) = window() {
                                        spawn_local(async move {
                                            if let Ok(response) = Request::get(&format!("{}/api/profile", get_api_base_url()))
                                                .header("Authorization", &format!("Bearer {}", token))
                                                .send()
                                                .await 
                                            {
                                                if response.status() == 200 {
                                                    if let Ok(profile) = response.json::<serde_json::Value>().await {
                                                        if let Some(balance) = profile.get("currency_balance").and_then(|b| b.as_i64()) {
                                                            if let Some(storage) = window.local_storage().ok().flatten() {
                                                                let _ = storage.set_item("currency", &balance.to_string());
                                                            }
                                                            
                                                            let event_init = web_sys::CustomEventInit::new();
                                                            event_init.set_detail(&JsValue::from_f64(balance as f64));
                                                            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                                                "currencyUpdate",
                                                                &event_init
                                                            ) {
                                                                let _ = window.dispatch_event(&event);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                    }
                                } else {
                                    // Purchase failed (e.g., insufficient funds); show error message
                                    let error_msg = json.get("error")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Purchase failed")
                                        .to_string();
                                    let mut states = (*listing_states).clone();
                                    states.insert(listing_id, super::ListingState {
                                        loading: false,
                                        error: error_msg,
                                    });
                                    listing_states.set(states);
                                }
                            }
                        }
                        429 => {
                            let mut states = (*listing_states).clone();
                            states.insert(listing_id, super::ListingState {
                                loading: false,
                                error: "Too Many Requests".to_string(),
                            });
                            listing_states.set(states);

                            // Clear the error after 5 seconds
                            let listing_states = listing_states.clone();
                            spawn_local(async move {
                                gloo_timers::future::TimeoutFuture::new(5_000).await;
                                let mut states = (*listing_states).clone();
                                states.insert(listing_id, super::ListingState::default());
                                listing_states.set(states);
                            });
                        }
                        _ => {
                            let error_msg = if let Ok(err_response) = response.json::<serde_json::Value>().await {
                                err_response.get("error")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Failed to purchase item")
                                    .to_string()
                            } else {
                                "Failed to purchase item".to_string()
                            };

                            let mut states = (*listing_states).clone();
                            states.insert(listing_id, super::ListingState {
                                loading: false,
                                error: error_msg,
                            });
                            listing_states.set(states);
                        }
                    }
                }
                Err(_) => {
                    let mut states = (*listing_states).clone();
                    states.insert(listing_id, super::ListingState {
                        loading: false,
                        error: "Network error during purchase".to_string(),
                    });
                    listing_states.set(states);
                }
            }
        });
    })
}

pub fn handle_create_listing(
    token: String,
    listings: UseStateHandle<Vec<super::MarketListing>>,
    create_step: UseStateHandle<super::CreateListingStep>,
    price_input: UseStateHandle<String>,
    inventory: UseStateHandle<Vec<DisplayItem>>,
    error: UseStateHandle<String>,
    refresh_counter: UseStateHandle<i32>,
) -> Callback<DisplayItem> {
    Callback::from(move |item: DisplayItem| {
        let price = price_input.parse::<i32>().unwrap_or(0);
        if price <= 0 {
            return;
        }

        let token = token.clone();
        let listings = listings.clone();
        let create_step = create_step.clone();
        let error = error.clone();
        let inventory = inventory.clone();
        let price_input = price_input.clone();
        let refresh_counter = refresh_counter.clone();

        let (item_id, item_type) = match &item {
            DisplayItem::Egg(egg) => (egg.id, "egg"),
            DisplayItem::Creature(creature) => (creature.id, "creature"),
            DisplayItem::Scroll(scroll) => (scroll.id, "scroll"),
        };

        let request = CreateListingRequest {
            item_id,
            item_type: item_type.to_string(),
            price,
            quantity: 1,
        };

        spawn_local(async move {
            match Request::post(&format!("{}/api/market/listings", get_api_base_url()))
                .header("Authorization", &format!("Bearer {}", token))
                .json(&request)
                .unwrap()
                .send()
                .await
            {
                Ok(response) => {
                    log::info!("Got response with status: {}", response.status());
                    match response.status() {
                        200 => {
                            if let Ok(api_response) = response.json::<super::ApiResponse<super::MarketListing>>().await {
                                if api_response.success {
                                    if let Some(mut new_listing) = api_response.data {
                                        // Add the item to the new listing
                                        new_listing.item = Some(item.clone());
                                        
                                        // Update the listings by adding the new one
                                        let mut current_listings = (*listings).clone();
                                        current_listings.insert(0, new_listing); // Add to the beginning
                                        listings.set(current_listings);
                                        // On success, clear the modal and inputs
                                        create_step.set(super::CreateListingStep::Closed);
                                        inventory.set(Vec::new()); // Clear inventory as it's now outdated
                                        price_input.set(String::new());
                                        error.set(String::new());
                                        refresh_counter.set(*refresh_counter + 1); // Still trigger a refresh for consistency
                                    } else {
                                        // Missing listing data
                                        error.set("Failed to create listing: Missing listing data".to_string());
                                    }
                                } else {
                                    // API indicated failure, check error message and display custom message if needed
                                    if let Some(err_msg) = api_response.error {
                                        if err_msg.contains("Insufficient funds") {
                                            error.set("Not enough pax".to_string());
                                        } else {
                                            error.set(err_msg);
                                        }
                                    } else {
                                        error.set("Failed to create listing".to_string());
                                    }
                                }
                            } else {
                                error.set("Failed to parse response".to_string());
                            }
                        }
                        429 => {
                            error.set("Too many requests. Please try again later.".to_string());
                        }
                        _ => {
                            if let Ok(err_response) = response.json::<serde_json::Value>().await {
                                if let Some(err_msg) = err_response.get("error").and_then(|v| v.as_str()) {
                                    error.set(err_msg.to_string());
                                } else {
                                    error.set("Failed to create listing".to_string());
                                }
                            } else {
                                error.set("Failed to create listing".to_string());
                            }
                        }
                    }
                }
                Err(_) => {
                    error.set("Network error while creating listing".to_string());
                }
            }
        });
    })
}

pub fn handle_cancel(
    token: String,
    listings: UseStateHandle<Vec<super::MarketListing>>,
    error: UseStateHandle<String>,
    refresh_counter: UseStateHandle<i32>,
) -> Callback<Uuid> {
    Callback::from(move |listing_id: Uuid| {
        let token = token.clone();
        let listings = listings.clone();
        let error = error.clone();
        let refresh_counter = refresh_counter.clone();

        spawn_local(async move {
            match Request::delete(&format!("{}/api/market/listings/{}", get_api_base_url(), listing_id))
                .header("Authorization", &format!("Bearer {}", token))
                .send()
                .await
            {
                Ok(response) => {
                    match response.status() {
                        200 => {
                            // Parse the returned JSON and update currency and scroll count
                            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                                if let Some(data) = api_response.get("data") {
                                    if let Some(window) = web_sys::window() {
                                        if let Some(storage) = window.local_storage().ok().flatten() {
                                            // Update currency
                                            if let Some(balance) = data.get("currency_balance").and_then(|b| b.as_i64()) {
                                                let _ = storage.set_item("currency", &balance.to_string());
                                                let event_init = web_sys::CustomEventInit::new();
                                                event_init.set_detail(&wasm_bindgen::JsValue::from_f64(balance as f64));
                                                if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("currencyUpdate", &event_init) {
                                                    let _ = window.dispatch_event(&event);
                                                }
                                            }

                                            // Update scroll count
                                            if let Some(scroll_count) = data.get("scroll_count").and_then(|s| s.as_i64()) {
                                                let event_init = web_sys::CustomEventInit::new();
                                                event_init.set_detail(&wasm_bindgen::JsValue::from_f64(scroll_count as f64));
                                                if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("scrollUpdate", &event_init) {
                                                    let _ = window.dispatch_event(&event);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            listings.set(Vec::new()); // Clear listings to force refresh
                            error.set(String::new()); // Clear any existing error
                            refresh_counter.set(*refresh_counter + 1); // Increment refresh counter
                        }
                        429 => {
                            error.set("Too many requests. Please try again later.".to_string());
                        }
                        _ => {
                            if let Ok(err_response) = response.json::<serde_json::Value>().await {
                                if let Some(err_msg) = err_response.get("error").and_then(|v| v.as_str()) {
                                    error.set(err_msg.to_string());
                                } else {
                                    error.set("Failed to cancel listing".to_string());
                                }
                            } else {
                                error.set("Failed to cancel listing".to_string());
                            }
                        }
                    }
                }
                Err(_) => {
                    error.set("Network error while canceling listing".to_string());
                }
            }
        });
    })
}

pub fn handle_price_change(price_input: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |e: InputEvent| {
        if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
            price_input.set(input.value());
        }
    })
} 
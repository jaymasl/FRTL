use yew::prelude::*;
use gloo_net::http::Request;
use crate::components::displays::DisplayItem;
use futures::future::join_all;
use super::MarketListing;
use super::ApiResponse;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use wasm_bindgen::JsValue;
use log;
use std::future::Future;
use std::pin::Pin;
use crate::config::get_api_base_url;

thread_local! {
    static LAST_REFRESH: std::cell::RefCell<f64> = std::cell::RefCell::new(0.0);
}

fn get_current_time() -> f64 {
    if let Some(window) = window() {
        if let Some(performance) = window.performance() {
            return performance.now();
        }
    }
    0.0
}

pub async fn update_currency(token: &str) -> bool {
    let mut updated = false;
    if let Ok(response) = Request::get(&format!("{}/api/profile", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await 
    {
        if response.status() == 200 {
            if let Ok(profile) = response.json::<serde_json::Value>().await {
                if let Some(balance) = profile.get("currency_balance").and_then(|b| b.as_i64()) {
                    if let Some(window) = window() {
                        if let Some(storage) = window.local_storage().ok().flatten() {
                            // Check if the balance has changed
                            if let Ok(Some(current_balance)) = storage.get_item("currency") {
                                if let Ok(current_balance) = current_balance.parse::<i64>() {
                                    if current_balance != balance {
                                        updated = true;
                                        log::info!("Currency balance changed from {} to {}", current_balance, balance);
                                    }
                                }
                            }
                            
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
    }
    updated
}

pub async fn fetch_listing_item(listing_id: &str, token: &str) -> Option<DisplayItem> {
    match Request::get(&format!("{}/api/market/listings/{}/item", get_api_base_url(), listing_id))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) if response.status() == 200 => {
            if let Ok(api_response) = response.json::<ApiResponse<DisplayItem>>().await {
                if api_response.success {
                    return api_response.data;
                }
            }
        }
        _ => ()
    }
    None
}

async fn fetch_listings(
    token: &str,
    listings: &UseStateHandle<Vec<MarketListing>>,
    error: &UseStateHandle<String>,
    is_refreshing: &UseStateHandle<bool>,
) {
    if token.is_empty() {
        return;
    }

    is_refreshing.set(true);
    error.set(String::new()); // Clear any previous errors

    // Update currency first
    let _ = update_currency(token).await;

    match Request::get(&format!("{}/api/market/listings", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            match response.status() {
                200 => {
                    if let Ok(api_response) = response.json::<ApiResponse<Vec<MarketListing>>>().await {
                        if !api_response.success {
                            if let Some(err) = api_response.error {
                                error.set(err);
                                is_refreshing.set(false);
                                return;
                            }
                        }
                        if let Some(new_listings) = api_response.data {
                            let current_listings = (*listings).clone();
                            let mut futures: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();
                            
                            // First filter out non-active listings
                            let active_listings: Vec<_> = new_listings
                                .into_iter()
                                .filter(|listing| listing.status == "active")
                                .collect();
                            
                            let new_listings_vec = std::rc::Rc::new(std::cell::RefCell::new(active_listings));

                            // Check for removed listings
                            let new_ids: std::collections::HashSet<_> = new_listings_vec.borrow().iter()
                                .map(|listing| listing.id)
                                .collect();
                            
                            let removed_listings: Vec<_> = current_listings.iter()
                                .filter(|listing| !new_ids.contains(&listing.id))
                                .collect();

                            if !removed_listings.is_empty() {
                                log::info!("Detected {} removed/cancelled listings", removed_listings.len());
                            }

                            // Fetch items for all listings that don't have items
                            for i in 0..new_listings_vec.borrow().len() {
                                let listing = &new_listings_vec.borrow()[i];
                                if listing.item.is_none() {
                                    let token = token.to_string();
                                    let listing_id = listing.id.to_string();
                                    let listings_ref = new_listings_vec.clone();
                                    futures.push(Box::pin(async move {
                                        if let Some(item) = fetch_listing_item(&listing_id, &token).await {
                                            listings_ref.borrow_mut()[i].item = Some(item);
                                        }
                                    }));
                                }
                            }

                            // Wait for all item fetches to complete
                            join_all(futures).await;

                            // Update listings
                            let final_listings = new_listings_vec.borrow().to_vec();
                            let valid_listings: Vec<_> = final_listings
                                .into_iter()
                                .filter(|listing| listing.item.is_some())
                                .collect();

                            // Always update if we have new listings or if listings were removed
                            if !valid_listings.is_empty() || !removed_listings.is_empty() {
                                log::info!("Updating market listings with {} items (active only)", valid_listings.len());
                                listings.set(valid_listings);
                            }
                        }
                    }
                }
                429 => {
                    log::warn!("Rate limit hit, will retry in 3 seconds");
                    error.set("Rate limit reached. Please wait...".to_string());
                }
                _ => {
                    error.set("Failed to fetch listings".to_string());
                }
            }
        }
        Err(e) => {
            log::error!("Network error while refreshing market listings: {:?}", e);
            error.set("Network error".to_string());
        }
    }

    is_refreshing.set(false);
}

pub fn setup_refresh(
    token: String,
    listings: UseStateHandle<Vec<MarketListing>>,
    error: UseStateHandle<String>,
    is_refreshing: UseStateHandle<bool>,
) -> gloo_timers::callback::Interval {
    // Immediate initial fetch
    let token_clone = token.clone();
    let listings_clone = listings.clone();
    let error_clone = error.clone();
    let is_refreshing_clone = is_refreshing.clone();
    
    spawn_local(async move {
        fetch_listings(&token_clone, &listings_clone, &error_clone, &is_refreshing_clone).await;
    });

    let handle_refresh = {
        let listings = listings.clone();
        let token = token.clone();
        let error = error.clone();
        let is_refreshing = is_refreshing.clone();
        
        move || {
            let listings = listings.clone();
            let token = token.clone();
            let error = error.clone();
            let is_refreshing = is_refreshing.clone();
            
            // Check if enough time has passed since last refresh
            let should_refresh = LAST_REFRESH.with(|last| {
                let now = get_current_time();
                let elapsed = now - *last.borrow();
                if elapsed >= 3000.0 { // 3000ms = 3 seconds
                    *last.borrow_mut() = now;
                    true
                } else {
                    false
                }
            });

            if !should_refresh {
                return;
            }
            
            spawn_local(async move {
                fetch_listings(&token, &listings, &error, &is_refreshing).await;
            });
        }
    };

    // Set interval to 3 seconds
    let interval = gloo_timers::callback::Interval::new(3_000, handle_refresh.clone());
    interval
} 
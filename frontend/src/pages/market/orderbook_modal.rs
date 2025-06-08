use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use uuid::Uuid;
use std::cell::RefCell;
use web_sys::window;
use gloo_timers::callback::Interval;
use crate::config::get_api_base_url;
use base64::Engine;

thread_local! {
    static LAST_REFRESH: RefCell<f64> = RefCell::new(0.0);
}

fn get_current_time() -> f64 {
    if let Some(window) = window() {
        if let Some(performance) = window.performance() {
            return performance.now();
        }
    }
    0.0
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AggregatedOrder {
    pub side: String,
    pub price: i32,
    pub id: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScrollResponse {
    pub id: Uuid,
    pub display_name: String,
    pub quantity: i32,
}

#[derive(Properties, PartialEq)]
pub struct OrderbookModalProps {
    pub on_close: Callback<()>,
}

// Helper function to get token from storage
fn get_token() -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item("token").ok().flatten())
        .or_else(|| {
            web_sys::window()
                .and_then(|w| w.session_storage().ok().flatten())
                .and_then(|storage| storage.get_item("token").ok().flatten())
        })
        .unwrap_or_default()
}

// Helper function to get the current user's ID from storage or JWT token
fn get_current_user_id() -> Option<String> {
    // First try to get from local storage
    if let Some(user_id) = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item("user_id").ok().flatten()) {
        return Some(user_id);
    }
    
    // If not in local storage, try to extract from JWT token
    let token = get_token();
    if token.is_empty() {
        return None;
    }
    
    // JWT tokens are in the format: header.payload.signature
    // We need to extract the payload and decode it
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    
    // Decode the base64 payload
    let payload = parts[1];
    let decoded = base64_decode_url_safe(payload);
    
    // Parse the JSON payload
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&decoded) {
        // Extract the user ID from the "sub" claim
        if let Some(sub) = json.get("sub").and_then(|s| s.as_str()) {
            // Store in local storage for future use
            if let Some(window) = web_sys::window() {
                if let Some(storage) = window.local_storage().ok().flatten() {
                    let _ = storage.set_item("user_id", sub);
                }
            }
            return Some(sub.to_string());
        }
    }
    
    None
}

// Helper function to decode base64 URL-safe strings
fn base64_decode_url_safe(input: &str) -> String {
    // Add padding if needed
    let mut padded = input.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    
    // Replace URL-safe characters
    let standard_base64 = padded.replace('-', "+").replace('_', "/");
    
    // Decode using the standard engine
    match base64::engine::general_purpose::STANDARD.decode(&standard_base64) {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

async fn fetch_orders(token: &str) -> Result<Vec<AggregatedOrder>, String> {
    match Request::get(&format!("{}/api/scrolls/orders", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            match response.status() {
                200 => {
                    match response.json::<Vec<AggregatedOrder>>().await {
                        Ok(data) => Ok(data),
                        Err(e) => Err(format!("Failed to parse response: {:?}", e))
                    }
                },
                429 => Err("Rate limit reached".to_string()),
                status => Err(format!("Server error: {}", status))
            }
        },
        Err(e) => Err(format!("Network error: {:?}", e))
    }
}

async fn fulfill_order(token: &str, order_id: &str) -> Result<String, String> {
    match Request::post(&format!("{}/api/scrolls/orders/{}/fulfill", get_api_base_url(), order_id))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            match response.status() {
                200 => {
                    match response.json::<serde_json::Value>().await {
                        Ok(data) => {
                            if let Some(message) = data.get("message").and_then(|m| m.as_str()) {
                                Ok(message.to_string())
                            } else {
                                Ok("Order fulfilled successfully".to_string())
                            }
                        },
                        Err(e) => Err(format!("Failed to parse response: {:?}", e))
                    }
                },
                400 => {
                    match response.json::<serde_json::Value>().await {
                        Ok(data) => {
                            if let Some(error) = data.get("error").and_then(|e| e.as_str()) {
                                Err(error.to_string())
                            } else {
                                Err("Bad request".to_string())
                            }
                        },
                        Err(_) => Err("Bad request".to_string())
                    }
                },
                401 => Err("Unauthorized".to_string()),
                403 => Err("Forbidden".to_string()),
                404 => Err("Order not found".to_string()),
                429 => Err("Rate limit reached".to_string()),
                status => Err(format!("Server error: {}", status))
            }
        },
        Err(e) => Err(format!("Network error: {:?}", e))
    }
}

async fn refresh_scrolls(token: &str, scrolls: UseStateHandle<i32>) -> bool {
    match Request::get(&format!("{}/api/scrolls", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            if response.status() == 200 {
                if let Ok(data) = response.json::<Vec<ScrollResponse>>().await {
                    let total = data.iter().find(|s| s.display_name == "Summoning Scroll").map_or(0, |s| s.quantity);
                    scrolls.set(total);
                    if let Some(window) = web_sys::window() {
                        let event_init = web_sys::CustomEventInit::new();
                        event_init.set_detail(&JsValue::from_f64(total as f64));
                        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("scrollUpdate", &event_init) {
                            let _ = window.dispatch_event(&event);
                        }
                    }
                    return true;
                }
            }
            false
        },
        Err(_) => false
    }
}

async fn refresh_currency(token: &str, currency: UseStateHandle<i32>) -> bool {
    match Request::get(&format!("{}/api/profile", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            if response.status() == 200 {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(balance) = data.get("currency_balance").and_then(|v| v.as_i64()) {
                        currency.set(balance as i32);
                        if let Some(window) = web_sys::window() {
                            let event_init = web_sys::CustomEventInit::new();
                            event_init.set_detail(&JsValue::from_f64(balance as f64));
                            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("currencyUpdate", &event_init) {
                                let _ = window.dispatch_event(&event);
                            }
                        }
                        return true;
                    }
                }
            }
            false
        },
        Err(_) => false
    }
}

async fn cancel_order(token: &str, order_id: &str) -> Result<String, String> {
    web_sys::console::log_1(&format!("Cancelling order: {}", order_id).into());
    
    match Request::delete(&format!("{}/api/scrolls/orders/{}", get_api_base_url(), order_id))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            web_sys::console::log_1(&format!("Cancel response status: {}", response.status()).into());
            
            if response.status() == 200 {
                web_sys::console::log_1(&"Order cancelled successfully".into());
                Ok("Order cancelled successfully".to_string())
            } else {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                web_sys::console::log_1(&format!("Cancel error: {}", error_text).into());
                Err(format!("Failed to cancel order: {}", error_text))
            }
        },
        Err(e) => {
            web_sys::console::log_1(&format!("Cancel network error: {}", e).into());
            Err(format!("Network error: {}", e))
        },
    }
}

#[function_component(OrderbookModal)]
pub fn orderbook_modal(props: &OrderbookModalProps) -> Html {
    let orders = use_state(|| Vec::<AggregatedOrder>::new());
    let sell_price_input = use_state(|| "".to_string());
    let buy_price_input = use_state(|| "".to_string());
    let sell_error = use_state(|| "".to_string());
    let buy_error = use_state(|| "".to_string());
    let sell_loading = use_state(|| false);
    let buy_loading = use_state(|| false);
    let cancel_loading = use_state(|| Option::<String>::None);
    let fulfill_loading = use_state(|| Option::<String>::None);
    let fulfill_message = use_state(|| "".to_string());
    let is_refreshing = use_state(|| false);
    let error = use_state(String::new);
    let scrolls = use_state(|| 0);
    let currency = use_state(|| 0);
    let current_user_id = get_current_user_id();

    // Auto-hide notification after 3 seconds
    {
        let fulfill_message_for_effect = fulfill_message.clone();
        use_effect_with(
            (*fulfill_message_for_effect).clone(),
            move |message| {
                let cleanup = if !message.is_empty() {
                    let fulfill_message_clone = fulfill_message_for_effect.clone();
                    let timeout = gloo_timers::callback::Timeout::new(3000, move || {
                        fulfill_message_clone.set("".to_string());
                    });
                    let timeout_handle = timeout.forget();
                    Box::new(move || {
                        drop(timeout_handle);
                    }) as Box<dyn FnOnce()>
                } else {
                    Box::new(|| {}) as Box<dyn FnOnce()>
                };
                cleanup
            },
        );
    }

    // Auto-hide sell error after 3 seconds
    {
        let sell_error_for_effect = sell_error.clone();
        use_effect_with(
            (*sell_error_for_effect).clone(),
            move |error| {
                let cleanup = if !error.is_empty() {
                    let sell_error_clone = sell_error_for_effect.clone();
                    let timeout = gloo_timers::callback::Timeout::new(3000, move || {
                        sell_error_clone.set("".to_string());
                    });
                    let timeout_handle = timeout.forget();
                    Box::new(move || {
                        drop(timeout_handle);
                    }) as Box<dyn FnOnce()>
                } else {
                    Box::new(|| {}) as Box<dyn FnOnce()>
                };
                cleanup
            },
        );
    }

    // Auto-hide buy error after 3 seconds
    {
        let buy_error_for_effect = buy_error.clone();
        use_effect_with(
            (*buy_error_for_effect).clone(),
            move |error| {
                let cleanup = if !error.is_empty() {
                    let buy_error_clone = buy_error_for_effect.clone();
                    let timeout = gloo_timers::callback::Timeout::new(3000, move || {
                        buy_error_clone.set("".to_string());
                    });
                    let timeout_handle = timeout.forget();
                    Box::new(move || {
                        drop(timeout_handle);
                    }) as Box<dyn FnOnce()>
                } else {
                    Box::new(|| {}) as Box<dyn FnOnce()>
                };
                cleanup
            },
        );
    }

    // Insert refresh callbacks for auto-updating scrolls and currency
    let _refresh_scrolls_callback = {
        let scrolls = scrolls.clone();
        
        Callback::from(move |_: ()| {
            let scrolls = scrolls.clone();
            let token = get_token();
            
            spawn_local(async move {
                let _ = refresh_scrolls(&token, scrolls).await;
            });
        })
    };

    let _refresh_currency_callback = {
        let currency = currency.clone();
        
        Callback::from(move |_: ()| {
            let currency = currency.clone();
            let token = get_token();
            
            spawn_local(async move {
                let _ = refresh_currency(&token, currency).await;
            });
        })
    };

    // Get user's scroll count
    {
        let scrolls = scrolls.clone();
        let token = get_token();
        use_effect_with((), move |_| {
            if !token.is_empty() {
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/scrolls", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        if let Ok(data) = response.json::<Vec<ScrollResponse>>().await {
                            let summoning_scroll_count = data.iter()
                                .find(|scroll| scroll.display_name == "Summoning Scroll")
                                .map(|scroll| scroll.quantity)
                                .unwrap_or(0);
                            scrolls.set(summoning_scroll_count);
                        }
                    }
                });
            }
            || ()
        });
    }

    // Get user's currency balance
    {
        let currency = currency.clone();
        use_effect_with((), move |_| {
            if let Some(window) = web_sys::window() {
                if let Some(storage) = window.local_storage().ok().flatten() {
                    if let Ok(Some(balance)) = storage.get_item("currency") {
                        if let Ok(balance) = balance.parse::<i32>() {
                            currency.set(balance);
                        }
                    }
                }
                
                let currency_clone = currency.clone();
                let closure = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                    if let Some(new_balance) = e.detail().as_f64() {
                        currency_clone.set(new_balance as i32);
                    }
                }) as Box<dyn FnMut(_)>);
                
                window.add_event_listener_with_callback(
                    "currencyUpdate",
                    closure.as_ref().unchecked_ref()
                ).ok();
                
                closure.forget();
            }
            || ()
        });
    }

    // Right after the existing use_effect that listens for "currencyUpdate", add a new block for "scrollUpdate"
    {
        let scrolls = scrolls.clone();
        use_effect_with((), move |_| {
            if let Some(window) = web_sys::window() {
                let scrolls_clone = scrolls.clone();
                let closure = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                    if let Some(new_scroll) = e.detail().as_f64() {
                        scrolls_clone.set(new_scroll as i32);
                    }
                }) as Box<dyn FnMut(_)>);
                window.add_event_listener_with_callback("scrollUpdate", closure.as_ref().unchecked_ref()).ok();
                closure.forget();
            }
            || ()
        });
    }

    // Split orders into sell and buy lists
    let sell_orders: Vec<AggregatedOrder> = (*orders)
        .clone()
        .into_iter()
        .filter(|order| order.side == "sell")
        .collect();
    let buy_orders: Vec<AggregatedOrder> = (*orders)
        .clone()
        .into_iter()
        .filter(|order| order.side == "buy")
        .collect();

    // Sort orders
    let mut sorted_sell = sell_orders.clone();
    sorted_sell.sort_by_key(|order| order.price);

    let mut sorted_buy = buy_orders.clone();
    sorted_buy.sort_by(|a, b| b.price.cmp(&a.price));

    // Setup refresh mechanism
    {
        // Clone the state handles for the effect
        let orders_effect = orders.clone();
        let error_effect = error.clone();
        let is_refreshing_effect = is_refreshing.clone();
        let token_effect = get_token();

        use_effect_with((), move |_| -> Box<dyn FnOnce()> {
            if token_effect.is_empty() {
                return Box::new(|| ());
            }
            {
                // Initial fetch
                let orders_initial = orders_effect.clone();
                let error_initial = error_effect.clone();
                let is_refreshing_initial = is_refreshing_effect.clone();
                let token_initial = token_effect.clone();
                spawn_local(async move {
                    is_refreshing_initial.set(true);
                    match fetch_orders(&token_initial).await {
                        Ok(data) => {
                            orders_initial.set(data);
                            error_initial.set(String::new());
                        },
                        Err(e) => error_initial.set(e),
                    }
                    is_refreshing_initial.set(false);
                });
            }
            
            // Setup interval for periodic refresh
            let orders_interval = orders_effect.clone();
            let error_interval = error_effect.clone();
            let is_refreshing_interval = is_refreshing_effect.clone();
            let token_interval = token_effect.clone();
            let handle_refresh = move || {
                let should_refresh = LAST_REFRESH.with(|last| {
                    let now = get_current_time();
                    let elapsed = now - *last.borrow();
                    if elapsed >= 3000.0 {
                        *last.borrow_mut() = now;
                        true
                    } else {
                        false
                    }
                });
                if !should_refresh {
                    return;
                }
                let orders_inner = orders_interval.clone();
                let error_inner = error_interval.clone();
                let is_refreshing_inner = is_refreshing_interval.clone();
                let token_inner = token_interval.clone();
                spawn_local(async move {
                    is_refreshing_inner.set(true);
                    match fetch_orders(&token_inner).await {
                        Ok(data) => {
                            orders_inner.set(data);
                            error_inner.set(String::new());
                        },
                        Err(e) => error_inner.set(e),
                    }
                    is_refreshing_inner.set(false);
                });
            };
            let interval = Interval::new(3000, handle_refresh);
            Box::new(move || drop(interval))
        });
    }

    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    // Input handlers for sell form
    let on_sell_price_input = {
        let sell_price_input = sell_price_input.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                sell_price_input.set(input.value());
            }
        })
    };

    let on_buy_price_input = {
        let buy_price_input = buy_price_input.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                buy_price_input.set(input.value());
            }
        })
    };

    // Add a callback for fulfilling orders
    let on_fulfill_order = {
        let token = get_token();
        let orders = orders.clone();
        let fulfill_loading = fulfill_loading.clone();
        let fulfill_message = fulfill_message.clone();
        let currency = currency.clone();
        let scrolls = scrolls.clone();
        
        Callback::from(move |order_id: String| {
            let token = token.clone();
            let orders = orders.clone();
            let fulfill_loading = fulfill_loading.clone();
            let fulfill_message = fulfill_message.clone();
            let currency = currency.clone();
            let scrolls = scrolls.clone();
            
            fulfill_loading.set(Some(order_id.clone()));
            
            spawn_local(async move {
                match fulfill_order(&token, &order_id).await {
                    Ok(message) => {
                        fulfill_message.set(message);
                        let token_for_orders = token.clone();
                        spawn_local(async move {
                            let _ = refresh_orders(token_for_orders, orders).await;
                        });
                        let _ = refresh_scrolls(&token, scrolls).await;
                        let _ = refresh_currency(&token, currency).await;
                    },
                    Err(err) => {
                        fulfill_message.set(format!("Error: {}", err));
                    }
                }
                fulfill_loading.set(None);
            });
        })
    };

    // Add a callback for cancelling orders
    let on_cancel_order = {
        let token = get_token();
        let orders = orders.clone();
        let cancel_loading = cancel_loading.clone();
        let fulfill_message = fulfill_message.clone();
        let currency = currency.clone();
        let scrolls = scrolls.clone();
        
        Callback::from(move |order_id: String| {
            web_sys::console::log_1(&format!("Cancel button clicked for order: {}", order_id).into());
            
            let token = token.clone();
            let orders = orders.clone();
            let cancel_loading = cancel_loading.clone();
            let fulfill_message = fulfill_message.clone();
            let currency = currency.clone();
            let scrolls = scrolls.clone();
            
            cancel_loading.set(Some(order_id.clone()));
            
            spawn_local(async move {
                web_sys::console::log_1(&format!("Calling cancel_order for order: {}", order_id).into());
                
                match cancel_order(&token, &order_id).await {
                    Ok(message) => {
                        web_sys::console::log_1(&format!("Cancel success: {}", message).into());
                        fulfill_message.set(message);
                        
                        web_sys::console::log_1(&"Refreshing orders after cancel".into());
                        let token_for_orders = token.clone();
                        spawn_local(async move {
                            let _ = refresh_orders(token_for_orders, orders).await;
                        });
                        
                        web_sys::console::log_1(&"Refreshing scrolls after cancel".into());
                        let _ = refresh_scrolls(&token, scrolls).await;
                        
                        web_sys::console::log_1(&"Refreshing currency after cancel".into());
                        let _ = refresh_currency(&token, currency).await;
                    },
                    Err(err) => {
                        web_sys::console::log_1(&format!("Cancel error: {}", err).into());
                        fulfill_message.set(format!("Error: {}", err));
                    }
                }
                cancel_loading.set(None);
            });
        })
    };

    // Update the sell order form
    let on_sell_order = {
        let token = get_token();
        let sell_price_input = sell_price_input.clone();
        let sell_error = sell_error.clone();
        let sell_loading = sell_loading.clone();
        let orders = orders.clone();
        let scrolls = scrolls.clone();
        let currency = currency.clone();
        
        Callback::from(move |_| {
            let token = token.clone();
            let sell_price_input = sell_price_input.clone();
            let sell_error = sell_error.clone();
            let sell_loading = sell_loading.clone();
            let orders = orders.clone();
            let scrolls = scrolls.clone();
            let currency = currency.clone();
            
            let price = match sell_price_input.parse::<i32>() {
                Ok(p) if p > 0 => p,
                _ => {
                    sell_error.set("Invalid price".to_string());
                    return;
                }
            };
            
            sell_loading.set(true);
            sell_error.set("".to_string());
            
            let payload = CreateOrderPayload {
                side: "sell".to_string(),
                price,
            };
            
            spawn_local(async move {
                match Request::post(&format!("{}/api/scrolls/orders", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&payload)
                    .expect("Failed to serialize payload")
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.status() {
                            200 | 201 => {
                                sell_price_input.set("".to_string());
                                let token_for_orders = token.clone();
                                spawn_local(async move {
                                    let _ = refresh_orders(token_for_orders, orders).await;
                                });
                                let _ = refresh_scrolls(&token, scrolls).await;
                                let _ = refresh_currency(&token, currency).await;
                            },
                            400 => {
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    if let Some(error) = data.get("error").and_then(|e| e.as_str()) {
                                        sell_error.set(error.to_string());
                                    } else {
                                        sell_error.set("Bad request".to_string());
                                    }
                                } else {
                                    sell_error.set("Bad request".to_string());
                                }
                            },
                            401 => sell_error.set("Unauthorized".to_string()),
                            429 => sell_error.set("Rate limit reached".to_string()),
                            status => sell_error.set(format!("Server error: {}", status)),
                        }
                    },
                    Err(e) => sell_error.set(format!("Network error: {:?}", e)),
                }
                sell_loading.set(false);
            });
        })
    };

    // Update the buy order form
    let on_buy_order = {
        let token = get_token();
        let buy_price_input = buy_price_input.clone();
        let buy_error = buy_error.clone();
        let buy_loading = buy_loading.clone();
        let orders = orders.clone();
        let currency = currency.clone();
        
        Callback::from(move |_| {
            let token = token.clone();
            let buy_price_input = buy_price_input.clone();
            let buy_error = buy_error.clone();
            let buy_loading = buy_loading.clone();
            let orders = orders.clone();
            let currency = currency.clone();
            
            let price = match buy_price_input.parse::<i32>() {
                Ok(p) if p > 0 => p,
                _ => {
                    buy_error.set("Invalid price".to_string());
                    return;
                }
            };
            
            buy_loading.set(true);
            buy_error.set("".to_string());
            
            let payload = CreateOrderPayload {
                side: "buy".to_string(),
                price,
            };
            
            spawn_local(async move {
                match Request::post(&format!("{}/api/scrolls/orders", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&payload)
                    .expect("Failed to serialize payload")
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.status() {
                            200 | 201 => {
                                buy_price_input.set("".to_string());
                                let token_for_orders = token.clone();
                                spawn_local(async move {
                                    let _ = refresh_orders(token_for_orders, orders).await;
                                });
                                let _ = refresh_currency(&token, currency).await;
                            },
                            400 => {
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    if let Some(error) = data.get("error").and_then(|e| e.as_str()) {
                                        buy_error.set(error.to_string());
                                    } else {
                                        buy_error.set("Bad request".to_string());
                                    }
                                } else {
                                    buy_error.set("Bad request".to_string());
                                }
                            },
                            401 => buy_error.set("Unauthorized".to_string()),
                            429 => buy_error.set("Rate limit reached".to_string()),
                            status => buy_error.set(format!("Server error: {}", status)),
                        }
                    },
                    Err(e) => buy_error.set(format!("Network error: {:?}", e)),
                }
                buy_loading.set(false);
            });
        })
    };

    html! {
        <div 
            class="fixed inset-0 bg-white/90 dark:bg-black/90 backdrop-blur-md flex items-center justify-center z-50 p-4 overflow-y-auto"
            onclick={on_close.clone()}
        >
            <div 
                class="bg-gray-100 dark:bg-gray-900 rounded-2xl shadow-xl max-w-5xl w-full max-h-[90vh] overflow-y-auto"
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
            >
                <div class="p-6">
                    <div class="flex justify-between items-center mb-6">
                        <div class="flex items-center">
                            <h2 class="text-2xl font-bold text-gray-900 dark:text-white">{"Scroll Orderbook"}</h2>
                            <span class="ml-3 text-xs text-gray-500 dark:text-gray-400">{"5 pax fee per order (non-refundable)"}</span>
                        </div>
                        <button 
                            onclick={on_close} 
                            class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>

                    if !(*fulfill_message).is_empty() {
                        <div class="mb-4 p-3 bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 rounded-lg">
                            {(*fulfill_message).clone()}
                        </div>
                    }

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div>
                            <div class="mb-6 p-4 border dark:border-gray-700 rounded-xl bg-gray-50 dark:bg-gray-700/50">
                                <h3 class="text-xl font-semibold text-red-600 dark:text-red-400 mb-3">{"Create Sell Order"}
                                    <span class="text-lg font-bold text-white ml-2">{format!("(Scrolls: {})", *scrolls)}</span>
                                </h3>
                                <div class="flex items-end gap-3">
                                    <div class="flex flex-col">
                                        <span class="text-xs text-gray-300 mb-1">{"1 scroll for"}</span>
                                        <input 
                                            type="number" 
                                            placeholder="(price)" 
                                            class="w-32 p-2 bg-white dark:bg-gray-800 border dark:border-gray-600 rounded-lg text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 text-sm [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                                            value={(*sell_price_input).clone()}
                                            oninput={on_sell_price_input.clone()} 
                                        />
                                    </div>
                                    <span class="text-sm text-gray-300 dark:text-gray-400 mb-2">{"pax"}</span>
                                    <button 
                                        onclick={on_sell_order.clone()} 
                                        class="px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:opacity-50 transition-colors duration-200" 
                                        disabled={*sell_loading || *scrolls == 0}
                                    >
                                        { if *sell_loading { "Creating..." } else { "Sell" } }
                                    </button>
                                </div>
                                if !(*sell_error).is_empty() {
                                    <p class="mt-2 text-red-500 dark:text-red-400">{(*sell_error).clone()}</p>
                                }
                            </div>
                            <h3 class="text-xl font-semibold text-red-600 dark:text-red-400 mb-3">{"Sell Orders"}</h3>
                            if !sorted_sell.is_empty() {
                                <div class="bg-white dark:bg-gray-800 rounded-xl p-4">
                                    <div class="grid grid-cols-[1fr,auto,auto] gap-4">
                                        <div class="font-medium text-gray-600 dark:text-gray-400 text-sm">{"Price"}</div>
                                        <div class="font-medium text-gray-600 dark:text-gray-400 text-sm">{"Seller"}</div>
                                        <div class="font-medium text-gray-600 dark:text-gray-400 text-sm text-right">{"Action"}</div>
                                        { for sorted_sell.iter().map(|order| {
                                            let order_id = order.id.clone();
                                            let is_fulfill_loading = fulfill_loading.clone();
                                            let is_cancel_loading = cancel_loading.clone();
                                            let is_fulfill_this_loading = (*is_fulfill_loading).as_ref().map_or(false, |id| id == &order_id);
                                            let is_cancel_this_loading = (*is_cancel_loading).as_ref().map_or(false, |id| id == &order_id);
                                            let is_own_order = current_user_id.as_ref().map_or(false, |id| id == &order.user_id);
                                            
                                            html! {
                                                <>
                                                    <div class="text-gray-900 dark:text-gray-100 font-medium">{ order.price }</div>
                                                    <div class="text-gray-600 dark:text-gray-400">{ &order.username }</div>
                                                    <div class="text-right">
                                                        {
                                                            if is_own_order {
                                                                html! {
                                                                    <button 
                                                                        onclick={
                                                                            let callback = on_cancel_order.clone();
                                                                            let order_id = order.id.clone();
                                                                            Callback::from(move |_| callback.emit(order_id.clone()))
                                                                        }
                                                                        class="px-2 py-1 bg-yellow-500 text-white text-xs rounded hover:bg-yellow-600 disabled:opacity-50 transition-colors duration-200"
                                                                        disabled={is_cancel_this_loading}
                                                                    >
                                                                        { if is_cancel_this_loading { "Cancelling..." } else { "Cancel" } }
                                                                    </button>
                                                                }
                                                            } else {
                                                                html! {
                                                                    <button 
                                                                        onclick={
                                                                            let callback = on_fulfill_order.clone();
                                                                            let order_id = order.id.clone();
                                                                            Callback::from(move |_| callback.emit(order_id.clone()))
                                                                        }
                                                                        class="px-2 py-1 bg-green-500 text-white text-xs rounded hover:bg-green-600 disabled:opacity-50 transition-colors duration-200"
                                                                        disabled={is_fulfill_this_loading || *currency < order.price}
                                                                    >
                                                                        { if is_fulfill_this_loading { "Buying..." } else { "Buy" } }
                                                                    </button>
                                                                }
                                                            }
                                                        }
                                                    </div>
                                                </>
                                            }
                                        }) }
                                    </div>
                                </div>
                            } else {
                                <p class="text-gray-500 dark:text-gray-400 bg-white dark:bg-gray-800 p-4 rounded-xl">{"No sell orders available"}</p>
                            }
                        </div>
                        <div>
                            <div class="mb-6 p-4 border dark:border-gray-700 rounded-xl bg-gray-50 dark:bg-gray-700/50">
                                <h3 class="text-xl font-semibold text-green-600 dark:text-green-400 mb-3">{"Create Buy Order"}
                                    <span class="text-lg font-bold text-white ml-2">{format!("(Balance: {} pax)", *currency)}</span>
                                </h3>
                                <div class="flex items-end gap-3">
                                    <div class="flex flex-col">
                                        <span class="text-xs text-gray-300 mb-1">{"1 scroll for"}</span>
                                        <input 
                                            type="number" 
                                            placeholder="(price)" 
                                            class="w-32 p-2 bg-white dark:bg-gray-800 border dark:border-gray-600 rounded-lg text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 text-sm [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                                            value={(*buy_price_input).clone()}
                                            oninput={on_buy_price_input.clone()} 
                                        />
                                    </div>
                                    <span class="text-sm text-gray-300 dark:text-gray-400 mb-2">{"pax"}</span>
                                    <button 
                                        onclick={on_buy_order.clone()} 
                                        class="px-4 py-2 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 transition-colors duration-200" 
                                        disabled={*buy_loading}
                                    >
                                        { if *buy_loading { "Creating..." } else { "Buy" } }
                                    </button>
                                </div>
                                if !(*buy_error).is_empty() {
                                    <p class="mt-2 text-red-500 dark:text-red-400">{(*buy_error).clone()}</p>
                                }
                            </div>
                            <h3 class="text-xl font-semibold text-green-600 dark:text-green-400 mb-3">{"Buy Orders"}</h3>
                            if !sorted_buy.is_empty() {
                                <div class="bg-white dark:bg-gray-800 rounded-xl p-4">
                                    <div class="grid grid-cols-[1fr,auto,auto] gap-4">
                                        <div class="font-medium text-gray-600 dark:text-gray-400 text-sm">{"Price"}</div>
                                        <div class="font-medium text-gray-600 dark:text-gray-400 text-sm">{"Buyer"}</div>
                                        <div class="font-medium text-gray-600 dark:text-gray-400 text-sm text-right">{"Action"}</div>
                                        { for sorted_buy.iter().map(|order| {
                                            let order_id = order.id.clone();
                                            let is_fulfill_loading = fulfill_loading.clone();
                                            let is_cancel_loading = cancel_loading.clone();
                                            let is_fulfill_this_loading = (*is_fulfill_loading).as_ref().map_or(false, |id| id == &order_id);
                                            let is_cancel_this_loading = (*is_cancel_loading).as_ref().map_or(false, |id| id == &order_id);
                                            let is_own_order = current_user_id.as_ref().map_or(false, |id| id == &order.user_id);
                                            
                                            html! {
                                                <>
                                                    <div class="text-gray-900 dark:text-gray-100 font-medium">{ order.price }</div>
                                                    <div class="text-gray-600 dark:text-gray-400">{ &order.username }</div>
                                                    <div class="text-right">
                                                        {
                                                            if is_own_order {
                                                                html! {
                                                                    <button 
                                                                        onclick={
                                                                            let callback = on_cancel_order.clone();
                                                                            let order_id = order.id.clone();
                                                                            Callback::from(move |_| callback.emit(order_id.clone()))
                                                                        }
                                                                        class="px-2 py-1 bg-yellow-500 text-white text-xs rounded hover:bg-yellow-600 disabled:opacity-50 transition-colors duration-200"
                                                                        disabled={is_cancel_this_loading}
                                                                    >
                                                                        { if is_cancel_this_loading { "Cancelling..." } else { "Cancel" } }
                                                                    </button>
                                                                }
                                                            } else {
                                                                html! {
                                                                    <button 
                                                                        onclick={
                                                                            let callback = on_fulfill_order.clone();
                                                                            let order_id = order.id.clone();
                                                                            Callback::from(move |_| callback.emit(order_id.clone()))
                                                                        }
                                                                        class="px-2 py-1 bg-red-500 text-white text-xs rounded hover:bg-red-600 disabled:opacity-50 transition-colors duration-200"
                                                                        disabled={is_fulfill_this_loading || *scrolls == 0}
                                                                    >
                                                                        { if is_fulfill_this_loading { "Selling..." } else { "Sell" } }
                                                                    </button>
                                                                }
                                                            }
                                                        }
                                                    </div>
                                                </>
                                            }
                                        }) }
                                    </div>
                                </div>
                            } else {
                                <p class="text-gray-500 dark:text-gray-400 bg-white dark:bg-gray-800 p-4 rounded-xl">{"No buy orders available"}</p>
                            }
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[derive(serde::Serialize)]
struct CreateOrderPayload {
    side: String,
    price: i32,
}

async fn refresh_orders(token: String, orders: UseStateHandle<Vec<AggregatedOrder>>) {
    web_sys::console::log_1(&"Refreshing orders...".into());
    
    match Request::get(&format!("{}/api/scrolls/orders", get_api_base_url()))
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => {
            web_sys::console::log_1(&format!("Orders response status: {}", response.status()).into());
            
            match response.json::<Vec<AggregatedOrder>>().await {
                Ok(data) => {
                    web_sys::console::log_1(&format!("Received {} orders", data.len()).into());
                    orders.set(data);
                },
                Err(e) => {
                    web_sys::console::log_1(&format!("Failed to parse orders: {}", e).into());
                }
            }
        },
        Err(e) => {
            web_sys::console::log_1(&format!("Failed to fetch orders: {}", e).into());
        }
    }
} 
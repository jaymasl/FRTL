use wasm_bindgen::prelude::*;
use web_sys::{window, CustomEvent};
use yew::prelude::*;
use wasm_bindgen::JsCast;

#[hook]
pub fn use_currency() -> UseStateHandle<i32> {
    let current_balance = use_state(|| {
        window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("currency").ok().flatten())
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    });

    {
        let current_balance = current_balance.clone();
        use_effect(move || {
            let cb = current_balance.clone();
            
            // Create event listener for currency updates
            let listener = Closure::wrap(Box::new(move |e: CustomEvent| {
                let detail = e.detail();
                
                // Handle the case where detail is a number (new total balance)
                if let Some(new_total) = detail.as_f64() {
                    cb.set(new_total as i32);
                    
                    // Update localStorage with the new balance
                    if let Some(w) = window() {
                        if let Ok(Some(storage)) = w.local_storage() {
                            let _ = storage.set_item("currency", &new_total.to_string());
                        }
                    }
                }
                // Handle the case where detail is an object with an 'amount' property
                else if detail.is_object() {
                    if let Some(amount) = js_sys::Reflect::get(&detail, &JsValue::from_str("amount"))
                        .ok()
                        .and_then(|v| v.as_f64()) 
                    {
                        let new_balance = *cb + amount as i32;
                        cb.set(new_balance);
                        
                        // Update localStorage with the new balance
                        if let Some(w) = window() {
                            if let Ok(Some(storage)) = w.local_storage() {
                                let _ = storage.set_item("currency", &new_balance.to_string());
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(CustomEvent)>);
            
            // Add event listener
            if let Some(window) = window() {
                let _ = window.add_event_listener_with_callback(
                    "currencyUpdate",
                    listener.as_ref().unchecked_ref()
                );
            }

            // Store listener to keep it alive during component lifetime
            let cleanup_listener = listener;
            
            // Return cleanup function
            move || {
                if let Some(window) = window() {
                    let _ = window.remove_event_listener_with_callback(
                        "currencyUpdate",
                        cleanup_listener.as_ref().unchecked_ref()
                    );
                }
            }
        });
    }

    current_balance
} 
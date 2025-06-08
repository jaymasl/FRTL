use yew::prelude::*;
use gloo::net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlIFrameElement};
use wasm_bindgen::JsCast;
use serde::{Deserialize};
use crate::config::get_api_base_url;
use crate::styles;
use gloo_timers::callback::Timeout;
use wasm_bindgen::JsValue;
use gloo_utils::format::JsValueSerdeExt;

// Import the leaderboard component properly instead of declaring it as a local module
use crate::pages::games::hexort_leaderboard::HexortLeaderboard;

// Helper function to get authentication token
fn get_auth_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
}

// Cooldown response structure
#[derive(Deserialize, Debug, Clone)]
struct CooldownResponse {
    cooldown_seconds: i64,
}

// Simple function to generate hex string from a message (not a real hash, but just for demonstration purposes)
fn generate_session_signature(message: &str) -> String {
    let mut result = String::new();
    for (i, c) in message.chars().enumerate() {
        result.push_str(&format!("{:02x}", c as u8 ^ (i as u8)));
    }
    result
}

// Component for the Hexort game
#[function_component(FrontendHexortGame)]
pub fn frontend_hexort_game() -> Html {
    let iframe_ref = use_node_ref();
    let session_info_state = use_state(|| None::<(String, String)>);
    let error_message = use_state(String::new);
    let cooldown_seconds = use_state(|| 0i64);
    let is_on_cooldown = use_state(|| false);
    let cooldown_timer = use_state(|| None::<gloo_timers::callback::Interval>);
    let leaderboard_update_trigger = use_state(|| 0u32);
    
    // Callback to fetch session info (replaces initialize_game_session)
    let fetch_session_info = {
        let session_info_state = session_info_state.clone();
        let error_message = error_message.clone();
        let iframe_ref = iframe_ref.clone();
        
        Callback::from(move |_| {
            let session_info_state = session_info_state.clone();
            let error_message = error_message.clone();
            let iframe_ref_clone = iframe_ref.clone();
            
            session_info_state.set(None); // Clear previous session info before fetch
            
            spawn_local(async move {
                match get_auth_token() {
                    Some(token) => {
                        let api_base = get_api_base_url();
                        let url = if api_base.is_empty() {
                            "/hexort/new".to_string()
                        } else {
                            format!("{}/hexort/new", api_base)
                        };
                        
                        match Request::post(&url)
                            .header("Authorization", &format!("Bearer {}", token))
                            .body(String::new())
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(resp) => {
                                if resp.status() == 200 {
                                    match resp.text().await {
                                        Ok(session_text) => {
                                            if !session_text.is_empty() {
                                                 // Generate signature based on the *received* session token string
                                                let sig = generate_session_signature(&format!("session:{}", &session_text));
                                                web_sys::console::log_1(&format!("Session info fetched successfully. ID: {}, SIG: {}", session_text, sig).into()); // DEBUG LOG
                                                session_info_state.set(Some((session_text.clone(), sig.clone())));
                                                error_message.set(String::new()); // Clear error on success

                                                // --- Proactively send session info --- START
                                                let id_clone = session_text.clone();
                                                let sig_clone = sig.clone();
                                                let iframe_ref_for_send = iframe_ref_clone.clone(); // Clone for the timeout closure

                                                // Update state FIRST
                                                session_info_state.set(Some((session_text.clone(), sig)));
                                                error_message.set(String::new()); // Clear error on success

                                                // Give the iframe a moment to load and set up listener before sending
                                                Timeout::new(150, move || { // Slightly increased delay
                                                    if let Some(iframe) = iframe_ref_for_send.cast::<HtmlIFrameElement>() {
                                                        if let Some(win) = iframe.content_window() {
                                                            // Use serde_json::json! for creating the message object
                                                            let message = serde_json::json!({
                                                                "type": "session_info",
                                                                "session_id": id_clone,
                                                                "session_signature": sig_clone
                                                            });
                                                            // Convert serde_json::Value to JsValue
                                                            match JsValue::from_serde(&message) {
                                                                Ok(js_msg) => {
                                                                    match win.post_message(&js_msg, "*") { // Restrict origin in production
                                                                        Ok(_) => web_sys::console::log_1(&"Proactively sent session_info to iframe.".into()),
                                                                        Err(e) => web_sys::console::error_1(&format!("Error posting session_info: {:?}", e).into()),
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    web_sys::console::error_1(&format!("Failed to serialize session_info message: {}", e).into());
                                                                }
                                                            }
                                                        } else {
                                                            web_sys::console::warn_1(&"Could not get iframe content window to send session_info.".into());
                                                        }
                                                    } else {
                                                        web_sys::console::warn_1(&"Could not get iframe element ref to send session_info.".into());
                                                    }
                                                }).forget();
                                                // --- Proactively send session info --- END

                                            } else {
                                                 error_message.set("Received empty session token from server.".to_string());
                                                 session_info_state.set(None); // Explicitly set None on error
                                            }
                                        },
                                        Err(e) => {
                                            error_message.set(format!("Failed to parse session response: {}", e));
                                            session_info_state.set(None);
                                        }
                                    }
                                } else if resp.status() == 429 {
                                    error_message.set("Rate limited: You need to wait before playing again.".to_string());
                                    session_info_state.set(None);
                                } else {
                                    let status = resp.status();
                                    let err_text = resp.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
                                    error_message.set(format!("Server error fetching session ({}): {}", status, err_text));
                                    session_info_state.set(None);
                                }
                            },
                            Err(e) => {
                                error_message.set(format!("Network error fetching session: {}", e));
                                session_info_state.set(None);
                            }
                        }
                    },
                    None => {
                        error_message.set("Authentication required to fetch session.".to_string());
                        session_info_state.set(None);
                    }
                }
            });
        })
    };

    // Check cooldown status and fetch session info if not on cooldown
    {
        let is_on_cooldown = is_on_cooldown.clone();
        let cooldown_seconds = cooldown_seconds.clone();
        let cooldown_timer = cooldown_timer.clone();
        let error_message = error_message.clone();
        let fetch_session_info = fetch_session_info.clone(); // Use the fetch callback
        let session_info_state = session_info_state.clone(); // Need state to check if session already fetched
        
        use_effect_with((), move |_| {
            let cooldown_timer_clone = cooldown_timer.clone();
            // Fetch is needed if session is None AND user is not currently known to be on cooldown
            let fetch_needed = session_info_state.is_none() && !*is_on_cooldown;

            if fetch_needed {
                 web_sys::console::log_1(&"use_effect: Fetching initial cooldown and potentially session info...".into());
                 spawn_local(async move {
                    match get_auth_token() {
                        Some(token) => {
                            let api_base = get_api_base_url();
                            let url = if api_base.is_empty() {
                                "/hexort/cooldown".to_string()
                            } else {
                                format!("{}/hexort/cooldown", api_base)
                            };
                            
                            match Request::get(&url)
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await
                            {
                                Ok(resp) => {
                                    if resp.status() == 200 {
                                        match resp.json::<CooldownResponse>().await {
                                            Ok(cooldown_resp) => {
                                                let cd = cooldown_resp.cooldown_seconds;
                                                cooldown_seconds.set(cd);
                                                
                                                if cd > 0 {
                                                    is_on_cooldown.set(true);
                                                    web_sys::console::log_1(&format!("User is on cooldown for {}s", cd).into());
                                                    
                                                    // Setup countdown timer (no need to fetch session if on cooldown)
                                                    let cs = cooldown_seconds.clone();
                                                    let is_cd = is_on_cooldown.clone();
                                                    let fetch_sess_cb = fetch_session_info.clone(); // Need callback when timer ends
                                                    let session_state = session_info_state.clone(); // Clone for the interval

                                                    let interval = gloo_timers::callback::Interval::new(1000, move || {
                                                        let current = *cs;
                                                        if current <= 1 {
                                                            is_cd.set(false);
                                                            cs.set(0);
                                                            
                                                            // Only fetch a new session if we don't already have one
                                                            if session_state.is_none() {
                                                                web_sys::console::log_1(&"Cooldown finished, triggering session fetch.".into());
                                                                fetch_sess_cb.emit(()); // Fetch session now
                                                            } else {
                                                                web_sys::console::log_1(&"Cooldown finished, but session already exists. No need to fetch.".into());
                                                            }
                                                        } else {
                                                            cs.set(current - 1);
                                                        }
                                                    });
                                                    cooldown_timer.set(Some(interval));
                                                } else {
                                                    // Not on cooldown from server's perspective
                                                    is_on_cooldown.set(false);
                                                    // Check again if session is None before fetching, in case it was fetched by another trigger
                                                    if session_info_state.is_none() {
                                                        web_sys::console::log_1(&"User not on cooldown, fetching session info...".into());
                                                        fetch_session_info.emit(());
                                                    } else {
                                                        web_sys::console::log_1(&"User not on cooldown, but session info already exists. Skipping fetch.".into());
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                error_message.set(format!("Failed to parse cooldown: {}", e));
                                                is_on_cooldown.set(false);
                                                if session_info_state.is_none() { fetch_session_info.emit(()); } // Try fetching session anyway if needed
                                            }
                                        }
                                    } else {
                                        error_message.set(format!("Server error getting cooldown: {}", resp.status()));
                                        is_on_cooldown.set(false);
                                        if session_info_state.is_none() { fetch_session_info.emit(()); } // Try fetching session anyway if needed
                                    }
                                },
                                Err(e) => {
                                    error_message.set(format!("Network error getting cooldown: {}", e));
                                    is_on_cooldown.set(false);
                                    if session_info_state.is_none() { fetch_session_info.emit(()); } // Try fetching session anyway if needed
                                }
                            }
                        },
                        None => {
                            error_message.set("Authentication required for cooldown check.".to_string());
                        }
                    }
                });
            } else {
                 web_sys::console::log_1(&"use_effect: Skipping cooldown/session fetch (already have session or known to be on cooldown).".into());
            }
            
            // Cleanup the timer
            move || {
                cooldown_timer_clone.set(None);
            }
        });
    }

    // Handle message event from iframe
    {
        let session_info_state = session_info_state.clone();
        let iframe_ref_clone = iframe_ref.clone();
        // Clone the trigger handle *before* the move closure
        let leaderboard_update_trigger_clone = leaderboard_update_trigger.clone(); 
        
        use_effect_with((), move |_| {
            let window = web_sys::window().expect("no global `window` exists");
            
            let session_info_state = session_info_state.clone();
            let iframe_ref_for_closure = iframe_ref_clone.clone();
            // Use the pre-cloned handle inside the closure
            let leaderboard_update_trigger_for_closure = leaderboard_update_trigger_clone.clone();
            
            let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
                 // Origin check recommended here in production

                if let Ok(data) = event.data().dyn_into::<js_sys::Object>() {
                    if let Some(type_prop_val) = js_sys::Reflect::get(&data, &"type".into()).ok() {
                        if let Some(message_type) = type_prop_val.as_string() {
                            
                            match message_type.as_str() {
                                "REQUEST_SESSION_INFO" => {
                                    web_sys::console::log_1(&"Received 'REQUEST_SESSION_INFO' message from iframe".into());
                                    // Respond ONLY if session info is available in state
                                    match &*session_info_state { // Use match for clarity and debugging
                                        Some((id, sig)) => {
                                            if !id.is_empty() && !sig.is_empty() {
                                                 web_sys::console::log_1(&format!("State has session info. Sending ID: {}, SIG: {}", id, sig).into()); // DEBUG LOG
                                                 if let Some(iframe) = iframe_ref_for_closure.cast::<HtmlIFrameElement>() {
                                                     if let Some(win) = iframe.content_window() {
                                                         let message = serde_json::json!({
                                                             "type": "session_info",
                                                             "session_id": id,
                                                             "session_signature": sig
                                                         });
                                                         match JsValue::from_serde(&message) { // Use the imported trait
                                                            Ok(js_msg) => {
                                                                let _ = win.post_message(
                                                                    &js_msg,
                                                                    "*" // Target origin - Consider restricting in production
                                                                );
                                                                web_sys::console::log_1(&"Sent 'session_info' back to iframe".into());
                                                             }
                                                             Err(e) => {
                                                                web_sys::console::error_1(&format!("Failed to serialize session_info message: {}", e).into());
                                                             }
                                                          }
                                                     }
                                                 }
                                             } else {
                                                  web_sys::console::warn_1(&"State has session info, but ID or SIG is empty.".into()); // DEBUG LOG
                                             }
                                        }
                                        None => {
                                             web_sys::console::warn_1(&"State is None when processing REQUEST_SESSION_INFO.".into()); // DEBUG LOG
                                             // Optionally, send back an error or empty response?
                                             // Consider if iframe should retry after a delay if it gets no response.
                                        }
                                    }
                                },
                                "GAME_SCORE_UPDATE" => {
                                    web_sys::console::log_1(&"Received 'GAME_SCORE_UPDATE' message from iframe".into());
                                    if let Some(score_val) = js_sys::Reflect::get(&data, &"score".into()).ok() {
                                        if let Some(score) = score_val.as_f64() {
                                            web_sys::console::log_1(&format!("Received score: {}", score).into());
                                            
                                            // Extract session_id from message data with enhanced logging
                                            let session_id = match js_sys::Reflect::get(&data, &"session_id".into()).ok() {
                                                Some(v) => {
                                                    if let Some(s) = v.as_string() {
                                                        // Log and validate the session ID format
                                                        if s.contains(':') {
                                                            web_sys::console::log_1(&format!("✅ Received valid session ID with colon: {}", s).into());
                                                            s
                                                        } else if s == "unknown" {
                                                            web_sys::console::error_1(&"⚠️ Received 'unknown' session ID from iframe".into());
                                                            s
                                                        } else {
                                                            web_sys::console::error_1(&format!("⚠️ Received malformed session ID without colon: {}", s).into());
                                                            s
                                                        }
                                                    } else {
                                                        web_sys::console::error_1(&"❌ session_id is not a string".into());
                                                        "unknown".to_string()
                                                    }
                                                },
                                                None => {
                                                    web_sys::console::error_1(&"❌ session_id field missing from message".into());
                                                    "unknown".to_string()
                                                }
                                            };
                                            
                                            web_sys::console::log_1(&format!("Extracted session ID from message: {}", session_id).into());
                                            
                                            // Check if we have a session in state that we could use instead
                                            if !session_id.contains(':') {
                                                if let Some((stored_id, _)) = &*session_info_state {
                                                    if stored_id.contains(':') {
                                                        web_sys::console::log_1(&format!("Replacing invalid session ID with stored one: {}", stored_id).into());
                                                        // Pass the valid session_id from state to submit_score
                                                        submit_score(score as i32, stored_id.clone(), leaderboard_update_trigger_for_closure.clone());
                                                        return;
                                                    }
                                                }
                                            }
                                            
                                            // Pass the session_id to submit_score
                                            submit_score(score as i32, session_id, leaderboard_update_trigger_for_closure.clone());
                                        }
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(web_sys::MessageEvent)>);
            
            window.add_event_listener_with_callback("message", closure.as_ref().unchecked_ref()).expect("listener add failed");
            
            move || {
                window.remove_event_listener_with_callback("message", closure.as_ref().unchecked_ref()).expect("listener remove failed");
                drop(closure);
            }
        });
    }

    // Add this function after the message event handler use_effect_with block
    fn submit_score(score: i32, session_id: String, leaderboard_update_trigger: UseStateHandle<u32>) {
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(token) = get_auth_token() {
                let api_base = get_api_base_url();
                // Use the correct API endpoint
                let url = if api_base.is_empty() {
                    "/hexort/score".to_string()
                } else {
                    format!("{}/hexort/score", api_base)
                };
                
                // Get current time in seconds
                let now = js_sys::Date::now() / 1000.0;
                
                web_sys::console::log_1(&format!("Using session ID: {}", &session_id).into());
                
                // Construct the proper payload
                let payload = serde_json::json!({
                    "score": score,
                    "game_type": "hexort",
                    "timestamp": now as u64,
                    "session_id": session_id, // Use the session ID from the message
                    "disable_rewards": true  // Disable PAX rewards
                });
                
                web_sys::console::log_1(&format!("Submitting score to {}. Payload: {:?}", url, payload).into());
                
                // Create a structured request
                let request_builder = Request::post(&url).header("Authorization", &format!("Bearer {}", token));
                
                web_sys::console::log_1(&"Request builder created. Adding JSON payload...".into());
                
                // Add JSON payload, handle potential error
                let request = match request_builder.json(&payload) {
                    Ok(req) => req,
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to serialize JSON payload: {:?}", e).into());
                        return;
                    }
                };
                
                web_sys::console::log_1(&"Request ready. Sending to backend...".into());
                
                // Send the request
                match request.send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        web_sys::console::log_1(&format!("Received response with status: {}", status).into());
                        
                        if status == 200 {
                            web_sys::console::log_1(&"Hexort score updated successfully".into());
                            // Increment the leaderboard update trigger to refresh the leaderboard
                            leaderboard_update_trigger.set(*leaderboard_update_trigger + 1);
                        } else {
                            // Try to get error text for better diagnostics
                            match resp.text().await {
                                Ok(error_text) => {
                                    web_sys::console::error_1(&format!("Failed to update score: HTTP {} - {}", status, error_text).into());
                                }
                                Err(_) => {
                                    web_sys::console::error_1(&format!("Failed to update score: HTTP {}", status).into());
                                }
                            }
                        }
                    },
                    Err(e) => {
                        web_sys::console::error_1(&format!("Error submitting score: {}", e).into());
                    }
                }
            } else {
                web_sys::console::error_1(&"No auth token available for score submission".into());
            }
        });
    }

    // Conditional Rendering based on state
    let leaderboard_trigger_value = *leaderboard_update_trigger;
    
    html! {
        <div class={classes!(styles::CONTAINER, "flex", "flex-col", "items-center", "justify-start")}>
            <h1 class="text-2xl font-bold text-center my-4 text-gray-900 dark:text-white">{"Hexort"}</h1>
            <p class="text-center text-sm text-gray-600 dark:text-gray-400 mb-4">{"Hexort gives no rewards currently"}</p>

            if !(*error_message).is_empty() {
                <div class={classes!(styles::ALERT_ERROR, "m-4", "p-3")}> {&*error_message} </div>
            }
            
            if *is_on_cooldown {
                <div class="p-4 bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-200 text-center">
                    {format!("You can play again in {} seconds", *cooldown_seconds)}
                </div>
            } else if session_info_state.is_none() && (*error_message).is_empty() {
                 <div class="p-4 text-center text-gray-600 dark:text-gray-300">{"Loading game session..."}</div>
            } else if session_info_state.is_some() {
                 <>
                    <div class="w-full mx-auto pt-0">
                        <div 
                            // Container with aspect ratio, max-width and responsive width
                            class="relative aspect-[9/16] rounded-lg overflow-hidden mx-auto" 
                            style="max-width: 360px; width: 100%;"
                        >
                            <iframe 
                                ref={iframe_ref}
                                // Keep original transform classes but make it responsive while maintaining position
                                class="absolute top-0 left-1/2 -translate-x-1/2 -translate-y-1/4 border-none rounded-lg transform origin-top"
                                style="width: 200%; height: 200%;"
                                src="/static/webgl-games/hexort/index.html"
                                sandbox="allow-same-origin allow-scripts"
                                title="Hexort Game"
                                allow="autoplay"
                                onload={Callback::from(|_| {
                                    web_sys::console::log_1(&"Hexort game iframe loaded, iframe should request session info now.".into());
                                })}
                            />
                        </div>
                    </div>
                </>
            } else if !(*error_message).is_empty() {
                 <></>
            } else {
                 <div class="p-4 text-center text-red-600 dark:text-red-400">{"Failed to load game session. Please try refreshing."}</div>
            }

            // Add the leaderboard component
            <div class="w-full">
                <HexortLeaderboard update_trigger={leaderboard_trigger_value} />
            </div>
        </div>
    }
} 
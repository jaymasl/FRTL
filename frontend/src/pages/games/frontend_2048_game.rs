use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use shared::shared_2048_game::{Direction, PublicGame2048};
use web_sys::{window, CustomEvent, CustomEventInit, KeyboardEvent};
use wasm_bindgen::JsValue;
use wasm_bindgen::JsCast;
use gloo_events::EventListener;
use crate::config::get_api_base_url;
use crate::pages::games::frontend_2048_leaderboard::Game2048Leaderboard;

fn get_auth_token() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| {
            web_sys::window()
                .and_then(|w| w.session_storage().ok().flatten())
                .and_then(|s| s.get_item("token").ok().flatten())
        })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewGame2048Response {
    pub session_id: String,
    pub session_signature: String,
    pub game: PublicGame2048,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MoveRequest {
    pub session_id: String,
    pub direction: Direction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MoveResponse {
    pub moved: bool,
    pub game: PublicGame2048,
    pub score: u32,
    pub new_balance: Option<i32>,
}

#[function_component(Frontend2048Game)]
pub fn frontend_2048_game() -> Html {
    let game_state = use_state(|| None as Option<PublicGame2048>);
    let session_id = use_state(|| None as Option<String>);
    let session_sig = use_state(|| None as Option<String>);
    let last_move = use_state(|| 0f64);
    let error_message = use_state(String::new);
    let is_processing_move = use_state(|| false);
    let leaderboard_update_trigger = use_state(|| 0u32);

    {
        let game_state_eff = game_state.clone();
        let session_id_eff = session_id.clone();
        let session_sig_eff = session_sig.clone();
        let last_move_eff = last_move.clone();
        let error_message_eff = error_message.clone();
        let is_processing_move_eff = is_processing_move.clone();
        let leaderboard_update_trigger_eff = leaderboard_update_trigger.clone();
        
        use_effect(move || {
            let document = web_sys::window().unwrap().document().unwrap();
            
            let options = gloo_events::EventListenerOptions {
                passive: false,
                phase: gloo_events::EventListenerPhase::Bubble,
            };
            
            let game_state_down = game_state_eff.clone();
            let session_id_down = session_id_eff.clone();
            let session_sig_down = session_sig_eff.clone();
            let last_move_down = last_move_eff.clone();
            let error_message_down = error_message_eff.clone();
            let is_processing_move_down = is_processing_move_eff.clone();
            let leaderboard_update_trigger_down = leaderboard_update_trigger_eff.clone();
            
            let keydown_listener = EventListener::new_with_options(&document, "keydown", options.clone(), move |event| {
                let event = event.dyn_ref::<KeyboardEvent>().unwrap();
                if event.repeat() { return; }
                let key = event.key();
                match key.as_str() {
                    "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" |
                    "w" | "W" | "a" | "A" | "s" | "S" | "d" | "D" |
                    "i" | "I" | "j" | "J" | "k" | "K" | "l" | "L" |
                    "8" | "4" | "2" | "6" | "Numpad8" | "Numpad4" | "Numpad2" | "Numpad6" => {
                        event.prevent_default();
                        event.stop_propagation();
                        log::info!("Prevented default for key: {}", key);
                    },
                    _ => { return; }
                }
                
                let direction = match key.as_str() {
                    "ArrowUp" | "w" | "W" | "i" | "I" | "8" | "Numpad8" => Some(Direction::Up),
                    "ArrowDown" | "s" | "S" | "k" | "K" | "2" | "Numpad2" => Some(Direction::Down),
                    "ArrowLeft" | "a" | "A" | "j" | "J" | "4" | "Numpad4" => Some(Direction::Left),
                    "ArrowRight" | "d" | "D" | "l" | "L" | "6" | "Numpad6" => Some(Direction::Right),
                    _ => None,
                };
                if direction.is_none() { return; }
                let direction = direction.unwrap();
                
                if game_state_down.as_ref().map_or(true, |game| game.game_over) {
                    return;
                }
                
                let now = web_sys::window().unwrap().performance().unwrap().now();
                
                if now - *last_move_down < 200.0 || *is_processing_move_down {
                    return;
                }
                
                last_move_down.set(now);
                error_message_down.set(String::new());
                is_processing_move_down.set(true);
                
                let sid = session_id_down.clone();
                let ss = session_sig_down.clone();
                let gs = game_state_down.clone();
                let em = error_message_down.clone();
                let ipm = is_processing_move_down.clone();
                let lut = leaderboard_update_trigger_down.clone();
                
                spawn_local(async move {
                    handle_move(direction, &sid, &ss, gs.clone(), em.clone(), lut).await;
                    ipm.set(false);
                });
            });
            
            let game_state_touch = game_state_eff.clone();
            let session_id_touch = session_id_eff.clone();
            let session_sig_touch = session_sig_eff.clone();
            let last_move_touch = last_move_eff.clone();
            let error_message_touch = error_message_eff.clone();
            let is_processing_move_touch = is_processing_move_eff.clone();
            let leaderboard_update_trigger_touch = leaderboard_update_trigger_eff.clone();
            
            let touch_start_x = std::rc::Rc::new(std::cell::RefCell::new(0.0));
            let touch_start_y = std::rc::Rc::new(std::cell::RefCell::new(0.0));
            
            let touch_start_x_clone = touch_start_x.clone();
            let touch_start_y_clone = touch_start_y.clone();
            let touchstart_listener = EventListener::new_with_options(&document, "touchstart", options.clone(), move |event| {
                let event = event.dyn_ref::<web_sys::TouchEvent>().unwrap();
                if is_game_board_element(event.target()) {
                    event.prevent_default();
                }
                
                if let Some(touch) = event.touches().get(0) {
                    *touch_start_x_clone.borrow_mut() = touch.client_x() as f64;
                    *touch_start_y_clone.borrow_mut() = touch.client_y() as f64;
                }
            });
            
            let touch_start_x_clone = touch_start_x.clone();
            let touch_start_y_clone = touch_start_y.clone();
            let game_state_touch_clone = game_state_touch.clone();
            let session_id_touch_clone = session_id_touch.clone();
            let session_sig_touch_clone = session_sig_touch.clone();
            let last_move_touch_clone = last_move_touch.clone();
            let error_message_touch_clone = error_message_touch.clone();
            let is_processing_move_touch_clone = is_processing_move_touch.clone();
            let leaderboard_update_trigger_touch_clone = leaderboard_update_trigger_touch.clone();
            
            let touchend_listener = EventListener::new_with_options(&document, "touchend", options.clone(), move |event| {
                let event = event.dyn_ref::<web_sys::TouchEvent>().unwrap();
                
                if is_game_board_element(event.target()) {
                    event.prevent_default();
                }
                
                if let Some(touch) = event.changed_touches().get(0) {
                    let touch_end_x = touch.client_x() as f64;
                    let touch_end_y = touch.client_y() as f64;
                    
                    let delta_x = touch_end_x - *touch_start_x_clone.borrow();
                    let delta_y = touch_end_y - *touch_start_y_clone.borrow();
                    
                    let min_swipe_distance = 30.0;
                    
                    let direction = if delta_x.abs() > delta_y.abs() {
                        if delta_x > min_swipe_distance {
                            Some(Direction::Right)
                        } else if delta_x < -min_swipe_distance {
                            Some(Direction::Left)
                        } else {
                            None
                        }
                    } else {
                        if delta_y > min_swipe_distance {
                            Some(Direction::Down)
                        } else if delta_y < -min_swipe_distance {
                            Some(Direction::Up)
                        } else {
                            None
                        }
                    };
                    
                    if let Some(direction) = direction {
                        if game_state_touch_clone.as_ref().map_or(true, |game| game.game_over) {
                            return;
                        }
                        
                        let now = web_sys::window().unwrap().performance().unwrap().now();
                        
                        if now - *last_move_touch_clone < 200.0 || *is_processing_move_touch_clone {
                            return;
                        }
                        
                        last_move_touch_clone.set(now);
                        error_message_touch_clone.set(String::new());
                        is_processing_move_touch_clone.set(true);
                        
                        let sid = session_id_touch_clone.clone();
                        let ss = session_sig_touch_clone.clone();
                        let gs = game_state_touch_clone.clone();
                        let em = error_message_touch_clone.clone();
                        let ipm = is_processing_move_touch_clone.clone();
                        let lut = leaderboard_update_trigger_touch_clone.clone();
                        
                        spawn_local(async move {
                            handle_move(direction, &sid, &ss, gs.clone(), em.clone(), lut).await;
                            ipm.set(false);
                        });
                    }
                }
            });
            
            let touchmove_listener = EventListener::new_with_options(&document, "touchmove", options.clone(), move |event| {
                let event = event.dyn_ref::<web_sys::TouchEvent>().unwrap();
                if is_game_board_element(event.target()) {
                    event.prevent_default();
                }
            });
            
            || {
                drop(keydown_listener);
                drop(touchstart_listener);
                drop(touchend_listener);
                drop(touchmove_listener);
            }
        });
    }

    let on_new_game = {
        let game_state = game_state.clone();
        let session_id = session_id.clone();
        let session_sig = session_sig.clone();
        Callback::from(move |_| {
            let game_state = game_state.clone();
            let session_id = session_id.clone();
            let session_sig = session_sig.clone();
            spawn_local(async move {
                let token = get_auth_token();
                let api_base = get_api_base_url();
                
                let url = if api_base.is_empty() {
                    "/2048/new".to_string()
                } else {
                    format!("{}/2048/new", api_base)
                };
                
                if let Ok(resp) = Request::post(&url)
                    .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                    .send()
                    .await
                {
                    if resp.status() == 200 {
                        if let Ok(new_game) = resp.json::<NewGame2048Response>().await {
                            session_id.set(Some(new_game.session_id.clone()));
                            session_sig.set(Some(new_game.session_signature.clone()));
                            game_state.set(Some(new_game.game));
                        }
                    } else {
                        log::error!("Failed to start new game: {}", resp.status());
                    }
                } else {
                    log::error!("Network error starting new game");
                }
            });
        })
    };

    async fn handle_move(
        direction: Direction,
        session_id: &UseStateHandle<Option<String>>,
        session_sig: &UseStateHandle<Option<String>>,
        game_state: UseStateHandle<Option<PublicGame2048>>,
        error_message: UseStateHandle<String>,
        leaderboard_update_trigger: UseStateHandle<u32>,
    ) {
        let token = get_auth_token();
        let api_base = get_api_base_url();
        if let Some(sid) = (*session_id).as_ref() {
            let move_req = MoveRequest {
                session_id: sid.clone(),
                direction,
            };
            
            let url = if api_base.is_empty() {
                "/2048/move".to_string()
            } else {
                format!("{}/2048/move", api_base)
            };
            
            if let Ok(resp) = Request::post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                .header("X-Session-Signature", &session_sig.as_ref().unwrap_or(&String::new()))
                .body(serde_json::to_string(&move_req).unwrap())
                .expect("Failed to build request")
                .send()
                .await
            {
                if resp.status() == 200 {
                    if let Ok(move_resp) = resp.json::<MoveResponse>().await {
                        let is_game_over = move_resp.game.game_over;
                        let new_balance = move_resp.new_balance;
                        
                        if is_game_over {
                            leaderboard_update_trigger.set(*leaderboard_update_trigger + 1);
                            
                            if let Some(balance) = new_balance {
                                log::info!("Received new balance after game over: {}", balance);
                                if let Some(window) = window() {
                                    if let Some(storage) = window.local_storage().ok().flatten() {
                                        let _ = storage.set_item("currency", &balance.to_string());
                                    }
                                    
                                    let event_init = CustomEventInit::new();
                                    event_init.set_detail(&JsValue::from_f64(balance as f64));
                                    if let Ok(event) = CustomEvent::new_with_event_init_dict(
                                        "currencyUpdate",
                                        &event_init,
                                    ) {
                                        let _ = window.dispatch_event(&event);
                                        log::info!("Dispatched currencyUpdate event with new balance: {}", balance);
                                    }
                                }
                            } else {
                                log::warn!("Game over but no new balance received");
                            }
                        }

                        game_state.set(Some(move_resp.game));
                        error_message.set(String::new());
                    }
                } else if resp.status() == 429 {
                    // Removed rate limit handling to simplify the game; do nothing.
                } else if resp.status() == 410 {
                    error_message.set("Session expired. Please start a new game.".to_string());
                    game_state.set(None);
                    session_id.set(None);
                    session_sig.set(None);
                    log::error!("Session expired");
                } else if resp.status() == 404 {
                    error_message.set("Game not found. Please start a new game.".to_string());
                    game_state.set(None);
                    session_id.set(None);
                    session_sig.set(None);
                    log::error!("Game not found");
                } else {
                    error_message.set("Failed to process move".to_string());
                    log::error!("Failed to process move: {}", resp.status());
                }
            } else {
                error_message.set("Network error".to_string());
                log::error!("Network error processing move");
            }
        }
    }

    let leaderboard_update_trigger_for_template = leaderboard_update_trigger.clone();

    html! {
        <div class="flex flex-col items-center w-full">
            <div class="bg-white dark:bg-gray-800 p-8 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] max-w-2xl mx-auto">
                <h1 class="text-3xl font-bold mb-6 text-center text-gray-900 dark:text-white">{"2048 Game"}</h1>
                { if game_state.is_none() { html! {
                    <p class="mb-4 text-center text-gray-700 dark:text-gray-300">
                        { "Use arrow keys, WASD, IJKL, or numpad 8426. 
                        On mobile, swipe up/down/left/right to move. 
                        Earn 1 pax for every 50 points when game ends. 
                        On mobile, swipe up/down/left/right to move. 
                        Earn 1 pax for every 50 points when game ends" }
                    </p>
                } } else { html! {} } }
                
                { if game_state.is_none() || (game_state.as_ref().map_or(false, |game| game.game_over)) {
                    html! {
                        <div class="flex justify-center mb-6">
                            <button 
                                onclick={on_new_game}
                                class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                            >
                                { if game_state.is_none() { "New Game" } else { "Play Again" } }
                            </button>
                        </div>
                    }
                } else {
                    html! {}
                } }
                
                { if !(*error_message).is_empty() {
                    html! {
                        <div class="mb-4 text-red-500 text-center">{ &*error_message }</div>
                    }
                } else {
                    html! {}
                } }
                { if let Some(game) = &*game_state {
                    html! {
                        <div class="space-y-4">
                            <div class="grid grid-cols-4 gap-4 w-[320px] h-[320px] mx-auto">
                                { for game.board.iter().flat_map(|row| row.iter()).map(|cell| {
                                    let display = cell.map_or("".to_string(), |n| n.to_string());
                                    let bg_color = match cell {
                                        Some(2) => "bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
                                        Some(4) => "bg-gray-300 dark:bg-gray-600 text-gray-800 dark:text-gray-200",
                                        Some(8) => "bg-orange-200 text-gray-800",
                                        Some(16) => "bg-orange-300 text-gray-800",
                                        Some(32) => "bg-orange-400 text-white",
                                        Some(64) => "bg-orange-500 text-white",
                                        Some(128) => "bg-yellow-200 text-gray-800",
                                        Some(256) => "bg-yellow-300 text-gray-800",
                                        Some(512) => "bg-yellow-400 text-white",
                                        Some(1024) => "bg-yellow-500 text-white",
                                        Some(2048) => "bg-yellow-600 text-white",
                                        _ => "bg-gray-100 dark:bg-gray-900",
                                    };
                                    html! {
                                        <div class={classes!("w-[70px]", "h-[70px]", "flex", "items-center", "justify-center", "text-xl", "font-bold", "rounded", bg_color)}>
                                            { display }
                                        </div>
                                    }
                                }) }
                            </div>
                            <p class="mt-4 text-center text-gray-800 dark:text-gray-200">{ format!("Score: {}", game.score) }</p>
                            { if game.game_over {
                                let pax_earned = game.score / 50;
                                html! {
                                    <div class="mt-2 text-center">
                                        <p class="text-red-500 font-bold">{"Game Over!"}</p>
                                        <div class="mt-2 p-3 border-2 border-green-500 rounded-lg bg-green-50 dark:bg-green-900/20">
                                            <p class="text-green-600 dark:text-green-400">
                                                { if pax_earned > 0 {
                                                    format!("You received {} pax!", pax_earned)
                                                } else {
                                                    "Get over 50 points to earn pax! (1 pax per 50 score)".to_string()
                                                } }
                                            </p>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            } }
                        </div>
                    }
                } else {
                    html! {}
                } }
            </div>
            
            <div class="mt-8 w-full max-w-3xl"> 
                <Game2048Leaderboard update_trigger={*leaderboard_update_trigger_for_template} />
            </div>
        </div>
    }
}

fn is_game_board_element(target: Option<web_sys::EventTarget>) -> bool {
    if let Some(target) = target {
        if let Some(element) = target.dyn_ref::<web_sys::Element>() {
            if let Ok(closest) = element.closest(".grid-cols-4") {
                return closest.is_some();
            }
        }
    }
    false
}
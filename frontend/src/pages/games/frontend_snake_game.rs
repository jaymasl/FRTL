use yew::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, CanvasRenderingContext2d};
use wasm_bindgen::{JsCast, closure::Closure};
use gloo_net::websocket::{futures::WebSocket, Message};
use futures::{StreamExt, SinkExt};
use shared::shared_snake_game::{SnakeGame, Direction, SnakeMessage, FoodType};
use std::rc::Rc;
use futures::lock::Mutex;
use futures::stream::{SplitSink, SplitStream};
use log::{info, error, debug};
use web_sys::window;
use crate::pages::games::snake_leaderboard::SnakeLeaderboard;
use crate::config::get_api_base_url;

fn get_auth_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
}

pub enum Msg {
    Connect,
    Received(String),
    KeyPress(KeyboardEvent),
    Disconnect,
    StartGame,
    ConnectionError(String),
    GameOver,
    SwipeDirection(Direction),
    ArrowButtonClick(Direction),
}

pub struct FrontendSnakeGame {
    game_state: Option<SnakeGame>,
    ws_write: Option<Rc<Mutex<SplitSink<WebSocket, Message>>>>,
    canvas_ref: NodeRef,
    _keydown_listener: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    _keypress_listener: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    error_message: Option<String>,
    game_over: bool,
    waiting_for_first_key: bool,
    leaderboard_update_trigger: u32,
    _touchstart_listener: Option<Closure<dyn FnMut(web_sys::TouchEvent)>>,
    _touchend_listener: Option<Closure<dyn FnMut(web_sys::TouchEvent)>>,
    _touchmove_listener: Option<Closure<dyn FnMut(web_sys::TouchEvent)>>,
}

impl Component for FrontendSnakeGame {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            game_state: None,
            ws_write: None,
            canvas_ref: NodeRef::default(),
            _keydown_listener: None,
            _keypress_listener: None,
            error_message: None,
            game_over: false,
            waiting_for_first_key: false,
            leaderboard_update_trigger: 0,
            _touchstart_listener: None,
            _touchend_listener: None,
            _touchmove_listener: None,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let start_game = ctx.link().callback(|_| Msg::StartGame);
        
        html! {
            <div class="flex flex-col items-center w-full">
                <div class="bg-white dark:bg-gray-800 p-8 pb-6 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] max-w-md w-full">
                    <h1 class="text-3xl font-bold mb-6 text-center text-gray-900 dark:text-white">{ "Snake Game" }</h1>
                    
                    // Only show the description and button if the game is not active or game is over
                    if self.game_state.is_none() || self.game_over {
                        <>
                            // Show reward information before game starts
                            <div class="text-center mb-6">
                                <div class="flex flex-col items-center text-base text-gray-900 dark:text-gray-100 space-y-1">
                                    {"Earn pax reward every 5 points.
                                    Rare yellow squares give 1 scroll.
                                    Higher score gives more pax and scroll %"}
                                </div>
                            </div>
                            
                            <div class="flex justify-center mb-4">
                                <button 
                                    onclick={start_game}
                                    class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                                >
                                    { if self.game_over { "Play Again" } else { "Start Game" } }
                                </button>
                            </div>
                        </>
                    }
                    
                    if let Some(error) = &self.error_message {
                        <div class="text-red-500 text-center mb-4">
                            {error}
                        </div>
                    }

                    if let Some(game) = &self.game_state {
                        <div class="flex flex-col items-center">
                            <div class="game-container relative">
                                <canvas 
                                    ref={self.canvas_ref.clone()}
                                    width="400"
                                    height="400"
                                    id="snake-game-canvas"
                                    class="border-2 border-gray-300 dark:border-gray-600"
                                />
                            </div>
                            <p class="text-xl text-gray-700 dark:text-gray-300 mt-4">
                                { format!("Score: {}", game.score) }
                            </p>
                            
                            // Mobile arrow keypad - only show when game is active and not game over
                            if !self.game_over {
                                <div class="mt-4 grid grid-cols-3 gap-2 w-48">
                                    <div></div>
                                    <button 
                                        onclick={ctx.link().callback(|_| Msg::ArrowButtonClick(Direction::Up))}
                                        class="bg-gradient-to-br from-indigo-500 to-blue-600 hover:from-indigo-600 hover:to-blue-700 text-white font-bold py-2 px-4 rounded-lg shadow-lg hover:shadow-xl transition-all duration-200 flex items-center justify-center active:scale-95"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7" />
                                        </svg>
                                    </button>
                                    <div></div>
                                    <button 
                                        onclick={ctx.link().callback(|_| Msg::ArrowButtonClick(Direction::Left))}
                                        class="bg-gradient-to-br from-indigo-500 to-blue-600 hover:from-indigo-600 hover:to-blue-700 text-white font-bold py-2 px-4 rounded-lg shadow-lg hover:shadow-xl transition-all duration-200 flex items-center justify-center active:scale-95"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                                        </svg>
                                    </button>
                                    <div></div>
                                    <button 
                                        onclick={ctx.link().callback(|_| Msg::ArrowButtonClick(Direction::Right))}
                                        class="bg-gradient-to-br from-indigo-500 to-blue-600 hover:from-indigo-600 hover:to-blue-700 text-white font-bold py-2 px-4 rounded-lg shadow-lg hover:shadow-xl transition-all duration-200 flex items-center justify-center active:scale-95"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                                        </svg>
                                    </button>
                                    <div></div>
                                    <button 
                                        onclick={ctx.link().callback(|_| Msg::ArrowButtonClick(Direction::Down))}
                                        class="bg-gradient-to-br from-indigo-500 to-blue-600 hover:from-indigo-600 hover:to-blue-700 text-white font-bold py-2 px-4 rounded-lg shadow-lg hover:shadow-xl transition-all duration-200 flex items-center justify-center active:scale-95"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                                        </svg>
                                    </button>
                                    <div></div>
                                </div>
                            }
                        </div>
                    }

                    if self.game_over {
                        <p class="text-xl text-center mt-2 text-red-500">
                            { "Game Over!" }
                        </p>
                    }
                </div>
                
                // Add the snake game leaderboard component below the game card with more spacing
                <div class="mt-2 w-full max-w-3xl">
                    <SnakeLeaderboard update_trigger={self.leaderboard_update_trigger} />
                </div>
            </div>
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        // Only initialize canvas if we have an active game
        if let Some(_) = &self.game_state {
            if let Some(canvas) = self.canvas_ref.cast::<HtmlCanvasElement>() {
                if let Ok(Some(context)) = canvas.get_context("2d") {
                    if let Ok(_) = context.dyn_into::<CanvasRenderingContext2d>() {
                        self.render_game();
                    }
                }
            }
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Connect => {
                info!("Attempting to connect to WebSocket");
                let token = get_auth_token();
                let api_base = get_api_base_url();
                
                // Construct WebSocket URL based on API base
                let ws_url = if api_base.is_empty() {
                    // If API base is empty (production), use current origin with ws/wss protocol
                    if let Some(window) = window() {
                        let location = window.location();
                        let protocol = location.protocol().unwrap_or_default();
                        let host = location.host().unwrap_or_default();
                        
                        // Convert http/https to ws/wss
                        let ws_protocol = if protocol.starts_with("https") {
                            "wss"
                        } else {
                            "ws"
                        };
                        
                        format!("{}://{}/snake-game/ws", ws_protocol, host)
                    } else {
                        "ws://127.0.0.1:3000/snake-game/ws".to_string()
                    }
                } else {
                    // Replace http/https with ws/wss
                    let ws_base = api_base.replace("http://", "ws://").replace("https://", "wss://");
                    format!("{}/snake-game/ws", ws_base)
                };
                
                info!("Connecting to WebSocket at: {}", ws_url);
                match WebSocket::open(&ws_url) {
                    Ok(ws) => {
                        let (write, mut read): (SplitSink<_, _>, SplitStream<_>) = ws.split();
                        let link = ctx.link().clone();
                        
                        info!("WebSocket connected successfully");
                        
                        let write = Rc::new(Mutex::new(write));
                        self.ws_write = Some(write.clone());
                        
                        let token = token.unwrap_or_default();
                        wasm_bindgen_futures::spawn_local(async move {
                            let mut ws_write = write.lock().await;
                            if let Err(e) = ws_write.send(Message::Text(format!("Bearer {}", token))).await {
                                error!("Failed to send auth token: {:?}", e);
                                return;
                            }
                            // We'll send the Start message, but we won't set started=true until a key is pressed
                            let msg = SnakeMessage::Start;
                            if let Ok(text) = serde_json::to_string(&msg) {
                                if let Err(e) = ws_write.send(Message::Text(text)).await {
                                    error!("Failed to send start message: {:?}", e);
                                }
                            } else {
                                error!("Failed to serialize start message");
                            }
                        });
                        
                        // Create a flag to track if game over has been received
                        let game_over_received = Rc::new(std::cell::RefCell::new(false));
                        let game_over_clone = game_over_received.clone();
                        
                        wasm_bindgen_futures::spawn_local(async move {
                            while let Some(msg) = read.next().await {
                                match msg {
                                    Ok(Message::Text(text)) => {
                                        // Try to parse as SnakeMessage first
                                        if let Ok(snake_msg) = serde_json::from_str::<SnakeMessage>(&text) {
                                            match snake_msg {
                                                SnakeMessage::GameOver => {
                                                    info!("Received GameOver message");
                                                    link.send_message(Msg::GameOver);
                                                    
                                                    // Set the game over flag
                                                    *game_over_clone.borrow_mut() = true;
                                                    
                                                    // Don't close the connection immediately, let the server handle it
                                                    // This prevents the WebSocket error on game over
                                                    continue;
                                                }
                                                SnakeMessage::BalanceUpdate(new_balance) => {
                                                    if let Some(window) = web_sys::window() {
                                                        let event_init = web_sys::CustomEventInit::new();
                                                        event_init.set_detail(&wasm_bindgen::JsValue::from_f64(new_balance));
                                                        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                                            "currencyUpdate",
                                                            &event_init,
                                                        ) {
                                                            let _ = window.dispatch_event(&event);
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            link.send_message(Msg::Received(text));
                                        }
                                    }
                                    Err(e) => {
                                        // Only log as error if the game is not over
                                        if !*game_over_clone.borrow() {
                                            error!("WebSocket error: {:?}", e);
                                            link.send_message(Msg::ConnectionError(format!("WebSocket error: {:?}", e)));
                                        } else {
                                            // If game is over, just log as info since we expect the connection to close
                                            info!("WebSocket closed after game over: {:?}", e);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            debug!("WebSocket connection closed");
                        });

                        self.error_message = None;
                        self.game_over = false;
                        
                        if let Some(old_listener) = self._keydown_listener.take() {
                            web_sys::window()
                                .unwrap()
                                .remove_event_listener_with_callback(
                                    "keydown",
                                    old_listener.as_ref().unchecked_ref(),
                                )
                                .unwrap_or_else(|_| error!("Failed to remove old event listener"));
                        }
                        
                        // Set up keyboard event listeners
                        let link = ctx.link().clone();
                        let link_down = link.clone();
                        let listener = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                            e.prevent_default();
                            link_down.send_message(Msg::KeyPress(e));
                        }) as Box<dyn FnMut(KeyboardEvent)>);
                        
                        web_sys::window()
                            .unwrap()
                            .add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
                            .unwrap();
                        
                        let link_press = link.clone();
                        let listener_press = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                            e.prevent_default();
                            link_press.send_message(Msg::KeyPress(e));
                        }) as Box<dyn FnMut(KeyboardEvent)>);
                        
                        web_sys::window()
                            .unwrap()
                            .add_event_listener_with_callback("keypress", listener_press.as_ref().unchecked_ref())
                            .unwrap();
                        
                        self._keydown_listener = Some(listener);
                        self._keypress_listener = Some(listener_press);
                        
                        // Set up touch event listeners for swipe detection
                        let document = web_sys::window().unwrap().document().unwrap();
                        
                        // Use RefCell to store touch positions
                        let touch_start_x = std::rc::Rc::new(std::cell::RefCell::new(0.0));
                        let touch_start_y = std::rc::Rc::new(std::cell::RefCell::new(0.0));
                        
                        // Create options for event listeners with passive: false to allow preventDefault
                        let options = web_sys::AddEventListenerOptions::new();
                        options.set_passive(false);
                        
                        // Add touchstart event listener
                        let touch_start_x_clone = touch_start_x.clone();
                        let touch_start_y_clone = touch_start_y.clone();
                        let touchstart_listener = Closure::wrap(Box::new(move |e: web_sys::TouchEvent| {
                            // Prevent default only on game canvas to avoid interfering with other page interactions
                            if is_game_canvas_element(e.target()) {
                                e.prevent_default();
                                e.stop_propagation();
                            }
                            
                            if let Some(touch) = e.touches().get(0) {
                                *touch_start_x_clone.borrow_mut() = touch.client_x() as f64;
                                *touch_start_y_clone.borrow_mut() = touch.client_y() as f64;
                            }
                        }) as Box<dyn FnMut(web_sys::TouchEvent)>);
                        
                        document
                            .add_event_listener_with_callback_and_add_event_listener_options(
                                "touchstart", 
                                touchstart_listener.as_ref().unchecked_ref(),
                                &options
                            )
                            .unwrap();
                        
                        // Add touchend event listener for swipe detection
                        let touch_start_x_clone = touch_start_x.clone();
                        let touch_start_y_clone = touch_start_y.clone();
                        let link_touch = link.clone();
                        let touchend_listener = Closure::wrap(Box::new(move |e: web_sys::TouchEvent| {
                            // Prevent default only on game canvas
                            if is_game_canvas_element(e.target()) {
                                e.prevent_default();
                                e.stop_propagation();
                            }
                            
                            if let Some(touch) = e.changed_touches().get(0) {
                                let touch_end_x = touch.client_x() as f64;
                                let touch_end_y = touch.client_y() as f64;
                                
                                // Calculate the distance moved
                                let delta_x = touch_end_x - *touch_start_x_clone.borrow();
                                let delta_y = touch_end_y - *touch_start_y_clone.borrow();
                                
                                // Minimum distance to be considered a swipe
                                let min_swipe_distance = 30.0;
                                
                                // Determine swipe direction
                                let direction = if delta_x.abs() > delta_y.abs() {
                                    // Horizontal swipe
                                    if delta_x > min_swipe_distance {
                                        Some(Direction::Right)
                                    } else if delta_x < -min_swipe_distance {
                                        Some(Direction::Left)
                                    } else {
                                        None
                                    }
                                } else {
                                    // Vertical swipe
                                    if delta_y > min_swipe_distance {
                                        Some(Direction::Down)
                                    } else if delta_y < -min_swipe_distance {
                                        Some(Direction::Up)
                                    } else {
                                        None
                                    }
                                };
                                
                                if let Some(dir) = direction {
                                    // Send the direction change message directly without creating an unused key variable
                                    link_touch.send_message(Msg::SwipeDirection(dir));
                                }
                            }
                        }) as Box<dyn FnMut(web_sys::TouchEvent)>);
                        
                        document
                            .add_event_listener_with_callback_and_add_event_listener_options(
                                "touchend", 
                                touchend_listener.as_ref().unchecked_ref(),
                                &options
                            )
                            .unwrap();
                        
                        // Add touchmove listener to prevent scrolling when swiping on the game canvas
                        let touchmove_listener = Closure::wrap(Box::new(move |e: web_sys::TouchEvent| {
                            // Prevent default only on game canvas to avoid interfering with page scrolling
                            if is_game_canvas_element(e.target()) {
                                e.prevent_default();
                                e.stop_propagation();
                            }
                        }) as Box<dyn FnMut(web_sys::TouchEvent)>);
                        
                        document
                            .add_event_listener_with_callback_and_add_event_listener_options(
                                "touchmove", 
                                touchmove_listener.as_ref().unchecked_ref(),
                                &options
                            )
                            .unwrap();
                        
                        // Store the touch event listeners
                        self._touchstart_listener = Some(touchstart_listener);
                        self._touchend_listener = Some(touchend_listener);
                        self._touchmove_listener = Some(touchmove_listener);
                    }
                    Err(e) => {
                        let error = format!("Failed to connect to WebSocket: {:?}", e);
                        error!("{}", error);
                        self.error_message = Some(error);
                    }
                }
                true
            }
            Msg::Received(text) => {
                if let Ok(mut game) = serde_json::from_str::<SnakeGame>(&text) {
                    // Update currency if new balance is available
                    if let Some(new_balance) = game.new_balance {
                        if let Some(window) = web_sys::window() {
                            let event_init = web_sys::CustomEventInit::new();
                            event_init.set_detail(&wasm_bindgen::JsValue::from_f64(new_balance));
                            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                "currencyUpdate",
                                &event_init,
                            ) {
                                let _ = window.dispatch_event(&event);
                            }
                        }
                    }
                    
                    // If we're waiting for the first key press, make sure the game doesn't start moving
                    if self.waiting_for_first_key {
                        game.started = false;
                    }
                    
                    // Update game state
                    self.game_state = Some(game);
                    self.render_game();
                }
                true
            }
            Msg::GameOver => {
                info!("Game Over received");
                self.game_over = true;
                self.leaderboard_update_trigger = self.leaderboard_update_trigger.wrapping_add(1);
                if let Some(listener) = self._keydown_listener.take() {
                    web_sys::window()
                        .unwrap()
                        .remove_event_listener_with_callback(
                            "keydown",
                            listener.as_ref().unchecked_ref(),
                        )
                        .unwrap_or_else(|_| error!("Failed to remove event listener"));
                }
                true
            }
            Msg::KeyPress(e) => {
                if !self.game_over {
                    if let Some(ws_write) = &self.ws_write {
                        let direction = match e.key().as_str() {
                            "ArrowUp" | "w" | "W" | "i" | "I" | "8" | "Numpad8" => Some(Direction::Up),
                            "ArrowDown" | "s" | "S" | "k" | "K" | "2" | "Numpad2" => Some(Direction::Down),
                            "ArrowLeft" | "a" | "A" | "j" | "J" | "4" | "Numpad4" => Some(Direction::Left),
                            "ArrowRight" | "d" | "D" | "l" | "L" | "6" | "Numpad6" => Some(Direction::Right),
                            _ => None,
                        };

                        if let Some(dir) = direction {
                            e.prevent_default();
                            
                            // If waiting for first key press, set waiting_for_first_key to false
                            if self.waiting_for_first_key {
                                self.waiting_for_first_key = false;
                                
                                // If we have a game state, set it to started
                                if let Some(game) = &mut self.game_state {
                                    game.started = true;
                                }
                            }
                            
                            let msg = SnakeMessage::ChangeDirection(dir);
                            if let Ok(text) = serde_json::to_string(&msg) {
                                let ws_write = ws_write.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Err(e) = ws_write.lock().await.send(Message::Text(text)).await {
                                        error!("Failed to send direction: {:?}", e);
                                    }
                                });
                            }
                        }
                    }
                }
                true
            }
            Msg::SwipeDirection(dir) => {
                if !self.game_over {
                    if let Some(ws_write) = &self.ws_write {
                        // If waiting for first key press, set waiting_for_first_key to false
                        if self.waiting_for_first_key {
                            self.waiting_for_first_key = false;
                            
                            // If we have a game state, set it to started
                            if let Some(game) = &mut self.game_state {
                                game.started = true;
                            }
                        }
                        
                        let msg = SnakeMessage::ChangeDirection(dir);
                        if let Ok(text) = serde_json::to_string(&msg) {
                            let ws_write = ws_write.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Err(e) = ws_write.lock().await.send(Message::Text(text)).await {
                                    error!("Failed to send direction: {:?}", e);
                                }
                            });
                        }
                    }
                }
                true
            }
            Msg::ArrowButtonClick(dir) => {
                if !self.game_over {
                    if let Some(ws_write) = &self.ws_write {
                        // If waiting for first key press, set waiting_for_first_key to false
                        if self.waiting_for_first_key {
                            self.waiting_for_first_key = false;
                            
                            // If we have a game state, set it to started
                            if let Some(game) = &mut self.game_state {
                                game.started = true;
                            }
                        }
                        
                        let msg = SnakeMessage::ChangeDirection(dir);
                        if let Ok(text) = serde_json::to_string(&msg) {
                            let ws_write = ws_write.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Err(e) = ws_write.lock().await.send(Message::Text(text)).await {
                                    error!("Failed to send direction: {:?}", e);
                                }
                            });
                        }
                    }
                }
                true
            }
            Msg::Disconnect => {
                info!("Disconnecting WebSocket");
                if let Some(ws_write) = self.ws_write.take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(e) = ws_write.lock().await.close().await {
                            error!("Error closing WebSocket: {:?}", e);
                        }
                    });
                }
                self.game_state = None;
                true
            }
            Msg::StartGame => {
                info!("Starting new game");
                self.waiting_for_first_key = true;
                ctx.link().send_message(Msg::Connect);
                true
            }
            Msg::ConnectionError(error) => {
                error!("Connection error: {}", error);
                self.error_message = Some(error);
                true
            }
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        info!("Destroying FrontendSnakeGame component");
        
        // Clean up WebSocket
        if let Some(ws_write) = self.ws_write.take() {
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = ws_write.lock().await.close().await {
                    error!("Error closing WebSocket: {:?}", e);
                }
            });
        }
        
        // Clean up keyboard event listeners
        if let Some(listener) = self._keydown_listener.take() {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "keydown",
                    listener.as_ref().unchecked_ref(),
                );
            }
        }
        
        if let Some(listener) = self._keypress_listener.take() {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "keypress",
                    listener.as_ref().unchecked_ref(),
                );
            }
        }
        
        // Clean up touch event listeners
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(listener) = self._touchstart_listener.take() {
                    let _ = document.remove_event_listener_with_callback(
                        "touchstart",
                        listener.as_ref().unchecked_ref(),
                    );
                }
                
                if let Some(listener) = self._touchend_listener.take() {
                    let _ = document.remove_event_listener_with_callback(
                        "touchend",
                        listener.as_ref().unchecked_ref(),
                    );
                }
                
                if let Some(listener) = self._touchmove_listener.take() {
                    let _ = document.remove_event_listener_with_callback(
                        "touchmove",
                        listener.as_ref().unchecked_ref(),
                    );
                }
            }
        }
    }
}

impl FrontendSnakeGame {
    fn render_game(&self) {
        if let Some(canvas) = self.canvas_ref.cast::<HtmlCanvasElement>() {
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();

            // Clear background
            context.set_fill_style_str("#1a1a1a");
            context.fill_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

            if let Some(game) = &self.game_state {
                let cell_width = canvas.width() as f64 / game.grid_size.0 as f64;
                let cell_height = canvas.height() as f64 / game.grid_size.1 as f64;

                // Draw snake
                context.set_fill_style_str("#50fa7b");
                for pos in &game.snake {
                    context.fill_rect(
                        pos.x as f64 * cell_width,
                        pos.y as f64 * cell_height,
                        cell_width - 1.0,
                        cell_height - 1.0,
                    );
                }

                // Draw food with appropriate color based on type
                match game.food.food_type {
                    FoodType::Regular => context.set_fill_style_str("#ff5555"),  // Red
                    FoodType::Scroll => context.set_fill_style_str("#ffff00"),   // Yellow
                };
                
                context.fill_rect(
                    game.food.position.x as f64 * cell_width,
                    game.food.position.y as f64 * cell_height,
                    cell_width - 1.0,
                    cell_height - 1.0,
                );
            }

            // Display "Press WASD or arrow keys to begin" in the center of the game window
            if self.waiting_for_first_key {
                context.set_fill_style_str("rgba(0, 0, 0, 0.7)");
                context.fill_rect(0.0, 150.0, canvas.width() as f64, 100.0);
                
                context.set_font("16px Arial");
                context.set_text_align("center");
                context.set_text_baseline("middle");
                context.set_fill_style_str("#ffffff");
                context.fill_text("WASD, IJKL, arrow keys, numpad,", canvas.width() as f64 / 2.0, canvas.height() as f64 / 2.0 - 10.0).unwrap();
                context.fill_text("swipe on mobile, or use keypad below", canvas.width() as f64 / 2.0, canvas.height() as f64 / 2.0 + 15.0).unwrap();
            }
        }
    }
}

// Helper function to check if the event target is part of the game canvas
fn is_game_canvas_element(target: Option<web_sys::EventTarget>) -> bool {
    if let Some(target) = target {
        if let Some(element) = target.dyn_ref::<web_sys::Element>() {
            // Check if the element is the canvas or a parent of the canvas
            if let Ok(closest) = element.closest("canvas") {
                return closest.is_some();
            }
            
            // Also check for the game container
            if let Ok(closest) = element.closest(".game-container") {
                return closest.is_some();
            }
            
            // Check for the specific canvas by ID if it has one
            if let Ok(closest) = element.closest("#snake-game-canvas") {
                return closest.is_some();
            }
        }
    }
    false
} 
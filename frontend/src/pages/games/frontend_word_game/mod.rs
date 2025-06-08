mod dictionary_lookup;
mod game_board;
mod cooldown;
mod keyboard;

use yew::prelude::*;
use gloo_net::http::Request;
use web_sys::window;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::callback::Interval;
use crate::config::get_api_base_url;
use crate::pages::games::word_leaderboard::WordLeaderboard;
use crate::hooks::use_membership::use_membership;
use crate::components::membership_required::MembershipRequired;
use shared::shared_word_game::{PublicWordGame, NewWordGameResponse, GuessResponse, LetterTile};
use gloo::console::log;
use wasm_bindgen::JsValue;

use dictionary_lookup::DictionaryLookup;
use game_board::GameBoard;
use cooldown::{CooldownDisplay, CooldownState, CooldownStatus, format_time};
use keyboard::Keyboard;

// Add constant for game timer duration (in seconds)
const GAME_TIMER_SECONDS: f64 = 900.0; // 15 minutes

fn get_auth_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
}

// Updated fetch_active_game
fn fetch_active_game(
    game_state: UseStateHandle<Option<PublicWordGame>>,
    session_id: UseStateHandle<Option<String>>,
    session_sig: UseStateHandle<Option<String>>,
    feedback: UseStateHandle<String>,
    session_start: UseStateHandle<Option<f64>>,
    guess_history: UseStateHandle<Vec<Vec<LetterTile>>>,
    cooldown_state: UseStateHandle<CooldownState>
) {
    let token = match get_auth_token() {
        Some(token) => token,
        None => {
            feedback.set("Authentication required. Please log in.".to_string());
            return;
        }
    };

    spawn_local(async move {
        let url = format!("{}/word-game/active", get_api_base_url());
        web_sys::console::log_1(&format!("Making request to: {}", url).into());
        
        match Request::get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await
        {
            Ok(response) => {
                web_sys::console::log_1(&format!("Response status: {}", response.status()).into());
                
                if response.status() == 200 {
                    match response.json::<NewWordGameResponse>().await {
                        Ok(data) => {
                            web_sys::console::log_1(&"Successfully parsed response data".into());
                            session_id.set(Some(data.session_id));
                            session_sig.set(Some(data.session_signature));
                            
                            // Initialize guess history from game data
                            let mut history = Vec::new();
                            for tiles in &data.game.tiles_history {
                                history.push(tiles.clone());
                            }
                            guess_history.set(history);
                            
                            // Set game state after initializing history
                            game_state.set(Some(data.game.clone()));
                            
                            // Calculate the correct session start time based on the game's created_at timestamp
                            if let Some(created_at) = data.game.created_at {
                                let now = js_sys::Date::now() / 1000.0; // Current time in seconds
                                let elapsed = now - created_at as f64; // Elapsed time in seconds
                                let adjusted_start = js_sys::Date::now() - (elapsed * 1000.0); // Adjust start time
                                session_start.set(Some(adjusted_start));
                            } else {
                                // Fallback to current time if created_at is not available
                                session_start.set(Some(js_sys::Date::now()));
                            }
                            
                            feedback.set("".to_string());
                        },
                        Err(e) => {
                            web_sys::console::log_1(&format!("Error parsing response: {:?}", e).into());
                            feedback.set("Failed to parse game data.".to_string());
                        }
                    }
                } else if response.status() == 404 {
                    // No active game found, this is normal
                    web_sys::console::log_1(&"No active game found, this is normal".into());
                    game_state.set(None);
                    session_id.set(None);
                    session_sig.set(None);
                    session_start.set(None);
                    feedback.set("".to_string());
                } else if response.status() == 429 {
                    // Rate limited - check cooldown status
                    web_sys::console::log_1(&"Rate limited, checking cooldown status".into());
                    fetch_cooldown_status(cooldown_state);
                } else {
                    web_sys::console::log_1(&format!("Unexpected status: {}", response.status()).into());
                    feedback.set("Server error. Please try again.".to_string());
                }
            },
            Err(e) => {
                web_sys::console::log_1(&format!("Network error: {:?}", e).into());
                feedback.set("Network error. Please check your connection.".to_string());
            }
        }
    });
}

fn fetch_cooldown_status(cooldown_state: UseStateHandle<CooldownState>) {
    let token = match get_auth_token() {
        Some(token) => token,
        None => return,
    };
    
    let cooldown_state = cooldown_state.clone();

    spawn_local(async move {
        let url = format!("{}/word-game/cooldown", get_api_base_url());
        web_sys::console::log_1(&format!("Making cooldown request to: {}", url).into());
        
        cooldown_state.set(CooldownState {
            time: 0,
            is_win_cooldown: false,
            is_loading: true,
            requires_membership: false,
        });
        
        match Request::get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == 200 {
                    if let Ok(status) = response.json::<CooldownStatus>().await {
                        if status.in_cooldown {
                            log!("In cooldown: ", status.remaining_seconds.unwrap_or(0), "seconds, win cooldown:", status.is_win_cooldown);
                            cooldown_state.set(CooldownState {
                                time: status.remaining_seconds.unwrap_or(0),
                                is_win_cooldown: status.is_win_cooldown,
                                is_loading: false,
                                requires_membership: status.requires_membership,
                            });
                        } else {
                            cooldown_state.set(CooldownState::default());
                        }
                    } else {
                        cooldown_state.set(CooldownState::default());
                    }
                } else {
                    cooldown_state.set(CooldownState::default());
                }
            },
            Err(_) => {
                cooldown_state.set(CooldownState::default());
            }
        }
    });
}

// Add this new function to fetch the solution when the game times out
fn fetch_solution(
    session_id_val: String,
    session_sig_val: String,
    game_state_clone: UseStateHandle<Option<PublicWordGame>>,
    feedback_clone: UseStateHandle<String>,
    cooldown_state_clone: UseStateHandle<CooldownState>
) {
    let token = match get_auth_token() {
        Some(token) => token,
        None => {
            feedback_clone.set("Authentication required. Please log in.".to_string());
            return;
        }
    };
    
    spawn_local(async move {
        // Use the refresh endpoint to get the current game state with the solution
        let url = format!("{}/word-game/refresh?session_id={}", get_api_base_url(), session_id_val);
        
        match Request::get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .header("X-Session-Signature", &session_sig_val)
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == 200 {
                    if let Ok(data) = response.json::<shared::shared_word_game::RefreshResponse>().await {
                        // Update the game state with the solution from the server
                        if let Some(game) = &*game_state_clone {
                            let mut updated_game = game.clone();
                            updated_game.solution = data.game.solution;
                            updated_game.remaining_guesses = 0;
                            game_state_clone.set(Some(updated_game));
                        }
                    }
                }
                
                // Check cooldown status
                fetch_cooldown_status(cooldown_state_clone);
            },
            Err(_) => {
                // If we can't fetch the solution, at least show a better message
                if let Some(game) = &*game_state_clone {
                    let mut updated_game = game.clone();
                    updated_game.solution = Some("(couldn't fetch solution)".to_string());
                    updated_game.remaining_guesses = 0;
                    game_state_clone.set(Some(updated_game));
                }
                
                // Check cooldown status
                fetch_cooldown_status(cooldown_state_clone);
            }
        }
    });
}

#[function_component(FrontendWordGame)]
pub fn frontend_word_game() -> Html {
    let membership = use_membership();
    
    // Game state
    let game_state = use_state(|| None::<PublicWordGame>);
    let session_id = use_state(|| None::<String>);
    let session_sig = use_state(|| None::<String>);
    let feedback = use_state(|| String::new());
    let is_loading = use_state(|| false);
    let session_start = use_state(|| None::<f64>);
    let time_left = use_state(|| GAME_TIMER_SECONDS);
    let guess_history = use_state(|| Vec::<Vec<LetterTile>>::new());
    let cooldown_state = use_state(CooldownState::default);
    let current_guess = use_state(String::new);
    
    // Timer for game countdown
    {
        let time_left = time_left.clone();
        let session_start = session_start.clone();
        let game_state = game_state.clone();
        let feedback = feedback.clone();
        let session_id = session_id.clone();
        let session_sig = session_sig.clone();
        let cooldown_state = cooldown_state.clone();
        
        use_effect_with((*session_start).clone(), move |_| {
            // Immediately calculate the time left based on session_start
            if let Some(start_time) = *session_start {
                let elapsed = (js_sys::Date::now() - start_time) / 1000.0;
                let remaining = (GAME_TIMER_SECONDS - elapsed).max(0.0);
                time_left.set(remaining);
            }
            
            let interval = if session_start.is_some() && game_state.is_some() {
                let game_state_clone = game_state.clone();
                let feedback_clone = feedback.clone();
                let session_id_clone = session_id.clone();
                let session_sig_clone = session_sig.clone();
                let session_start_clone = session_start.clone();
                let cooldown_state_clone = cooldown_state.clone();
                
                let interval_handle = Interval::new(1000, move || {
                    if let Some(start_time) = *session_start {
                        // Check if the game is still active
                        let game_is_active = if let Some(game) = &*game_state_clone {
                            !game.solved && game.remaining_guesses > 0
                        } else {
                            false
                        };
                        
                        // Only update the timer if the game is still active or if time is left
                        let elapsed = (js_sys::Date::now() - start_time) / 1000.0;
                        let remaining = (GAME_TIMER_SECONDS - elapsed).max(0.0);
                        
                        // Only update the timer if the game is still active or we're showing the time's up message
                        if game_is_active || remaining <= 0.0 {
                            time_left.set(remaining);
                        }
                        
                        // End game if timer reaches 0
                        if remaining <= 0.0 && game_state_clone.is_some() {
                            // Game timed out
                            if let Some(game) = &*game_state_clone {
                                if !game.solved {
                                    // If we already have the solution, use it
                                    if let Some(_solution) = &game.solution {
                                        // Don't set feedback message here, it will be redundant
                                        feedback_clone.set("".to_string());
                                        
                                        // Set remaining guesses to 0 to indicate game over
                                        let mut updated_game = game.clone();
                                        updated_game.remaining_guesses = 0;
                                        game_state_clone.set(Some(updated_game));
                                    } else {
                                        // If we don't have the solution, fetch it from the server
                                        if let (Some(id), Some(sig)) = (session_id_clone.as_ref().clone(), session_sig_clone.as_ref().clone()) {
                                            fetch_solution(
                                                id.clone(),
                                                sig.clone(),
                                                game_state_clone.clone(),
                                                feedback_clone.clone(),
                                                cooldown_state_clone.clone()
                                            );
                                        } else {
                                            // Fallback if we don't have session info
                                            feedback_clone.set("".to_string());
                                            let mut updated_game = game.clone();
                                            updated_game.solution = Some("(session expired)".to_string());
                                            updated_game.remaining_guesses = 0;
                                            game_state_clone.set(Some(updated_game));
                                        }
                                    }
                                    
                                    // Reset session state
                                    session_id_clone.set(None);
                                    session_sig_clone.set(None);
                                    session_start_clone.set(None);
                                }
                            }
                        }
                    }
                });
                Some(interval_handle)
            } else {
                None
            };
            
            move || {
                if let Some(_) = interval {
                    // Interval will be dropped automatically
                }
            }
        });
    }
    
    // Cooldown timer
    {
        let cooldown_state = cooldown_state.clone();
        
        use_effect_with((*cooldown_state).clone(), move |_| {
            let interval = if cooldown_state.time > 0 {
                let cooldown_state_clone = cooldown_state.clone();
                let interval_handle = Interval::new(1000, move || {
                    let mut current = (*cooldown_state_clone).clone();
                    if current.time > 0 {
                        current.time -= 1;
                        cooldown_state_clone.set(current);
                    }
                });
                Some(interval_handle)
                } else {
                None
            };
            
            move || {
                if let Some(_) = interval {
                    // Interval will be dropped automatically
                }
            }
        });
    }
    
    // Initialize game on component mount
    {
        let game_state = game_state.clone();
        let session_id = session_id.clone();
        let session_sig = session_sig.clone();
        let feedback = feedback.clone();
        let session_start = session_start.clone();
        let guess_history = guess_history.clone();
        let cooldown_state = cooldown_state.clone();

        use_effect_with((), move |_| {
            fetch_cooldown_status(cooldown_state.clone());
            
            if *cooldown_state == CooldownState::default() {
            fetch_active_game(
                    game_state.clone(),
                    session_id.clone(),
                    session_sig.clone(),
                    feedback.clone(),
                    session_start.clone(),
                    guess_history.clone(),
                    cooldown_state.clone()
                );
            }
            
            || {}
        });
    }
    
    // Handle guess submission
    let on_submit_guess = {
        let current_guess = current_guess.clone();
        let game_state = game_state.clone();
        let session_id = session_id.clone();
        let session_sig = session_sig.clone();
        let feedback = feedback.clone();
        let is_loading = is_loading.clone();
        let guess_history = guess_history.clone();
        let cooldown_state = cooldown_state.clone();
        
        Callback::from(move |_| {
            let guess = (*current_guess).clone();
            if guess.trim().is_empty() {
                return;
            }
            
            // Get the expected word length from the game state
            let expected_length = match &*game_state {
                Some(game) => game.word_length,
                None => {
                    feedback.set("No active game. Please start a new game.".to_string());
                    return;
                }
            };
            
            // Client-side validation for word length
            if guess.trim().len() != expected_length {
                feedback.set(format!("Guess must be {} letters long", expected_length));
                
                // Auto-hide feedback after 3 seconds
                let feedback_clone = feedback.clone();
                let message = format!("Guess must be {} letters long", expected_length);
                spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(3000).await;
                    if &*feedback_clone == &message {
                        feedback_clone.set("".to_string());
                    }
                });
                
                return; // Don't submit the guess if length doesn't match
            }
            
            let token = match get_auth_token() {
                Some(token) => token,
                None => {
                    feedback.set("Authentication required. Please log in.".to_string());
                    return;
                }
            };
            
            let session_id_val = match &*session_id {
                Some(id) => id.clone(),
                None => {
                    feedback.set("No active session. Please start a new game.".to_string());
                    return;
                }
            };
            
            let session_sig_val = match &*session_sig {
                Some(sig) => sig.clone(),
                None => {
                    feedback.set("No session signature. Please start a new game.".to_string());
                    return;
                }
            };
            
            is_loading.set(true);
            
            let game_state_clone = game_state.clone();
            let feedback_clone = feedback.clone();
            let is_loading_clone = is_loading.clone();
            let guess_history_clone = guess_history.clone();
            let cooldown_state_clone = cooldown_state.clone();
            let current_guess_clone = current_guess.clone();
            
            spawn_local(async move {
                let payload = serde_json::json!({
                    "session_id": session_id_val,
                    "guess": guess
                });
                
                log!("Submitting guess:", &guess);
                
                match Request::post(&format!("{}/word-game/guess", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .header("X-Session-Signature", &session_sig_val)
                    .json(&payload)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            match response.json::<GuessResponse>().await {
                                Ok(data) => {
                                    // Update game state and guess history together
                                    let mut history = (*guess_history_clone).clone();
                                    history.push(data.tiles.clone());
                                    
                                    // Batch updates to reduce flickering
                                    is_loading_clone.set(false);
                                    guess_history_clone.set(history);
                                    
                                    // Only update game state if needed to avoid full re-render
                                    let current_game = (*game_state_clone).clone();
                                    if let Some(current) = current_game {
                                        let mut new_game = data.game.clone();
                                        
                                        // Create a new updated game object with only the changed fields
                                        let mut updated_game = current.clone();
                                        
                                        // Preserve the solution if it exists in the current game and not in the new game
                                        if let Some(ref solution) = current.solution {
                                            if new_game.solution.is_none() {
                                                new_game.solution = Some(solution.clone());
                                            }
                                        }
                                        
                                        // Only update specific fields that changed to minimize re-renders
                                        updated_game.guesses = new_game.guesses;
                                        updated_game.remaining_guesses = new_game.remaining_guesses;
                                        updated_game.solved = new_game.solved;
                                        
                                        // If the game is solved or over, update the solution
                                        if new_game.solved || new_game.remaining_guesses == 0 {
                                            updated_game.solution = new_game.solution;
                                        }
                                        
                                        game_state_clone.set(Some(updated_game));
                                    } else {
                                        game_state_clone.set(Some(data.game.clone()));
                                    }
                                    
                                    // Show feedback only if it's not the default "incorrect" message or the redundant "No more guesses left" message
                                    let message = data.message.clone();
                                    if message != "Incorrect guess. Try again." && !message.starts_with("No more guesses left. The word was") {
                                        feedback_clone.set(message.clone());
                                        
                                        // Auto-hide feedback after 3 seconds
                                        let feedback_inner = feedback_clone.clone();
                                        spawn_local(async move {
                                            gloo_timers::future::TimeoutFuture::new(3000).await;
                                            if &*feedback_inner == &message {
                                                feedback_inner.set("".to_string());
                                            }
                                        });
                                    } else {
                                        feedback_clone.set("".to_string());
                                    }
                                    
                                    // If game is solved or all guesses used, handle game completion
                                    if data.game.solved || data.game.remaining_guesses == 0 {
                                        // If game is solved, dispatch currency event
                                        if data.correct && data.new_balance.is_some() {
                                            // Dispatch with the amount added (25) instead of the total balance
                                            let amount_added = 25; // The fixed amount added for a correct guess
                                            
                                            // Create a custom event with an object that has an 'amount' property
                                            if let Some(window) = window() {
                                                let event_name = "currencyUpdate";
                                                let init = web_sys::CustomEventInit::new();
                                                
                                                let detail = js_sys::Object::new();
                                                let _ = js_sys::Reflect::set(
                                                    &detail,
                                                    &JsValue::from_str("amount"),
                                                    &JsValue::from_f64(amount_added as f64),
                                                );
                                                
                                                init.set_detail(&detail);
                                                
                                                if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(event_name, &init) {
                                                    let _ = window.dispatch_event(&event);
                                                }
                                            }
                                            
                                            // Remove the redundant feedback message
                                            feedback_clone.set("".to_string());
                                        } else if data.game.remaining_guesses == 0 {
                                            // Don't set feedback here, it's already shown in the game status area
                                        }
                                        
                                        // Check cooldown status after a short delay
                                        let cooldown_inner = cooldown_state_clone.clone();
                                        spawn_local(async move {
                                            gloo_timers::future::TimeoutFuture::new(500).await;
                                            fetch_cooldown_status(cooldown_inner);
                                        });
                                    }
                                    
                                    // Only clear the current guess if the submission was successful
                                    // and there was no validation error
                                    if !data.message.contains("must be") && 
                                       !data.message.contains("must contain only alphabetic") && 
                                       !data.message.contains("You already guessed that") {
                                        let current_guess_inner = current_guess_clone.clone();
                                        current_guess_inner.set(String::new());
                                    }
                                },
                                Err(_) => {
                                    feedback_clone.set("Failed to parse response.".to_string());
                                }
                            }
                        } else if response.status() == 400 {
                            // Get the response text to show a more detailed error message
                            match response.text().await {
                                Ok(error_text) => {
                                    // Check if the error message contains information about the required word length
                                    if error_text.contains("must be") && error_text.contains("letters long") {
                                        feedback_clone.set(error_text);
                                    } else {
                                        feedback_clone.set("Invalid guess. Please try again.".to_string());
                                    }
                                },
                                Err(_) => {
                                    feedback_clone.set("Invalid guess. Please try again.".to_string());
                                }
                            }
                        } else {
                            feedback_clone.set("Server error. Please try again.".to_string());
                        }
                    },
                    Err(_) => {
                        feedback_clone.set("Network error. Please check your connection.".to_string());
                    }
                }
                
                is_loading_clone.set(false);
            });
        })
    };

    // Start new game
    let on_new_game = {
        let game_state = game_state.clone();
        let session_id = session_id.clone();
        let session_sig = session_sig.clone();
        let feedback = feedback.clone();
        let session_start = session_start.clone();
        let guess_history = guess_history.clone();
        let cooldown_state = cooldown_state.clone();
        let is_loading = is_loading.clone();
        
        Callback::from(move |_| {
            // Create a closure that captures the variables by reference
            let game_state = game_state.clone();
            let session_id = session_id.clone();
            let session_sig = session_sig.clone();
            let feedback = feedback.clone();
            let session_start = session_start.clone();
            let guess_history = guess_history.clone();
            let cooldown_state = cooldown_state.clone();
            let is_loading = is_loading.clone();
            
            let token = match get_auth_token() {
                Some(token) => token,
                None => {
                    feedback.set("Authentication required. Please log in.".to_string());
                    return;
                }
            };
            
            let url = format!("{}/word-game/new", get_api_base_url());
            web_sys::console::log_1(&format!("Creating new game at: {}", url).into());
            
            // Set loading state
            is_loading.set(true);
            
            spawn_local(async move {
                match Request::post(&url)
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            match response.json::<NewWordGameResponse>().await {
                                Ok(data) => {
                                    // Clear previous game state
                                    session_id.set(Some(data.session_id));
                                    session_sig.set(Some(data.session_signature));
                                    session_start.set(Some(js_sys::Date::now()));
                                    
                                    // Reset guess history
                                    guess_history.set(Vec::new());
                                    
                                    // Set game state
                                    game_state.set(Some(data.game));
                                    
                                    // Clear feedback
                                    feedback.set("".to_string());
                                    
                                    // Reset cooldown state
                                    cooldown_state.set(CooldownState::default());
                                    
                                    web_sys::console::log_1(&"New game created successfully".into());
                                },
                                Err(err) => {
                                    web_sys::console::log_1(&format!("Error parsing new game response: {:?}", err).into());
                                    feedback.set("Error creating new game. Please try again.".to_string());
                                }
                            }
                        } else if response.status() == 429 {
                            // Rate limited
                            feedback.set("You're creating games too quickly. Please wait a moment.".to_string());
                            
                            // Check cooldown status
                            fetch_cooldown_status(cooldown_state.clone());
                        } else {
                            // Other error
                            feedback.set(format!("Error creating new game: status {}", response.status()));
                        }
                    },
                    Err(err) => {
                        web_sys::console::log_1(&format!("Network error creating new game: {:?}", err).into());
                        feedback.set("Network error. Please check your connection and try again.".to_string());
                    }
                }
                
                // Clear loading state
                is_loading.set(false);
            });
        })
    };

    // Add these callbacks for the keyboard
    let on_key_press = {
        let current_guess = current_guess.clone();
        let game_state_clone = game_state.clone(); // Clone game_state before using it
        
        Callback::from(move |key: char| {
            // Get the word length from the game state
            let word_length = if let Some(game) = &*game_state_clone {
                game.word_length
            } else {
                5 // Default
            };
            
            if (*current_guess).len() < word_length {
                let new_guess = format!("{}{}", *current_guess, key);
                current_guess.set(new_guess);
            }
        })
    };

    let on_backspace = {
        let current_guess = current_guess.clone();
        
        Callback::from(move |_| {
            if !(*current_guess).is_empty() {
                let new_guess = (*current_guess).chars().take((*current_guess).chars().count() - 1).collect::<String>();
                current_guess.set(new_guess);
            }
        })
    };

    // Add a callback to update the current_guess from the GameBoard
    let on_guess_change = {
        let current_guess = current_guess.clone();
        
        Callback::from(move |new_guess: String| {
            current_guess.set(new_guess);
        })
    };

    // Function to reset game state
    let reset_game_state = {
        let game_state = game_state.clone();
        let session_id = session_id.clone();
        let session_sig = session_sig.clone();
        let session_start = session_start.clone();
        let guess_history = guess_history.clone();
        let feedback = feedback.clone();
        let cooldown_state = cooldown_state.clone();
        
        Callback::from(move |_| {
            game_state.set(None);
            session_id.set(None);
            session_sig.set(None);
            session_start.set(None);
            guess_history.set(vec![]);
            feedback.set("".to_string());
            
            // Check cooldown status after resetting
            fetch_cooldown_status(cooldown_state.clone());
        })
    };

    html! {
        <div class="flex flex-col items-center w-full max-w-4xl mx-auto">
            <div class="w-full">
                <h1 class="text-2xl sm:text-3xl font-bold mb-4 sm:mb-6 text-center text-gray-900 dark:text-white">{"Word Game"}</h1>
                
                {
                    // Display loading spinner
                    if membership.loading {
                        html! {
                            <div class="flex justify-center items-center h-64">
                                <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500"></div>
                            </div>
                        }
                    } 
                    // Display active game for members
                    else if membership.is_member && game_state.is_some() {
                        let game = game_state.as_ref().unwrap();
                        html! {
                            <div class="w-[95%] mx-auto bg-white dark:bg-gray-800 p-4 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] mb-4">
                                {
                                    // Time and guesses at the top of the unified card
                                    if !game.solved && game.remaining_guesses > 0 && *time_left > 0.0 {
                                        html! {
                                            <div class="mb-4 text-xs sm:text-sm md:text-base text-gray-600 dark:text-gray-300 flex justify-between">
                                                <div>
                                                    <span class="font-medium">{"Time: "}</span>
                                                    { format_time(*time_left as i64) }
                                                </div>
                                                <div>
                                                    <span class="font-medium">{"Guesses: "}</span>
                                                    { game.remaining_guesses }
                                                </div>
                                            </div>
                                        }
                                    } else if game.solved {
                                        html! {
                                            <div class="mb-4 p-2 sm:p-3 rounded-lg bg-green-100 text-green-800 dark:bg-green-800 dark:text-green-100 text-center font-bold text-sm sm:text-base shadow-md">
                                                {"You solved it! You earned 25 pax and 1 scroll!"}
                                            </div>
                                        }
                                    } else if game.remaining_guesses == 0 || *time_left <= 0.0 {
                                        html! {
                                            <div class="mt-4 mb-4 text-center">
                                                <div class="mb-4 p-2 sm:p-3 rounded-lg bg-red-100 text-red-800 dark:bg-red-800 dark:text-red-100 text-center font-bold text-sm sm:text-base shadow-md">
                                                    {
                                                        if let Some(solution) = &game.solution {
                                                            if game.remaining_guesses > 0 && *time_left <= 0.0 {
                                                                format!("Time's up! The word was '{}'.", solution)
                                                            } else {
                                                                format!("Game over! The word was '{}'.", solution)
                                                            }
                                                        } else {
                                                            if game.remaining_guesses > 0 && *time_left <= 0.0 {
                                                                "Time's up! Game over.".to_string()
                                                            } else {
                                                                "Game over!".to_string()
                                                            }
                                                        }
                                                    }
                                                </div>
                                                <button 
                                                    onclick={reset_game_state.clone()}
                                                    class="px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white font-medium rounded-lg transition-colors duration-200"
                                                >
                                                    {"Try Again"}
                                                </button>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                
                                {
                                    // Display feedback message - hide "Incorrect guess" message and redundant "No more guesses left" and "Time's up!" messages
                                    if !(*feedback).is_empty() && 
                                       *feedback != "Incorrect guess. Try again." && 
                                       !(*feedback).starts_with("No more guesses left. The word was") &&
                                       !(*feedback).starts_with("Time's up!") {
                                        html! {
                                            <div class={classes!(
                                                "mb-4", "p-2", "sm:p-3", "rounded-lg", "text-sm", "sm:text-base", "shadow-md",
                                                if (*feedback).contains("Correct") || (*feedback).contains("solved") { 
                                                    "bg-green-100 text-green-800 dark:bg-green-800 dark:text-green-100" 
                                                } else if (*feedback).contains("Invalid") || (*feedback).contains("error") { 
                                                    "bg-red-100 text-red-800 dark:bg-red-800 dark:text-red-100" 
                                                } else { 
                                                    "bg-blue-100 text-blue-800 dark:bg-blue-800 dark:text-blue-100" 
                                                }
                                            )}>
                                                { &*feedback }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }

                                <GameBoard 
                                    game={game.clone()} 
                                    guess_history={(*guess_history).clone()} 
                                    is_loading={*is_loading} 
                                    on_submit={on_submit_guess.clone()}
                                    time_left={*time_left}
                                    current_guess={(*current_guess).clone()}
                                    on_guess_change={Some(on_guess_change)}
                                />
                                
                                <Keyboard 
                                    guess_history={(*guess_history).clone()} 
                                    on_key_press={Some(on_key_press)}
                                    on_backspace={Some(on_backspace)}
                                    on_enter={Some(on_submit_guess.clone())}
                                />

                                {
                                    // Dictionary lookup at the bottom of the unified card
                                    if !game.solved && game.remaining_guesses > 0 && *time_left > 0.0 && !*is_loading {
                                        html! {
                                            <>
                                                <div class="my-4 border-t border-gray-200 dark:border-gray-700"></div>
                                                <div key={format!("dict-{}", game.word_length)}>
                                                    <DictionaryLookup word_length={game.word_length} />
                                                </div>
                                            </>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        }
                    }
                    // Display cooldown for members
                    else if membership.is_member && *cooldown_state != CooldownState::default() {
                        html! {
                            <>
                                <div class="w-[95%] mx-auto mb-6">
                                    <CooldownDisplay cooldown_state={(*cooldown_state).clone()} />
                                </div>
                                
                                // Also show the game description when in cooldown
                                <div class="w-[95%] mx-auto mb-6 bg-white dark:bg-gray-800 p-4 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] text-left text-gray-700 dark:text-gray-300">
                                    <h2 class="text-xl font-bold mb-3 text-gray-900 dark:text-white">{"How to Play"}</h2>
                                    <ul class="space-y-3 list-disc list-inside text-base">
                                        <li>{"You have 15 minutes and 7 guesses to find the secret word. 
                                        Each word is randomly chosen from a list and can be from 4 to 8 letters long. 
                                        The word can be a proper noun. A dictionary lookup is available."}</li>
                                        <li>{"After each guess, you'll get visual feedback:"}</li>
                                        <ul class="ml-5 mt-1 space-y-1 list-disc list-inside">
                                            <li><span class="text-green-500 font-semibold">{"Green "}</span><span class="inline-block w-1.5 h-1.5 bg-green-500 rounded-full mx-0.5">{"  "}</span><span class="inline-block w-1.5 h-1.5 bg-green-500 rounded-full"></span>{" - In the word and correct position "}<span class="inline-flex gap-1 items-center"></span></li>
                                            <li><span class="text-yellow-500 font-semibold">{"Yellow "}</span><span class="inline-block w-1.5 h-1.5 bg-yellow-500 rounded-full"></span>{" - In the word but wrong position "}</li>
                                            <li><span class="text-red-500 font-semibold">{"Red"}</span>{" - Not in the word at all"}</li>
                                        </ul>
                                        <li>{"A correct guess rewards you with 25 pax and 1 scroll, followed by a 23-hour cooldown. 
                                        If you run out of guesses or time, the game ends and there's a 30-second cooldown before you can attempt again."}</li>
                                    </ul>
                                </div>
                            </>
                        }
                    }
                    // Display game description and start button or membership required
                    else {
                        html! {
                            <div class="w-[95%] mx-auto mb-6 bg-white dark:bg-gray-800 p-4 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] text-left text-gray-700 dark:text-gray-300">
                                <h2 class="text-xl font-bold mb-3 text-gray-900 dark:text-white">{"How to Play"}</h2>
                                <ul class="space-y-3 list-disc list-inside text-base">
                                    <li>{"You have 15 minutes and 7 guesses to find the secret word. 
                                    Each word is randomly chosen from a list and can be from 4 to 8 letters long. 
                                    The word can be a proper noun. A dictionary lookup is available."}</li>
                                    <li>{"After each guess, you'll get visual feedback:"}</li>
                                    <ul class="ml-5 mt-1 space-y-1 list-disc list-inside">
                                        <li><span class="text-green-500 font-semibold">{"Green "}</span><span class="inline-block w-1.5 h-1.5 bg-green-500 rounded-full mx-0.5">{"  "}</span><span class="inline-block w-1.5 h-1.5 bg-green-500 rounded-full"></span>{" - In the word and correct position "}<span class="inline-flex gap-1 items-center"></span></li>
                                        <li><span class="text-yellow-500 font-semibold">{"Yellow "}</span><span class="inline-block w-1.5 h-1.5 bg-yellow-500 rounded-full"></span>{" - In the word but wrong position "}</li>
                                        <li><span class="text-red-500 font-semibold">{"Red"}</span>{" - Not in the word at all"}</li>
                                    </ul>
                                    <li>{"A correct guess rewards you with 25 pax and 1 scroll, followed by a 23-hour cooldown. 
                                    If you run out of guesses or time, the game ends and there's a 30-second cooldown before you can attempt again."}</li>
                                </ul>
                                
                                <div class="flex flex-col items-center justify-center mt-6 pt-4 border-t border-gray-200 dark:border-gray-700">
                                    {
                                        if membership.is_member {
                                            html! {
                                                <>
                                                    <p class="mb-4 text-gray-600 dark:text-gray-300 text-center">{"Ready to play? Start a new word game!"}</p>
                                                    <button 
                                                        onclick={on_new_game.clone()} 
                                                        class="px-6 py-2 bg-blue-500 hover:bg-blue-600 text-white font-medium rounded-lg transition-colors duration-200"
                                                    >
                                                        {"New Game"}
                                                    </button>
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <MembershipRequired feature_name="Word Game" />
                                            }
                                        }
                                    }
                                </div>
                            </div>
                        }
                    }
                }
                
                <div class="w-[95%] mx-auto mt-8">
                    <WordLeaderboard update_trigger={(*game_state).is_some() as u32} />
                </div>
            </div>
        </div>
    }
}
use yew::prelude::*;
use gloo::net::http::Request;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use shared::shared_match_game::{Color, ColorVariant, PublicMatchGame};
use shared::shared_match_game::{NewGameResponse, RevealRequest, RevealResponse, RevealOneResponse};
use wasm_bindgen::JsValue;
use web_sys::CustomEvent;
use web_sys::CustomEventInit;
use web_sys::window;
use crate::config::get_api_base_url;

// Update color style function to handle shiny variants, but only when revealed
fn get_color_style(color: &Option<Color>, variant: &Option<ColorVariant>, is_revealed: bool) -> Vec<&'static str> {
    if !is_revealed {
        // Return ONLY basic card styling for unrevealed cards - using slate instead of gray
        return vec!["bg-slate-300", "dark:bg-slate-600", "text-gray-800", "dark:text-gray-200"];
    }

    let mut styles = match color {
        Some(Color::Red) => vec!["bg-red-500", "text-white"],
        Some(Color::Blue) => vec!["bg-blue-500", "text-white"],
        Some(Color::Green) => vec!["bg-green-700", "text-white"],
        Some(Color::Lime) => vec!["bg-lime-400", "text-white"],
        Some(Color::Purple) => vec!["bg-purple-500", "text-white"],
        Some(Color::Orange) => vec!["bg-orange-500", "text-white"],
        Some(Color::Pink) => vec!["bg-pink-500", "text-white"],
        Some(Color::Teal) => vec!["bg-teal-500", "text-white"],
        Some(Color::Gold) => vec!["bg-amber-400", "text-white"],
        None => vec!["bg-slate-300", "dark:bg-slate-600", "text-gray-800", "dark:text-gray-200"],
    };

    // Add shiny animation only for revealed shiny cards
    if let Some(ColorVariant::Shiny) = variant {
        styles.extend(&[
            "animate-pulse",
            "shadow-lg",
            "shadow-amber-200",
            "ring-2",
            "ring-amber-300",
            "border-2",
            "border-amber-200",
        ]);
    }

    styles
}

fn get_auth_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
}

#[function_component(FrontendMatchGame)]
pub fn frontend_match_game() -> Html {
    let game_state = use_state(|| None as Option<PublicMatchGame>);
    let session_id = use_state(|| None as Option<String>);
    let session_signature = use_state(|| None as Option<String>);
    let selected_indices = use_state(|| Vec::new());
    let is_processing = use_state(|| false);

    let on_new_game = {
        let game_state = game_state.clone();
        let session_id = session_id.clone();
        let session_signature = session_signature.clone();
        Callback::from(move |_| {
            let game_state = game_state.clone();
            let session_id = session_id.clone();
            let session_signature = session_signature.clone();
            spawn_local(async move {
                let token = get_auth_token();
                let api_base = get_api_base_url();
                
                // Fix: Remove /api prefix to match backend routing
                let url = if api_base.is_empty() {
                    "/match-game/new".to_string()
                } else {
                    format!("{}/match-game/new", api_base)
                };
                
                if let Ok(resp) = Request::post(&url)
                    .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                    .send()
                    .await
                {
                    if resp.status() == 200 {
                        match resp.json::<NewGameResponse>().await {
                            Ok(new_game_resp) => {
                                session_id.set(Some(new_game_resp.session_id));
                                session_signature.set(Some(new_game_resp.session_signature));
                                game_state.set(Some(new_game_resp.game));
                            },
                            Err(e) => {
                                log::error!("Failed to parse game state: {:?}", e);
                            }
                        }
                    } else {
                        log::error!("Server returned error status: {}", resp.status());
                    }
                }
            });
        })
    };

    let on_card_click = {
        let game_state = game_state.clone();
        let selected_indices = selected_indices.clone();
        let session_id = session_id.clone();
        let session_signature = session_signature.clone();
        let is_processing = is_processing.clone();
        Callback::from(move |index: usize| {
            if *is_processing {
                return;
            }

            // Add check for already revealed cards
            if let Some(game) = &*game_state {
                if game.cards[index].revealed || game.cards[index].matched {
                    log::info!("Card {} is already revealed or matched, ignoring click", index);
                    return;
                }
            }

            if let (Some(sid), Some(sig)) = ((*session_id).clone(), (*session_signature).clone()) {
                let current_selected = (*selected_indices).clone();
                // If no card is currently selected, this is the first card click
                if current_selected.is_empty() {
                    is_processing.set(true);
                    let game_state_inner = game_state.clone();
                    let selected_indices_inner = selected_indices.clone();
                    let is_processing_inner = is_processing.clone();
                    spawn_local(async move {
                        let token = get_auth_token();
                        let api_base = get_api_base_url();
                        
                        // Fix: Remove /api prefix to match backend routing
                        let url = if api_base.is_empty() {
                            format!("/match-game/reveal_one?session_id={}&card_index={}", sid, index)
                        } else {
                            format!("{}/match-game/reveal_one?session_id={}&card_index={}", api_base, sid, index)
                        };
                        
                        match Request::get(&url)
                            .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                            .header("X-Session-Signature", &sig)
                            .send()
                            .await {
                                Ok(resp) => {
                                    if resp.status() == 200 {
                                        let reveal_data: RevealOneResponse = resp.json().await.expect("Failed to parse reveal_one response");
                                        game_state_inner.set(Some(reveal_data.game));
                                        let mut new_selected = (*selected_indices_inner).clone();
                                        new_selected.push(index);
                                        selected_indices_inner.set(new_selected);
                                    } else {
                                        log::warn!("reveal_one returned error status: {}", resp.status());
                                    }
                                },
                                Err(e) => {
                                    log::error!("Network error in reveal_one: {:?}", e);
                                }
                        }
                        is_processing_inner.set(false);
                    });
                } else if current_selected.len() == 1 && current_selected[0] != index {  // Add check to prevent selecting same card twice
                    // Second card click, call the reveal endpoint as before
                    is_processing.set(true);
                    let first = current_selected[0];
                    let second = index;
                    let game_state_inner = game_state.clone();
                    let selected_indices_inner = selected_indices.clone();
                    let is_processing_inner = is_processing.clone();
                    spawn_local(async move {
                        let req = RevealRequest {
                            session_id: sid.clone(),
                            first_index: first,
                            second_index: second,
                        };

                        let token = get_auth_token();
                        let api_base = get_api_base_url();
                        
                        // Fix: Remove /api prefix to match backend routing
                        let url = if api_base.is_empty() {
                            "/match-game/reveal".to_string()
                        } else {
                            format!("{}/match-game/reveal", api_base)
                        };
                        
                        match Request::post(&url)
                            .header("Content-Type", "application/json")
                            .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                            .header("X-Session-Signature", &sig)
                            .body(serde_json::to_string(&req).unwrap())
                            .expect("Failed to build request")
                            .send()
                            .await {
                                Ok(resp) => {
                                    if resp.status() == 200 {
                                        let reveal_resp: RevealResponse = resp.json().await.expect("Failed to parse reveal response");
                                        game_state_inner.set(Some(reveal_resp.game));
                                        selected_indices_inner.set(vec![]);

                                        // If it was a match, dispatch currency update event
                                        if reveal_resp.match_found {
                                            if let Some(window) = window() {
                                                if let Some(new_balance) = reveal_resp.new_balance {
                                                    let event_init = CustomEventInit::new();
                                                    event_init.set_detail(&JsValue::from_f64(new_balance as f64));
                                                    let event = CustomEvent::new_with_event_init_dict(
                                                        "currencyUpdate",
                                                        &event_init
                                                    ).unwrap();
                                                    window.dispatch_event(&event).unwrap();
                                                }
                                            }
                                        }

                                        if !reveal_resp.match_found {
                                            // Wait 1 second before refreshing
                                            TimeoutFuture::new(1000).await;
                                            let api_base = get_api_base_url();
                                            
                                            // Fix: Remove /api prefix to match backend routing
                                            let url = if api_base.is_empty() {
                                                format!("/match-game/refresh?session_id={}", sid)
                                            } else {
                                                format!("{}/match-game/refresh?session_id={}", api_base, sid)
                                            };
                                            
                                            let token = get_auth_token();
                                            match Request::get(&url)
                                                .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                                                .header("X-Session-Signature", &sig)
                                                .send()
                                                .await {
                                                    Ok(refresh_resp) => {
                                                        if refresh_resp.status() == 200 {
                                                            let refresh_data: RevealOneResponse = refresh_resp.json().await.expect("Failed to parse refresh response");
                                                            game_state_inner.set(Some(refresh_data.game));
                                                        } else {
                                                            log::warn!("Refresh returned error status: {}", refresh_resp.status());
                                                        }
                                                    },
                                                    Err(e) => {
                                                        log::error!("Failed to refresh game state: {:?}", e);
                                                    }
                                            }
                                        }
                                    } else {
                                        log::warn!("Server returned error status: {}", resp.status());
                                    }
                                },
                                Err(e) => {
                                    log::error!("Network error in reveal request: {:?}", e);
                                }
                        }
                        is_processing_inner.set(false);
                    });
                } else if current_selected.len() == 1 && current_selected[0] == index {
                    // Trying to select the same card twice
                    log::info!("Can't select the same card twice");
                    selected_indices.set(vec![]);  // Reset selection
                    is_processing.set(false);
                }
            }
        })
    };

    html! {
        <div class="w-[95%] max-w-2xl mx-auto">
            <h1 class="text-3xl font-bold mb-6 text-center text-gray-900 dark:text-white">{ "Matching Game" }</h1>
            {
                if let Some(game) = &*game_state {
                    html! {
                        <div class="space-y-6">
                            {
                                if game.cards.iter().all(|card| card.matched) {
                                    html! {
                                        <div class="flex flex-col items-center space-y-4 p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)]">
                                            <div class="text-center">
                                                <p class="text-2xl font-bold text-green-500 dark:text-green-400">
                                                    { "🎉 You win 2 bonus pax! 🎉" }
                                                </p>
                                                <p class="mt-2 text-gray-600 dark:text-gray-400">
                                                    { "All pairs have been matched!" }
                                                </p>
                                            </div>
                                            <button 
                                                onclick={on_new_game}
                                                class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                                            >
                                                { "Play Again" }
                                            </button>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            // Fix: Using grid-cols-4 with explicit fixed sizing to enforce square aspect
                            <div class="grid grid-cols-4 gap-3 md:gap-4 w-full mx-auto">
                                { for game.cards.iter().enumerate().map(|(index, card)| {
                                    let on_click = {
                                        let on_card_click = on_card_click.clone();
                                        Callback::from(move |_| on_card_click.emit(index))
                                    };
                                    let display = if card.revealed || card.matched {
                                        "".to_string()
                                    } else {
                                        "?".to_string()
                                    };
                                    // Fix: More explicit styling to enforce square shape
                                    let card_classes = classes!(
                                        "aspect-square", 
                                        "w-full",
                                        "h-0",  // Add h-0 to enforce height calculation from width via aspect-ratio
                                        "pt-[100%]", // Padding trick to maintain aspect ratio
                                        "relative", // For absolute positioning of content
                                        "flex",
                                        "items-center",
                                        "justify-center",
                                        "rounded-lg",
                                        "cursor-pointer",
                                        "transition-all",
                                        "duration-300", 
                                        "text-xl",
                                        "font-bold",
                                        "shadow-md",
                                        get_color_style(&card.color, &card.variant, card.revealed || card.matched)
                                    );
                                    html! {
                                        <div onclick={on_click} class={card_classes}>
                                            // Fix: Absolute positioning of the question mark/content
                                            <div class="absolute inset-0 flex items-center justify-center">
                                                { display }
                                            </div>
                                        </div>
                                    }
                                }) }
                            </div>
                        </div>
                    }
                } else {
                    html! {
                        <div class="text-center p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)]">
                            <p class="mb-4 text-gray-700 dark:text-gray-300 space-y-2">
                                {"Match pairs of colored cards to win!
                                Rare chance (5%) to find Shiny Gold cards.
                                Matching Gold pairs = 1 scroll.
                                Completion = 2 pax bonus."}
                            </p>
                            <button 
                                onclick={on_new_game}
                                class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                            >
                                { "New Game" }
                            </button>
                        </div>
                    }
                }
            }
        </div>
    }
}
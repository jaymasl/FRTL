use yew::prelude::*;
use crate::models::Scroll;
use super::DisplayMode;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use serde::Deserialize;
use crate::models::Egg;
use gloo_timers::callback::Timeout;
use crate::config::get_asset_url;
use crate::config::get_api_base_url;

#[derive(Deserialize)]
struct GenerateEggResponse {
    new_balance: i32,
    egg: Egg,
}

#[derive(Properties, PartialEq)]
pub struct ScrollFocusProps {
    pub scroll: Scroll,
    pub on_close: Callback<()>,
    pub on_action: Option<Callback<()>>,
    pub action_label: Option<String>,
    pub loading: bool,
    pub error: String,
    pub mode: DisplayMode,
    #[prop_or_default]
    pub fetch_data: Option<Callback<()>>,
    #[prop_or_default]
    pub on_select_egg: Option<Callback<Egg>>,
}

#[function_component(ScrollFocus)]
pub fn scroll_focus(props: &ScrollFocusProps) -> Html {
    let loading = use_state(|| false);
    let error = use_state(String::new);
    let local_scroll = use_state(|| props.scroll.clone());
    
    let handle_summon = {
        let loading = loading.clone();
        let error = error.clone();
        let scroll_id = props.scroll.id;
        let fetch_data = props.fetch_data.clone();
        let on_select_egg = props.on_select_egg.clone();
        let on_close = props.on_close.clone();
        
        Callback::from(move |_| {
            loading.set(true);
            error.set(String::new());
            
            let token = {
                let window = window().unwrap();
                let local_token = window.local_storage().unwrap().unwrap()
                    .get_item("token").unwrap();
                let session_token = window.session_storage().unwrap().unwrap()
                    .get_item("token").unwrap();
                
                match (local_token, session_token) {
                    (Some(token), _) | (None, Some(token)) if !token.is_empty() => token,
                    _ => String::new()
                }
            };

            if token.is_empty() {
                error.set("Authentication error. Please log in again.".to_string());
                loading.set(false);
                return;
            }
            
            let loading = loading.clone();
            let error = error.clone();
            let fetch_data = fetch_data.clone();
            let on_select_egg = on_select_egg.clone();
            let on_close = on_close.clone();
            
            spawn_local(async move {
                match Request::post(&format!("{}/api/generator/generate-egg", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&serde_json::json!({ "scroll_id": scroll_id }))
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        match response.status() {
                            200 => {
                                if let Ok(data) = response.json::<GenerateEggResponse>().await {
                                    // Update currency in local storage
                                    if let Some(window) = window() {
                                        if let Some(storage) = window.local_storage().ok().flatten() {
                                            let _ = storage.set_item("currency", &data.new_balance.to_string());
                                        }
                                        let event_init = web_sys::CustomEventInit::new();
                                        event_init.set_detail(&wasm_bindgen::JsValue::from_f64(data.new_balance as f64));
                                        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("currencyUpdate", &event_init) {
                                            let _ = window.dispatch_event(&event);
                                        }
                                    }

                                    // Fix username race condition - add username to egg data before displaying
                                    let mut egg_with_username = data.egg.clone();
                                    
                                    // Get current username from localStorage
                                    if let Some(window) = window() {
                                        if let Some(storage) = window.local_storage().ok().flatten() {
                                            if let Ok(Some(username)) = storage.get_item("username") {
                                                egg_with_username.summoned_by_username = Some(username.clone());
                                                egg_with_username.owner_username = Some(username);
                                            }
                                        }
                                    }

                                    // Immediately select the new egg to show its focus view, if callback provided; otherwise close modal
                                    if let Some(on_select_egg) = on_select_egg {
                                        on_select_egg.emit(egg_with_username);
                                    } else {
                                        on_close.emit(());
                                    }

                                    // Finally update inventory data after a short delay
                                    if let Some(fetch_data) = fetch_data {
                                        Timeout::new(200, move || {
                                            fetch_data.emit(());
                                        }).forget();
                                    }
                                }
                            },
                            429 => error.set("Too many requests. Please try again later.".to_string()),
                            402 => error.set("Not enough pax. You need 55 pax to summon an egg.".to_string()),
                            401 => error.set("Please log in again.".to_string()),
                            _ => error.set("Failed to generate egg.".to_string()),
                        }
                    },
                    Err(_) => error.set("Network error occurred.".to_string()),
                }
                loading.set(false);
            });
        })
    };

    html! {
        <div class="flex flex-col items-center justify-center space-y-6 p-4 w-full">
            <div class="relative aspect-square rounded-2xl overflow-hidden bg-gradient-to-br from-blue-500/20 to-purple-500/20 w-full max-w-[450px]">
                <img 
                    src={
                        local_scroll.image_path.clone()
                            .map(|p| if p.starts_with("http") { p } else { get_asset_url(&p) })
                            .unwrap_or_else(|| get_asset_url("/static/images/scroll-default.avif"))
                    }
                    alt={local_scroll.display_name.clone()}
                    class="w-full h-full object-contain p-6"
                />
            </div>
            
            <div class="flex flex-col space-y-4 w-full max-w-[450px]">
                <button 
                    onclick={handle_summon}
                    disabled={*loading}
                    class={classes!(
                        "w-full",
                        "px-6",
                        "py-3",
                        "rounded-xl",
                        "font-semibold",
                        "shadow-lg",
                        "transition-all",
                        "duration-500",
                        if *loading {
                            "bg-gray-400 cursor-not-allowed"
                        } else {
                            "bg-gradient-to-r from-blue-500 to-purple-500 text-white hover:opacity-90"
                        }
                    )}
                >
                    {if *loading { "Summoning..." } else { "Summon New Egg (55 pax)" }}
                </button>
            </div>
            
            if !error.is_empty() {
                <p class="text-red-500 dark:text-red-400 text-sm">{&*error}</p>
            }
            
            if !props.error.is_empty() {
                <p class="text-red-500 dark:text-red-400 text-sm">{&props.error}</p>
            }
        </div>
    }
} 
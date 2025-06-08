use yew::prelude::*;
use gloo_net::http::Request;
use web_sys::window;
use gloo_timers::callback::Interval;
use serde::Deserialize;
use crate::config::get_api_base_url;
use crate::styles;
use wasm_bindgen_futures::spawn_local;
use chrono;

// Add a format_time function similar to the one in claim.rs
fn format_time(seconds: i32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

#[derive(Debug, Deserialize)]
struct LastClickInfo {
    username: String,
    clicked_at: String,
    reward_amount: i32,
}

#[derive(Debug, Deserialize)]
struct MagicButtonResponse {
    success: bool,
    reward_amount: Option<i32>,
    cooldown_remaining: i32,
    last_click: Option<Vec<LastClickInfo>>,
    new_balance: Option<i32>,
    total_clicks: i64,
}

#[function_component(MagicButton)]
pub fn magic_button() -> Html {
    let is_clicking = use_state(|| false);
    let cooldown = use_state(|| 0);
    let last_click = use_state(|| None::<Vec<LastClickInfo>>);
    let error = use_state(String::new);
    let show_reward = use_state(|| None::<i32>);
    let total_clicks = use_state(|| 0i64);

    // Function to check cooldown
    let check_cooldown = {
        let cooldown = cooldown.clone();
        let error = error.clone();
        let last_click = last_click.clone();
        let total_clicks = total_clicks.clone();

        move || {
            let token = match window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten() 
            {
                Some(token) => token,
                None => return,
            };

            let cooldown = cooldown.clone();
            let error = error.clone();
            let last_click = last_click.clone();
            let total_clicks = total_clicks.clone();

            spawn_local(async move {
                match Request::get(&format!("{}/api/magic-button/status", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<MagicButtonResponse>().await {
                                cooldown.set(data.cooldown_remaining);
                                last_click.set(data.last_click);
                                total_clicks.set(data.total_clicks);
                            }
                        }
                    }
                    Err(_) => error.set("Failed to check cooldown".to_string()),
                }
            });
        }
    };

    // Initial cooldown check
    {
        let check_cooldown = check_cooldown.clone();
        use_effect_with((), move |_| {
            check_cooldown();
            || ()
        });
    }

    // Regular cooldown updates
    {
        let cooldown = cooldown.clone();
        let check_cooldown = check_cooldown.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(1000, move || {
                if *cooldown > 0 {
                    cooldown.set((*cooldown).max(0) - 1);
                } else {
                    check_cooldown();
                }
            });
            || drop(interval)
        });
    }

    // Format timestamp function
    let format_timestamp = |timestamp: &str| {
        let parse_and_format = |dt: chrono::DateTime<chrono::FixedOffset>| {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(dt.with_timezone(&chrono::Utc));
            
            // Always display in hours format, regardless of duration
            let total_hours = duration.num_hours();
            let minutes = duration.num_minutes() % 60;
            
            if total_hours > 0 {
                format!("{}h {}m ago", total_hours, minutes)
            } else if minutes > 0 {
                format!("{}m ago", minutes)
            } else {
                "just now".to_string()
            }
        };
        
        // Try parsing as RFC3339 first
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) {
            return parse_and_format(dt);
        }
        
        // Fall back to custom format parsing
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S%.f %z") {
            let now = chrono::Utc::now();
            let naive_now = now.naive_utc();
            let duration = naive_now.signed_duration_since(dt);
            
            // Always display in hours format
            let total_hours = duration.num_hours();
            let minutes = duration.num_minutes() % 60;
            
            if total_hours > 0 {
                format!("{}h {}m ago", total_hours, minutes)
            } else if minutes > 0 {
                format!("{}m ago", minutes)
            } else {
                "just now".to_string()
            }
        } else {
            // If all parsing fails, just return the original timestamp
            timestamp.to_string()
        }
    };

    let handle_click = {
        let is_clicking = is_clicking.clone();
        let cooldown = cooldown.clone();
        let last_click = last_click.clone();
        let error = error.clone();
        let show_reward = show_reward.clone();
        let total_clicks = total_clicks.clone();

        Callback::from(move |_| {
            if *is_clicking || *cooldown > 0 {
                return;
            }

            is_clicking.set(true);
            error.set(String::new());

            // Get auth token
            let token = match window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten() 
            {
                Some(token) => token,
                None => {
                    error.set("Not authenticated".to_string());
                    is_clicking.set(false);
                    return;
                }
            };

            let is_clicking = is_clicking.clone();
            let cooldown = cooldown.clone();
            let last_click = last_click.clone();
            let error = error.clone();
            let show_reward = show_reward.clone();
            let total_clicks = total_clicks.clone();

            spawn_local(async move {
                match Request::post(&format!("{}/api/magic-button", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<MagicButtonResponse>().await {
                                if data.success {
                                    // Show reward animation
                                    if let Some(reward) = data.reward_amount {
                                        show_reward.set(Some(reward));
                                    }

                                    // Update currency in local storage and dispatch event
                                    if let Some(new_balance) = data.new_balance {
                                        if let Some(window) = window() {
                                            if let Some(storage) = window.local_storage().ok().flatten() {
                                                let _ = storage.set_item("currency", &new_balance.to_string());
                                            }
                                            
                                            let event_init = web_sys::CustomEventInit::new();
                                            event_init.set_detail(&wasm_bindgen::JsValue::from_f64(new_balance as f64));
                                            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                                "currencyUpdate",
                                                &event_init
                                            ) {
                                                let _ = window.dispatch_event(&event);
                                            }
                                        }
                                    }
                                }
                                cooldown.set(data.cooldown_remaining);
                                last_click.set(data.last_click);
                                total_clicks.set(data.total_clicks);
                            }
                        } else {
                            error.set("Failed to click button".to_string());
                        }
                    }
                    Err(_) => error.set("Network error".to_string()),
                }
                is_clicking.set(false);
            });
        })
    };

    html! {
        <div class="w-full max-w-4xl mx-auto py-2 text-center">
            // Container for button and claims in a more compact layout
            <div class="flex flex-col items-center justify-center mb-4">
                // Magic Button with improved styling
                <div class="relative group mb-6">
                    // Ambient glow effect
                    <div class="absolute -inset-1 bg-gradient-to-r from-indigo-500 to-purple-500 rounded-xl blur-lg opacity-0 group-hover:opacity-75 transition-all duration-500"></div>
                    
                    <button 
                        onclick={handle_click}
                        disabled={*is_clicking || *cooldown > 0}
                        class={format!("
                            relative px-10 py-5 text-lg font-bold rounded-xl
                            transition-all duration-500 transform
                            backdrop-blur-sm
                            {} {} {}",
                            if *is_clicking {
                                "scale-95 opacity-90"
                            } else if *cooldown > 0 {
                                "opacity-90 cursor-not-allowed"
                            } else {
                                "hover:scale-105 hover:shadow-[0_0_35px_rgba(99,102,241,0.5)]"
                            },
                            if *cooldown > 0 {
                                "bg-gray-500/90 text-white" // Use gray background for disabled
                            } else {
                                "bg-gradient-to-r from-indigo-600 to-purple-600 text-white shadow-lg"
                            },
                            styles::BUTTON_BASE // Keep base styles if needed
                        )}
                    >
                        if *cooldown > 0 {
                            <div class="flex items-center space-x-2">
                                <span class="w-5 h-5 rounded-full border-2 border-white/20 border-t-white/80 animate-spin"></span>
                                <span>{format_time(*cooldown)}</span>
                            </div>
                        } else if *is_clicking {
                            <div class="flex items-center space-x-2">
                                <span class="text-2xl">{"✨"}</span>
                                <span>{"Casting Magic..."}</span>
                            </div>
                        } else {
                            <div class="flex items-center space-x-2">
                                <span class="text-2xl">{"✨"}</span>
                                <span>{"Claim Pax!"}</span>
                            </div>
                        }
                    </button>

                    // Reward animation
                    if let Some(reward) = *show_reward {
                        <div class="absolute -top-12 left-1/2 transform -translate-x-1/2 animate-float-up">
                            <div class="flex items-center space-x-2 px-4 py-2 bg-amber-500/10 backdrop-blur-sm rounded-full border border-amber-500/20">
                                <span class="text-2xl font-bold bg-gradient-to-r from-amber-400 to-amber-600 bg-clip-text text-transparent">
                                    {format!("+ {} pax!", reward)}
                                </span>
                            </div>
                        </div>
                    }
                </div>

                // Horizontal claims list with latest claim on the left
                { if let Some(clicks) = &*last_click {
                    if !clicks.is_empty() {
                        html! { 
                            <div class="w-full overflow-x-auto py-2 px-2 scrollbar-thin scrollbar-thumb-gray-400 dark:scrollbar-thumb-gray-600 scrollbar-track-transparent [&::-webkit-scrollbar]:h-1.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-track]:bg-transparent [&::-webkit-scrollbar-thumb]:bg-gray-300 dark:[&::-webkit-scrollbar-thumb]:bg-gray-600 hover:[&::-webkit-scrollbar-thumb]:bg-gray-400 dark:hover:[&::-webkit-scrollbar-thumb]:bg-gray-500">
                                <div class="flex flex-row space-x-4 min-w-fit justify-center">
                                    { for clicks.iter().take(5).enumerate().map(|(index, click)| { // Added enumerate()
                                        let formatted_time = format_timestamp(&click.clicked_at);
                                        // Conditional classes for the first card
                                        let card_classes = classes!(
                                            "flex-shrink-0", "w-48", "bg-white/80", "dark:bg-gray-800/80", 
                                            "rounded-lg", "border", "shadow-sm", "hover:shadow-md", 
                                            "transition-all", "duration-300", "backdrop-blur-sm", "p-3", 
                                            "relative", "overflow-hidden", "group",
                                            if index == 0 {
                                                // Classes for the most recent card (glowing pulse)
                                                classes!("border-indigo-300", "dark:border-indigo-600", "shadow-lg", "animate-pulse")
                                            } else {
                                                // Default border for other cards
                                                classes!("border-indigo-100", "dark:border-indigo-800/50")
                                            }
                                        );
                                        html! {
                                            <div class={card_classes}>
                                                <div class="absolute inset-0 bg-gradient-to-br from-indigo-50/10 to-purple-50/5 dark:from-indigo-900/10 dark:to-purple-900/5 opacity-0 group-hover:opacity-100 transition-opacity duration-300"></div>
                                                <div class="flex flex-col h-full justify-between relative z-10">
                                                    <div class="mb-1">
                                                        <div class="flex items-center justify-between">
                                                            <span class="text-amber-500 dark:text-amber-400 font-bold flex items-center">
                                                                <span class="text-xs mr-1">{"+"}</span>
                                                                {click.reward_amount}
                                                                <span class="text-xs ml-1 text-amber-400 dark:text-amber-300">{"pax"}</span>
                                                            </span>
                                                            <span class="text-xs text-gray-500 dark:text-gray-400">{formatted_time}</span>
                                                        </div>
                                                    </div>
                                                    <div class="flex items-center space-x-1">
                                                        <span class="text-xs text-gray-500 dark:text-gray-400">{"by"}</span>
                                                        <span class="text-sm font-medium bg-gradient-to-r from-indigo-600 to-purple-600 dark:from-indigo-400 dark:to-purple-400 bg-clip-text text-transparent truncate">{&click.username}</span>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })}
                                </div>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="text-sm text-gray-500 dark:text-gray-400 mt-2 bg-white/50 dark:bg-gray-800/50 rounded-lg py-2 px-4 inline-block backdrop-blur-sm">
                                {"Be the first to claim a reward!"}
                            </div>
                        }
                    }
                } else { 
                    html! {
                        <div class="text-sm text-gray-500 dark:text-gray-400 mt-2 bg-white/50 dark:bg-gray-800/50 rounded-lg py-2 px-4 inline-block backdrop-blur-sm">
                            {"Recent claims will appear here"}
                        </div>
                    }
                }}
            </div>

            if !(*error).is_empty() {
                <div class="px-4 py-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/20 rounded-lg text-sm text-red-600 dark:text-red-400 animate-fade-in">
                    {&*error}
                </div>
            }
        </div>
    }
} 
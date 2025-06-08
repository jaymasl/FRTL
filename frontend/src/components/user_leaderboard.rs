use yew::prelude::*;
use serde::Deserialize;
use gloo_net::http::Request;
use web_sys::window;
use crate::config::get_api_base_url;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UserLeaderboardEntry {
    pub username: String,
    pub total_soul: i32,
    pub creature_count: i64,
    pub egg_count: i64,
    pub scroll_count: i64,
    pub pax: i32,
}

#[function_component(UserLeaderboard)]
pub fn user_leaderboard() -> Html {
    let leaderboard = use_state(|| Vec::<UserLeaderboardEntry>::new());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    
    // Fetch leaderboard data on mount
    {
        let leaderboard = leaderboard.clone();
        let loading = loading.clone();
        let error = error.clone();
        
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                loading.set(true);
                
                let token = window()
                    .and_then(|w| w.local_storage().ok().flatten())
                    .and_then(|s| s.get_item("token").ok().flatten())
                    .or_else(|| window()
                        .and_then(|w| w.session_storage().ok().flatten())
                        .and_then(|s| s.get_item("token").ok().flatten()))
                    .unwrap_or_default();
                
                let api_base = get_api_base_url();
                let url = format!("{}/api/leaderboard/users", api_base);
                
                match Request::get(&url)
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            match response.json::<Vec<UserLeaderboardEntry>>().await {
                                Ok(data) => {
                                    leaderboard.set(data);
                                    error.set(None);
                                },
                                Err(e) => {
                                    log::error!("Failed to parse leaderboard data: {:?}", e);
                                    error.set(Some("Failed to parse leaderboard data".to_string()));
                                }
                            }
                        } else {
                            error.set(Some(format!("Server returned status: {}", response.status())));
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to fetch leaderboard: {:?}", e);
                        error.set(Some("Failed to fetch leaderboard data".to_string()));
                    }
                }
                
                loading.set(false);
            });
            
            || ()
        });
    }
    
    html! {
        <div class="w-full mb-10 px-4">
            
            {if *loading {
                html! {
                    <div class="flex justify-center items-center p-12">
                        <div class="animate-spin rounded-full h-16 w-16 border-t-4 border-b-4 border-purple-500"></div>
                    </div>
                }
            } else if let Some(err) = &*error {
                html! {
                    <div class="bg-red-50 dark:bg-red-900/30 border border-red-200 dark:border-red-700 rounded-lg p-6 text-center max-w-md mx-auto">
                        <div class="text-red-600 dark:text-red-400 text-lg font-medium mb-2">{"Oops! Something went wrong"}</div>
                        <p class="text-red-500 dark:text-red-300">{err}</p>
                    </div>
                }
            } else if leaderboard.is_empty() {
                html! {
                    <div class="bg-gray-50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700 rounded-lg p-8 text-center max-w-md mx-auto">
                        <div class="text-3xl mb-4">{"🏆"}</div>
                        <p class="text-gray-600 dark:text-gray-300 text-lg">{"No adventurers have made their mark yet"}</p>
                    </div>
                }
            } else {
                html! {
                    <div class="space-y-8">
                        // Table for all players
                        <div class="bg-white/80 dark:bg-gray-800/80 backdrop-blur-sm shadow-xl rounded-xl overflow-hidden border border-gray-100 dark:border-gray-700 transition-all duration-300">
                            <div class="overflow-x-auto [&::-webkit-scrollbar]:h-1.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-track]:bg-transparent [&::-webkit-scrollbar-thumb]:bg-gray-300 dark:[&::-webkit-scrollbar-thumb]:bg-gray-600 hover:[&::-webkit-scrollbar-thumb]:bg-gray-400 dark:hover:[&::-webkit-scrollbar-thumb]:bg-gray-500">
                                <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                                    <thead class="bg-gray-50 dark:bg-gray-700/70">
                                        <tr>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Rank"}</th>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Username"}</th>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Total Soul"}</th>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Creatures"}</th>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Eggs"}</th>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Scrolls"}</th>
                                            <th scope="col" class="px-6 py-4 text-center text-xs font-bold text-gray-500 dark:text-gray-300 uppercase tracking-wider align-middle">{"Pax"}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {leaderboard.iter().enumerate().map(|(index, entry)| {
                                            let (bg_class, badge) = match index {
                                                0 => ("bg-yellow-50 dark:bg-yellow-900/20", "🥇"),
                                                1 => ("bg-gray-50 dark:bg-gray-700/30", "🥈"),
                                                2 => ("bg-amber-50 dark:bg-amber-900/20", "🥉"),
                                                _ => ("", "")
                                            };
                                            
                                            html! {
                                                <tr class={classes!(
                                                    "transition-colors", "duration-200",
                                                    "hover:bg-gray-50", "dark:hover:bg-gray-700/50",
                                                    bg_class
                                                )}>
                                                    <td class="px-6 py-4 whitespace-nowrap align-middle text-center">
                                                        <div class="flex items-center justify-center">
                                                            <div class={classes!(
                                                                "flex-shrink-0", "h-8", "w-8", "rounded-full", "flex", "items-center", "justify-center", "text-sm", "font-semibold",
                                                                match index {
                                                                    0 => "bg-gradient-to-br from-yellow-300 to-yellow-400 dark:from-yellow-500 dark:to-yellow-600 text-yellow-800 dark:text-yellow-100",
                                                                    1 => "bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-500 dark:to-gray-600 text-gray-800 dark:text-gray-100",
                                                                    2 => "bg-gradient-to-br from-amber-300 to-amber-400 dark:from-amber-600 dark:to-amber-700 text-amber-800 dark:text-amber-100",
                                                                    _ => "bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
                                                                }
                                                            )}>
                                                                {if !badge.is_empty() {
                                                                    html! { <span>{badge}</span> }
                                                                } else {
                                                                    html! { <span>{index + 1}</span> }
                                                                }}
                                                            </div>
                                                        </div>
                                                    </td>
                                                    <td class="px-6 py-4 whitespace-nowrap align-middle">
                                                        <div class="text-base font-semibold text-gray-900 dark:text-white">{&entry.username}</div>
                                                    </td>
                                                    <td class="px-6 py-4 whitespace-nowrap text-center align-middle">
                                                        <div class="text-base font-bold text-purple-600 dark:text-purple-400">{entry.total_soul}</div>
                                                    </td>
                                                    <td class="px-6 py-4 whitespace-nowrap text-center align-middle">
                                                        <div class="px-3 py-1 inline-flex text-sm leading-5 font-semibold rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300">
                                                            {entry.creature_count}
                                                        </div>
                                                    </td>
                                                    <td class="px-6 py-4 whitespace-nowrap text-center align-middle">
                                                        <div class="px-3 py-1 inline-flex text-sm leading-5 font-semibold rounded-full bg-amber-100 dark:bg-amber-900/30 text-amber-800 dark:text-amber-300">
                                                            {entry.egg_count}
                                                        </div>
                                                    </td>
                                                    <td class="px-6 py-4 whitespace-nowrap text-center align-middle">
                                                        <div class="px-3 py-1 inline-flex text-sm leading-5 font-semibold rounded-full bg-indigo-100 dark:bg-indigo-900/30 text-indigo-800 dark:text-indigo-300">
                                                            {entry.scroll_count}
                                                        </div>
                                                    </td>
                                                    <td class="px-6 py-4 whitespace-nowrap text-center align-middle">
                                                        <div class="text-base font-semibold text-blue-600 dark:text-blue-400">
                                                            {entry.pax}
                                                        </div>
                                                    </td>
                                                </tr>
                                            }
                                        }).collect::<Html>()}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                    </div>
                }
            }}
        </div>
    }
} 
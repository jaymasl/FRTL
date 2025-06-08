use yew::prelude::*;
use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use js_sys::{Date, Object};
use crate::config::get_api_base_url;
use web_sys::window;

#[derive(Properties, PartialEq)]
pub struct Props {
    #[prop_or_default]
    pub update_trigger: u32, // This will increment each time we need to update
}

#[derive(Deserialize, Debug, Clone)]
pub struct LeaderboardEntry {
    pub username: String,
    pub high_score: i32,
    pub updated_at: String,
}

impl LeaderboardEntry {
    fn format_local_time(&self) -> String {
        // Convert the backend timestamp ("YYYY-MM-DD HH:MM:SS") to ISO 8601 UTC format ("YYYY-MM-DDTHH:MM:SSZ")
        let utc_date_str = if self.updated_at.contains(" ") {
            format!("{}Z", self.updated_at.replace(" ", "T"))
        } else {
            self.updated_at.clone()
        };

        // Create options for date formatting
        let options = Object::new();
        let _ = js_sys::Reflect::set(&options, &"dateStyle".into(), &"medium".into());
        let _ = js_sys::Reflect::set(&options, &"timeStyle".into(), &"short".into());

        // Parse the ISO 8601 UTC timestamp string into milliseconds since epoch
        let timestamp = Date::parse(&utc_date_str);
        if timestamp.is_nan() {
            return self.updated_at.clone();
        }

        // Create a new Date from the timestamp
        let date = Date::new(&JsValue::from_f64(timestamp));
        
        // Format the date in local timezone
        date.to_locale_string("default", &options)
            .as_string()
            .unwrap_or_else(|| self.updated_at.clone())
    }
}

fn get_auth_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
}

#[function_component(Game2048Leaderboard)]
pub fn game_2048_leaderboard(props: &Props) -> Html {
    let leaderboard = use_state(|| Vec::<LeaderboardEntry>::new());
    
    // Function to fetch leaderboard data
    let fetch_leaderboard = {
        let leaderboard = leaderboard.clone();
        move || {
            wasm_bindgen_futures::spawn_local({
                let leaderboard = leaderboard.clone();
                async move {
                    let token = get_auth_token();
                    let api_base = get_api_base_url();
                    let url = format!("{}/api/leaderboard/2048?limit=10", api_base);
                    
                    match Request::get(&url)
                        .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                        .send()
                        .await 
                    {
                        Ok(response) => {
                            if response.status() == 200 {
                                match response.json::<Vec<LeaderboardEntry>>().await {
                                    Ok(entries) => {
                                        leaderboard.set(entries);
                                    },
                                    Err(e) => {
                                        log::error!("Failed to parse leaderboard JSON: {:?}", e);
                                    }
                                }
                            } else {
                                log::error!("Server returned status: {}", response.status());
                            }
                        },
                        Err(e) => {
                            log::error!("Failed to fetch leaderboard data: {:?}", e);
                        }
                    }
                }
            });
        }
    };

    // Fetch on mount and when update_trigger changes
    {
        let fetch_leaderboard = fetch_leaderboard.clone();
        use_effect_with(props.update_trigger, move |_| {
            fetch_leaderboard();
            || ()
        });
    }
    
    html! {
        <div class="mt-8 bg-white dark:bg-gray-800 rounded-lg shadow-lg p-6 max-w-3xl mx-auto">
            <h2 class="text-2xl font-bold mb-4 text-gray-800 dark:text-gray-100 text-center">
                {"2048 Leaderboard"}
            </h2>
            <div class="overflow-x-auto">
                <table class="w-full border-collapse bg-white dark:bg-gray-800">
                    <thead>
                        <tr class="bg-gray-50 dark:bg-gray-700">
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600">
                                {"Rank"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"Player"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"High Score"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"Date"}
                            </th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-gray-200 dark:divide-gray-600">
                        {for leaderboard.iter().enumerate().map(|(index, entry)| {
                            let rank_style = match index {
                                0 => "bg-yellow-500 text-white",
                                1 => "bg-gray-400 text-white",
                                2 => "bg-amber-600 text-white",
                                _ => "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-300"
                            };
                            html! {
                                <tr class="hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors duration-150 ease-in-out">
                                    <td class="px-4 py-2 whitespace-nowrap text-center">
                                        <span class={classes!(
                                            "inline-flex",
                                            "items-center",
                                            "justify-center",
                                            "w-6",
                                            "h-6",
                                            "rounded-full",
                                            "text-sm",
                                            "font-semibold",
                                            rank_style
                                        )}>
                                            {index + 1}
                                        </span>
                                    </td>
                                    <td class="px-4 py-2 whitespace-nowrap text-sm font-medium text-gray-800 dark:text-gray-200 text-center border-l border-gray-200 dark:border-gray-600">
                                        {&entry.username}
                                    </td>
                                    <td class="px-4 py-2 whitespace-nowrap text-sm font-bold text-blue-600 dark:text-blue-400 text-center border-l border-gray-200 dark:border-gray-600">
                                        {entry.high_score}
                                    </td>
                                    <td class="px-4 py-2 text-center text-sm text-gray-500 dark:text-gray-400 border-l border-gray-200 dark:border-gray-600 whitespace-nowrap">
                                        <div class="flex items-center justify-center">
                                            {entry.format_local_time()}
                                        </div>
                                    </td>
                                </tr>
                            }
                        })}
                        {if leaderboard.is_empty() {
                            html! {
                                <tr>
                                    <td colspan="4" class="px-4 py-4 text-center text-gray-500 dark:text-gray-400">
                                        {"No scores recorded yet. Be the first to play!"}
                                    </td>
                                </tr>
                            }
                        } else { html! {} }}
                    </tbody>
                </table>
            </div>
        </div>
    }
} 
use yew::prelude::*;
use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use js_sys;
use crate::config::get_api_base_url;
use web_sys::window;

#[derive(Properties, PartialEq)]
pub struct Props {
    #[prop_or_default]
    pub update_trigger: u32, // This will increment each time we need to update
}

#[derive(Deserialize, Debug, Clone)]
pub struct WordLeaderboardEntry {
    pub username: String,
    pub current_streak: i32,
    pub highest_streak: i32,
    pub fastest_time: Option<i32>,
    pub total_words_guessed: i32,
    pub total_games_played: i32,
    pub updated_at: String,
}

impl WordLeaderboardEntry {
    pub fn format_local_time(&self) -> String {
        let date_str = self.updated_at.clone();
        
        // Create a JavaScript Date object from the backend timestamp
        let date = js_sys::Date::new(&JsValue::from_str(&date_str));
        
        if date.get_time().is_nan() {
            return "Recently".to_string();
        }
        
        // Month names for formatting
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", 
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"
        ];
        
        // Get month as short name
        let month_index = date.get_month() as usize;
        let month_name = months[month_index];
        
        // Get day and year
        let day = date.get_date();
        let year = date.get_full_year();
        
        // Format time as H:MM AM/PM
        let hours = date.get_hours();
        let minutes = format!("{:02}", date.get_minutes());
        let period = if hours >= 12 { "PM" } else { "AM" };
        let hours_12 = if hours % 12 == 0 { 12 } else { hours % 12 };
        
        // Combine date and time in the format "Mar 4, 2025, 7:46 PM"
        format!("{} {}, {}, {}:{} {}", month_name, day, year, hours_12, minutes, period)
    }

    fn format_fastest_time(&self) -> String {
        match self.fastest_time {
            Some(time) => {
                let minutes = time / 60;
                let seconds = time % 60;
                if minutes > 0 {
                    format!("{}m {}s", minutes, seconds)
                } else {
                    format!("{}s", seconds)
                }
            },
            None => "-".to_string()
        }
    }
    
    fn calculate_ratio(&self) -> String {
        if self.total_games_played == 0 {
            return "-".to_string();
        }
        
        // Words guessed represents wins, so wins/total games is the win ratio
        let wins = self.total_words_guessed;
        let total = self.total_games_played;
        let win_percentage = (wins as f64 / total as f64) * 100.0;
        
        // Format as percentage with 1 decimal place
        format!("{:.1}%", win_percentage)
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

#[function_component(WordLeaderboard)]
pub fn word_leaderboard(props: &Props) -> Html {
    let leaderboard = use_state(|| Vec::<WordLeaderboardEntry>::new());
    
    // Function to fetch leaderboard data
    let fetch_leaderboard = {
        let leaderboard = leaderboard.clone();
        move || {
            wasm_bindgen_futures::spawn_local({
                let leaderboard = leaderboard.clone();
                async move {
                    let token = get_auth_token();
                    let api_base = get_api_base_url();
                    let url = format!("{}/word-game/leaderboard?limit=10", api_base);
                    
                    if let Ok(resp) = Request::get(&url)
                        .header("Authorization", &format!("Bearer {}", token.unwrap_or_default()))
                        .send()
                        .await
                    {
                        if let Ok(entries) = resp.json::<Vec<WordLeaderboardEntry>>().await {
                            leaderboard.set(entries);
                        } else {
                            log::error!("Failed to parse word leaderboard JSON");
                        }
                    } else {
                        log::error!("Failed to fetch word leaderboard data");
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
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow-lg p-6 w-full max-w-7xl mx-auto">
            <h2 class="text-2xl font-bold mb-4 text-gray-800 dark:text-gray-100 text-center">
                {"Leaderboard"}
            </h2>
            <div class="overflow-x-auto [&::-webkit-scrollbar]:h-1.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-track]:bg-transparent [&::-webkit-scrollbar-thumb]:bg-gray-300 dark:[&::-webkit-scrollbar-thumb]:bg-gray-600 hover:[&::-webkit-scrollbar-thumb]:bg-gray-400 dark:hover:[&::-webkit-scrollbar-thumb]:bg-gray-500">
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
                                {"Words Guessed"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"Success Ratio"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"Fastest Time"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"Games Played"}
                            </th>
                            <th class="px-4 py-2 text-center text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600 border-l border-gray-200 dark:border-gray-600">
                                {"Last Updated"}
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
                                    <td class="px-4 py-2 whitespace-nowrap text-sm font-bold text-orange-600 dark:text-orange-400 text-center border-l border-gray-200 dark:border-gray-600">
                                        {entry.total_words_guessed}
                                    </td>
                                    <td class="px-4 py-2 whitespace-nowrap text-sm font-bold text-pink-600 dark:text-pink-400 text-center border-l border-gray-200 dark:border-gray-600">
                                        {entry.calculate_ratio()}
                                    </td>
                                    <td class="px-4 py-2 whitespace-nowrap text-sm font-bold text-purple-600 dark:text-purple-400 text-center border-l border-gray-200 dark:border-gray-600">
                                        {entry.format_fastest_time()}
                                    </td>
                                    <td class="px-4 py-2 whitespace-nowrap text-sm font-bold text-gray-600 dark:text-gray-400 text-center border-l border-gray-200 dark:border-gray-600">
                                        {entry.total_games_played}
                                    </td>
                                    <td class="px-4 py-2 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 text-center border-l border-gray-200 dark:border-gray-600">
                                        {entry.format_local_time()}
                                    </td>
                                </tr>
                            }
                        })}
                        {if leaderboard.is_empty() {
                            html! {
                                <tr>
                                    <td colspan="7" class="px-4 py-4 text-center text-gray-500 dark:text-gray-400">
                                        {"No word game stats recorded yet. Be the first to play!"}
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
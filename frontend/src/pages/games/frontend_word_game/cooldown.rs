use yew::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct CooldownStatus {
    pub in_cooldown: bool,
    pub remaining_seconds: Option<i64>,
    pub is_win_cooldown: bool,
    pub requires_membership: bool,
}

#[derive(Clone, PartialEq)]
pub struct CooldownState {
    pub time: i64,
    pub is_win_cooldown: bool,
    pub is_loading: bool,
    pub requires_membership: bool,
}

impl Default for CooldownState {
    fn default() -> Self {
        Self {
            time: 0,
            is_win_cooldown: false,
            is_loading: false,
            requires_membership: false,
        }
    }
}

pub fn format_time(seconds: i64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

pub fn format_cooldown_time(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    format!("{}h {}m {}s", hours, minutes, secs)
}

#[derive(Properties, PartialEq)]
pub struct CooldownDisplayProps {
    pub cooldown_state: CooldownState,
}

#[function_component(CooldownDisplay)]
pub fn cooldown_display(props: &CooldownDisplayProps) -> Html {
    let cooldown_state = &props.cooldown_state;
    
    if cooldown_state.time > 0 {
        html! {
            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] text-center">
                <h3 class="text-lg font-semibold mb-2 text-gray-900 dark:text-white">
                    {
                        if cooldown_state.is_win_cooldown {
                            "Congratulations! You guessed the word!"
                        } else {
                            "Try Again Soon"
                        }
                    }
                </h3>
                <p class="text-gray-600 dark:text-gray-300">
                    {
                        if cooldown_state.is_win_cooldown {
                            "You can play again in:"
                        } else {
                            "You can try again in:"
                        }
                    }
                </p>
                <div class="text-2xl font-bold mt-2 text-blue-600 dark:text-blue-400">
                    { format_cooldown_time(cooldown_state.time) }
                </div>
            </div>
        }
    } else {
        html! {}
    }
} 
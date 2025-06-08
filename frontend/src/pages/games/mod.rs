mod claim;
mod frontend_match_game;
pub mod frontend_snake_game;
mod frontend_2048_game;
mod frontend_wheel_game;
mod frontend_word_game;
mod frontend_hexort_game;

pub mod snake_leaderboard;
pub mod word_leaderboard;
pub mod frontend_2048_leaderboard;
pub mod hexort_leaderboard;

use yew::prelude::*;
use crate::{base::Base, styles, hooks::auth_state::use_auth_check};
use claim::ClaimButton;
use frontend_match_game::FrontendMatchGame;
use frontend_snake_game::FrontendSnakeGame;
use frontend_2048_game::Frontend2048Game;
use frontend_wheel_game::FrontendWheelGame;
use frontend_word_game::FrontendWordGame;
use frontend_hexort_game::FrontendHexortGame;
use crate::components::GradientBackground;

#[derive(PartialEq, Clone)]
pub enum Tab {
    DailyReward,
    WheelGame,
    MatchGame,
    SnakeGame,
    Game2048,
    WordGame,
    HexortGame,
}

#[function_component]
pub fn Games() -> Html {
    use_auth_check();
    let error = use_state(String::new);
    let success = use_state(String::new);
    let active_tab = use_state(|| Tab::DailyReward);

    let on_success = {
        let success = success.clone();
        Callback::from(move |msg: String| success.set(msg))
    };

    let on_error = {
        let error = error.clone();
        Callback::from(move |msg: String| error.set(msg))
    };

    let switch_tab = {
        let active_tab = active_tab.clone();
        Callback::from(move |tab: Tab| active_tab.set(tab))
    };

    html! {
        <Base>
            <GradientBackground>
                <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
                    <div class="space-y-8">
                        <div class="flex justify-center">
                            <div class="overflow-x-auto pb-2 mb-8 -mb-2 [&::-webkit-scrollbar]:h-1.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-track]:bg-transparent [&::-webkit-scrollbar-thumb]:bg-gray-300 dark:[&::-webkit-scrollbar-thumb]:bg-gray-600 hover:[&::-webkit-scrollbar-thumb]:bg-gray-400 dark:hover:[&::-webkit-scrollbar-thumb]:bg-gray-500">
                                <div class="flex space-x-4 min-w-max px-4">
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::DailyReward)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::DailyReward) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"Daily Claim"}
                            </button>
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::WheelGame)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::WheelGame) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"Daily Wheel"}
                            </button>
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::WordGame)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::WordGame) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"Daily Word"}
                            </button>
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::MatchGame)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::MatchGame) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"Match"}
                            </button>
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::SnakeGame)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::SnakeGame) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"Snake"}
                            </button>
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::Game2048)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::Game2048) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"2048"}
                            </button>
                            <button
                                onclick={let switch_tab = switch_tab.clone(); move |_| switch_tab.emit(Tab::HexortGame)}
                                class={classes!(
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "transition-all",
                                    "whitespace-nowrap",
                                    if matches!(*active_tab, Tab::HexortGame) {
                                        "bg-blue-500 text-white"
                                    } else {
                                        "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                    }
                                )}
                            >
                                {"Hexort"}
                            </button>
                                </div>
                            </div>
                        </div>

                        {match *active_tab {
                            Tab::DailyReward => html! {
                                <div class="flex flex-col items-center justify-center min-h-[50vh]">
                                    <div class="w-full max-w-lg space-y-4">
                                        if !(*error).is_empty() {
                                            <div class={classes!(styles::ALERT_ERROR, "p-4")}>
                                                {&*error}
                                            </div>
                                        }
                                        if !(*success).is_empty() {
                                            <div class={classes!(styles::ALERT_SUCCESS, "p-4")}>
                                                {&*success}
                                            </div>
                                        }
                                        <ClaimButton
                                            on_success={on_success}
                                            on_error={on_error}
                                        />
                                    </div>
                                </div>
                            },
                            Tab::MatchGame => html! {
                                <div class="flex justify-center">
                                    <FrontendMatchGame />
                                </div>
                            },
                            Tab::SnakeGame => html! {
                                <div class="flex justify-center">
                                    <FrontendSnakeGame />
                                </div>
                            },
                            Tab::Game2048 => html! {
                                <div class="flex justify-center">
                                    <Frontend2048Game />
                                </div>
                            },
                            Tab::WheelGame => html! {
                                <div class="flex justify-center">
                                    <FrontendWheelGame />
                                </div>
                            },
                            Tab::WordGame => html! {
                                <div class="flex flex-col items-center">
                                    <FrontendWordGame />
                                </div>
                            },
                            Tab::HexortGame => html! {
                                <div class="flex justify-center w-full max-w-full px-2 sm:px-4">
                                    <FrontendHexortGame />
                                </div>
                            },
                        }}
                    </div>
                </div>
            </GradientBackground>
        </Base>
    }
}
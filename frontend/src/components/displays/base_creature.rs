use yew::prelude::*;
use crate::models::{Creature, ChaosRealmStatusResponse};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use super::chaos_realm_card::{enter_chaos_realm, claim_chaos_realm_reward, get_chaos_realm_status};
use crate::config::get_api_base_url;

#[derive(Clone, PartialEq)]
pub enum CreatureDisplayMode {
    Card,
    Focus
}

#[derive(Clone, PartialEq)]
pub enum ImageTab {
    Creature,
    Egg
}

#[derive(Properties, PartialEq, Clone)]
pub struct CreatureProps {
    pub creature: Creature,
    pub mode: CreatureDisplayMode,
    #[prop_or_default]
    pub on_click: Option<Callback<Creature>>,
    #[prop_or_default]
    pub on_close: Option<Callback<()>>,
}

pub fn get_creature_title(creature: &Creature) -> String {
    // Use the custom display_name if available, otherwise generate a default title
    creature.display_name.clone().unwrap_or_else(|| {
        format!("{} {}", 
            creature.essence.clone().unwrap_or_else(|| "Unknown".to_string()),
            creature.animal.clone().unwrap_or_else(|| "Creature".to_string())
        )
    })
}

pub fn get_creature_details(creature: &Creature) -> Vec<(String, Option<String>)> {
    vec![
        ("Owner".to_string(), creature.owner_username.clone()),
        ("Color".to_string(), creature.color.clone()),
        ("Creature".to_string(), creature.animal.clone()),
        ("Essence".to_string(), creature.essence.clone()),
        ("Style".to_string(), creature.art_style.clone())
    ]
}

pub fn get_creature_stats(creature: &Creature) -> Vec<(&'static str, &'static str, f64)> {
    let stats = if let Some(stats) = &creature.stats {
        vec![
            ("health", "bg-gradient-to-r from-pink-400 to-pink-500", stats["health"].as_f64().unwrap_or(100.0)),
            ("attack", "bg-gradient-to-r from-purple-500 to-fuchsia-600", stats["attack"].as_f64().unwrap_or(100.0)),
            ("speed", "bg-gradient-to-r from-emerald-500 to-emerald-600", stats["speed"].as_f64().unwrap_or(100.0)),
        ]
    } else {
        vec![
            ("health", "bg-gradient-to-r from-pink-400 to-pink-500", 100.0),
            ("attack", "bg-gradient-to-r from-purple-500 to-fuchsia-600", 100.0),
            ("speed", "bg-gradient-to-r from-emerald-500 to-emerald-600", 100.0),
        ]
    };
    stats
}

pub fn get_creature_rarity_style(creature: &Creature) -> &'static str {
    match creature.rarity.as_deref() {
        Some("Common") => "bg-gradient-to-r from-gray-500 to-gray-600 text-white ring-gray-400/30",
        Some("Uncommon") => "bg-gradient-to-r from-emerald-500 to-green-600 text-white ring-emerald-400/30",
        Some("Rare") => "bg-gradient-to-r from-blue-500 to-indigo-600 text-white ring-blue-400/30",
        Some("Epic") => "bg-gradient-to-r from-purple-500 to-fuchsia-600 text-white ring-purple-400/30",
        Some("Legendary") => "bg-gradient-to-r from-amber-400 to-yellow-500 text-white ring-amber-400/30",
        Some("Mythical") => "bg-gradient-to-r from-rose-500 to-pink-600 text-white ring-rose-400/30",
        _ => "bg-gradient-to-r from-gray-500 to-gray-600 text-white ring-gray-400/30"
    }
}

// Format time in hours, minutes, seconds format
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

pub fn get_creature_energy_info(creature: &Creature) -> Html {
    let (energy_status, progress) = if creature.energy_full {
        ("Full".to_string(), 100.0)
    } else if let Some(recharge_time) = &creature.energy_recharge_complete_at {
        let finish_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(recharge_time));
        let now = js_sys::Date::new_0();
        let remaining_ms = finish_date.get_time() - now.get_time();
        let remaining_seconds = (remaining_ms / 1000.0).ceil() as i32;
        let total_seconds = 21600; // Energy recharge takes 6 hours (21600 seconds)
        let progress = ((total_seconds - remaining_seconds.max(0)) as f64 / total_seconds as f64 * 100.0).max(0.0).min(100.0);
        (format!("Recharging: {}", format_time(remaining_seconds.max(0))), progress)
    } else {
        ("Ready to Recharge".to_string(), 0.0)
    };

    html! {
        <div class="bg-gray-800 rounded-2xl p-6 ring-1 ring-white/10">
            <div class="flex justify-between items-center mb-4">
                <h3 class="text-lg font-medium text-white">{"Energy Status"}</h3>
                <div class="flex items-center space-x-4">
                    <span class="px-3 py-1.5 bg-gradient-to-r from-purple-500 to-purple-600 rounded-lg text-sm font-medium text-white shadow-lg">
                        {format!("Soul: {}", creature.soul)}
                    </span>
                </div>
            </div>
            <div class="space-y-2">
                <div class="flex justify-between items-center">
                    <span class="text-sm text-gray-400">{"Energy Status"}</span>
                    <span class="text-sm text-white font-medium">
                        {energy_status}
                    </span>
                </div>
                <div class="relative h-2 bg-gray-700 rounded-full overflow-hidden ring-1 ring-white/10">
                    <div class="absolute top-0 left-0 h-full bg-gradient-to-r from-teal-500 to-cyan-500 rounded-full transition-all duration-300"
                        style={format!("width: {}%", progress)} />
                </div>
            </div>
        </div>
    }
}

pub fn get_creature_card_stats(creature: &Creature) -> Vec<(&'static str, f64, f64, &'static str)> {
    if creature.in_chaos_realm {
        if let Some(entry_time) = &creature.chaos_realm_entry_at {
            let entry_date = js_sys::Date::new(&JsValue::from_str(entry_time));
            let now = js_sys::Date::new_0();
            let elapsed_ms = now.get_time() - entry_date.get_time();
            let elapsed_seconds = (elapsed_ms / 1000.0).ceil() as i32;
            let total_seconds = 82800; // Chaos realm duration is 23 hours (82800 seconds)
            let remaining_seconds = total_seconds - elapsed_seconds;
            
            if remaining_seconds <= 0 {
                vec![
                    ("Ready!", 100.0, 100.0, "bg-gradient-to-r from-green-700 to-emerald-700 animate-pulse")
                ]
            } else {
                // Calculate progress as percentage of time remaining (100% to 0%)
                let progress = ((remaining_seconds as f64 / total_seconds as f64) * 100.0).max(0.0).min(100.0);
                vec![
                    ("Chaos Realm", progress, 100.0, "bg-gradient-to-r from-purple-500 to-fuchsia-600 animate-chaos-pulse")
                ]
            }
        } else {
            vec![
                ("Chaos Realm", 100.0, 100.0, "bg-gradient-to-r from-purple-500 to-fuchsia-600 animate-chaos-pulse")
            ]
        }
    } else {
        let energy_progress = if creature.energy_full {
            100.0
        } else if let Some(recharge_time) = &creature.energy_recharge_complete_at {
            let finish_date = js_sys::Date::new(&JsValue::from_str(recharge_time));
            let now = js_sys::Date::new_0();
            let remaining_ms = finish_date.get_time() - now.get_time();
            let remaining_seconds = (remaining_ms / 1000.0).ceil() as i32;
            let total_seconds = 21600; // Energy recharge takes 6 hours (21600 seconds)
            let progress = ((total_seconds - remaining_seconds.max(0)) as f64 / total_seconds as f64 * 100.0).max(0.0).min(100.0);
            progress
        } else {
            0.0
        };

        vec![
            ("Energy", energy_progress, 100.0, "bg-gradient-to-r from-teal-500 to-cyan-500")
        ]
    }
}

#[function_component(CreatureImageTabs)]
pub fn creature_image_tabs(props: &CreatureProps) -> Html {
    let creature = props.creature.clone();
    let tab = use_state(|| ImageTab::Creature);

    let current_image = match *tab {
        ImageTab::Creature => {
            creature.image_path.clone().map(|p| {
                if p.starts_with("http") { 
                    p 
                } else { 
                    format!("{}{}", get_api_base_url(), p) 
                }
            })
        },
        ImageTab::Egg => {
            Some(if creature.original_egg_image_path.starts_with("http") { 
                creature.original_egg_image_path.clone()
            } else { 
                format!("{}{}", get_api_base_url(), creature.original_egg_image_path)
            })
        }
    };

    let onclick_creature = {
        let tab_setter = tab.setter();
        Callback::from(move |_| {
            tab_setter.set(ImageTab::Creature);
        })
    };

    let onclick_egg = {
        let tab_setter = tab.setter();
        Callback::from(move |_| {
            tab_setter.set(ImageTab::Egg);
        })
    };

    html! {
        <div class="space-y-4">
            <div class="flex space-x-2 bg-gray-100 dark:bg-gray-800/50 rounded-lg p-1">
                <button 
                    onclick={onclick_creature}
                    class={classes!(
                        "flex-1", "px-4", "py-2", "text-sm", "font-medium", "rounded-md",
                        "transition-all", "duration-200",
                        match *tab {
                            ImageTab::Creature => "bg-white dark:bg-gray-700 text-gray-900 dark:text-white",
                            _ => "text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-white/50 dark:hover:bg-gray-700/50"
                        }
                    )}
                >
                    {"Creature"}
                </button>
                <button 
                    onclick={onclick_egg}
                    class={classes!(
                        "flex-1", "px-4", "py-2", "text-sm", "font-medium", "rounded-md",
                        "transition-all", "duration-200",
                        match *tab {
                            ImageTab::Egg => "bg-white dark:bg-gray-700 text-gray-900 dark:text-white",
                            _ => "text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-white/50 dark:hover:bg-gray-700/50"
                        }
                    )}
                >
                    {"Original Egg"}
                </button>
            </div>
            <div class="w-full max-h-[60vh] bg-gray-100 dark:bg-gray-800 rounded-lg flex items-center justify-center overflow-hidden">
                {if let Some(image_url) = current_image {
                    html! {
                        <img src={image_url.clone()} alt="Creature image" class="object-contain h-full w-full" />
                    }
                } else {
                    html! {
                        <div class="absolute inset-0 flex items-center justify-center text-gray-400 dark:text-gray-500">
                            {match *tab {
                                ImageTab::Egg => "🥚",
                                ImageTab::Creature => "✨"
                            }}
                        </div>
                    }
                }}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct BaseCreatureProps {
    pub creature: Creature,
    #[prop_or_default]
    pub show_details: bool,
    #[prop_or_default]
    pub on_click: Option<Callback<()>>,
}

pub enum Msg {
    EnterChaosRealm,
    ClaimReward,
    UpdateStatus(Result<ChaosRealmStatusResponse, String>),
    SetError(String),
    ClearError,
}

pub struct BaseCreature {
    is_in_chaos_realm: bool,
    remaining_time: Option<i64>,
    error_message: Option<String>,
    creature: Creature,
}

impl Component for BaseCreature {
    type Message = Msg;
    type Properties = BaseCreatureProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            is_in_chaos_realm: ctx.props().creature.in_chaos_realm,
            remaining_time: None,
            error_message: None,
            creature: ctx.props().creature.clone(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::EnterChaosRealm => {
                let creature_id = ctx.props().creature.id.to_string();
                let link = ctx.link().clone();
                self.creature.energy_full = false;
                self.is_in_chaos_realm = true;
                spawn_local(async move {
                    match enter_chaos_realm(&creature_id).await {
                        Ok(response) => {
                            if !response.success {
                                link.send_message(Msg::SetError(response.error.unwrap_or_default()));
                            } else {
                                link.send_message(Msg::UpdateStatus(
                                    get_chaos_realm_status(&creature_id).await
                                        .map_err(|e| e.to_string())
                                ));
                            }
                        }
                        Err(e) => link.send_message(Msg::SetError(e.to_string())),
                    }
                });
                true
            }
            Msg::ClaimReward => {
                let creature_id = ctx.props().creature.id.to_string();
                let link = ctx.link().clone();
                spawn_local(async move {
                    match claim_chaos_realm_reward(&creature_id).await {
                        Ok(response) => {
                            if !response.success {
                                link.send_message(Msg::SetError(response.error.unwrap_or_default()));
                            } else {
                                link.send_message(Msg::UpdateStatus(
                                    get_chaos_realm_status(&creature_id).await
                                        .map_err(|e| e.to_string())
                                ));
                            }
                        }
                        Err(e) => link.send_message(Msg::SetError(e.to_string())),
                    }
                });
                false
            }
            Msg::UpdateStatus(result) => {
                match result {
                    Ok(status) => {
                        self.is_in_chaos_realm = status.in_realm;
                        self.remaining_time = status.remaining_seconds;
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(e);
                    }
                }
                true
            }
            Msg::SetError(error) => {
                self.error_message = Some(error);
                true
            }
            Msg::ClearError => {
                self.error_message = None;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let creature = &ctx.props().creature;
        let onclick = ctx.props().on_click.clone();

        html! {
            <div class="creature-card" onclick={onclick.map(|f| Callback::from(move |_| f.emit(())))}>
                if ctx.props().show_details {
                    <div class="creature-details">
                        <div class="bg-gray-800 dark:bg-gray-900 rounded-2xl p-6 ring-1 ring-white/10 mt-4">
                            <div class="flex justify-between items-center mb-4">
                                <h3 class="text-lg font-medium text-white">{"Chaos Realm"}</h3>
                            </div>
                            <div class="space-y-4">
                                if self.is_in_chaos_realm {
                                    <div class="bg-gray-700/50 rounded-xl p-4 space-y-3">
                                        <p class="text-purple-400 font-medium">{"In Chaos Realm"}</p>
                                        if let Some(time) = self.remaining_time {
                                            <p class="text-gray-300">{format!("Time Remaining: {}", format_time(time as i32))}</p>
                                        }
                                        <button 
                                            class="w-full py-2 px-4 bg-gradient-to-r from-purple-500 to-purple-600 text-white rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed"
                                            disabled={self.remaining_time.unwrap_or(1) > 0}
                                            onclick={ctx.link().callback(|_| Msg::ClaimReward)}
                                        >
                                            {"Claim Reward"}
                                        </button>
                                    </div>
                                } else {
                                    <button 
                                        onclick={ctx.link().callback(|_| Msg::EnterChaosRealm)}
                                        disabled={!creature.energy_full}
                                        class="w-full py-2 px-4 bg-gradient-to-r from-purple-500 to-purple-600 text-white rounded-lg font-medium hover:from-purple-600 hover:to-purple-700 disabled:opacity-50 disabled:cursor-not-allowed"
                                        title={if !creature.energy_full { "Requires full energy to enter" } else { "Enter Chaos Realm" }}
                                    >
                                        if !creature.energy_full {
                                            {"Requires Full Energy"}
                                        } else {
                                            {"Enter Chaos Realm"}
                                        }
                                    </button>
                                }

                                if let Some(error) = &self.error_message {
                                    <p class="text-red-400 text-sm mt-2">{error}</p>
                                }
                            </div>
                        </div>
                    </div>
                }
            </div>
        }
    }
}
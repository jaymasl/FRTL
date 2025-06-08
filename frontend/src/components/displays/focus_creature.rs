use yew::prelude::*;
use web_sys::MouseEvent;
use crate::models::Creature;
use super::{CreatureImageTabs, CreatureDisplayMode, get_creature_title, get_creature_details, get_creature_stats, BindModal, ChaosRealmCard, SoulBindButton, EnergyManager, DisplayMode, RenameCreature};
use crate::pages::inventory::handlers::CreatureResponse;
use crate::styles;
use uuid::Uuid;
use crate::hooks::use_membership::use_membership;
use std::collections::HashSet;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::config::get_api_base_url;
use web_sys::{window, Event, CustomEvent};
use js_sys::{Date, Object};
use wasm_bindgen::{JsValue, JsCast};
use serde_json::Value;
use gloo::events::EventListener;
use gloo_utils::format::JsValueSerdeExt;

#[derive(Properties, PartialEq)]
pub struct CreatureFocusProps {
    pub creature: Creature,
    pub action_label: Option<String>,
    pub on_action: Option<Callback<()>>,
    pub loading: bool,
    pub error: String,
    pub fetch_data: Option<Callback<()>>,
    pub mode: DisplayMode,
    #[prop_or_default]
    pub on_energy_update: Option<Callback<(Uuid, bool)>>,
}

#[derive(Properties, PartialEq)]
struct TooltipProps {
    content: Html,
    title: String,
    bg_color: String,
    text_color: String,
}

#[function_component(Tooltip)]
fn tooltip(props: &TooltipProps) -> Html {
    let is_visible = use_state(|| false);

    let toggle_visibility = {
        let is_visible = is_visible.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            is_visible.set(!*is_visible);
        })
    };

    html! {
        <div class="group relative">
            <button 
                class={format!("w-5 h-5 rounded-full {} {} flex items-center justify-center text-sm hover:opacity-80", props.bg_color, props.text_color)}
                title={props.title.clone()}
                onclick={toggle_visibility}
            >
                {"i"}
            </button>
            <div class={classes!(
                "fixed",
                "z-[9999]",
                "bg-gray-900",
                "text-white",
                "text-sm",
                "rounded-lg",
                "p-3",
                "w-64",
                "shadow-lg",
                "transform",
                "-translate-x-1/2",
                "left-1/2",
                "-translate-y-full",
                if *is_visible { "visible" } else { "invisible" },
                "group-hover:visible"
            )}>
                {props.content.clone()}
            </div>
        </div>
    }
}

#[function_component(CreatureFocus)]
pub fn creature_focus(props: &CreatureFocusProps) -> Html {
    let show_bind_modal = use_state(|| false);
    let loading_chaos = use_state(|| false);
    let error = use_state(|| props.error.clone());
    let rename_mode = use_state(|| false);
    let updated_creature: UseStateHandle<Option<CreatureResponse>> = use_state(|| None);
    let recharging_creatures = use_state(|| HashSet::<Uuid>::new());
    let membership = use_membership();
    let available_creatures = use_state(Vec::new);
    
    // Add state to force refresh after chaos realm claim
    let force_update = use_state(|| 0);
    let override_in_chaos_realm = use_state(|| None::<bool>);
    let override_chaos_entry_time = use_state(|| None::<String>);

    {
        let available_creatures = available_creatures.clone();
        let creature_id = props.creature.id;
        let error_state = error.clone();

        // Fetch available creatures when component loads
        use_effect_with((), move |_| {
            let available_creatures = available_creatures.clone();
            let error_state = error_state.clone();
            
            spawn_local(async move {
                let token = window()
                    .and_then(|w| w.local_storage().ok().flatten())
                    .and_then(|s| s.get_item("token").ok().flatten())
                    .or_else(|| window()
                        .and_then(|w| w.session_storage().ok().flatten())
                        .and_then(|s| s.get_item("token").ok().flatten()))
                    .unwrap_or_default();

                if token.is_empty() {
                    error_state.set("Authentication error - please log in again".to_string());
                    return;
                }

                match Request::get(&format!("{}/api/creatures", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await {
                        Ok(response) => {
                            if let Ok(creatures) = response.json::<Vec<Creature>>().await {
                                let filtered = creatures.into_iter()
                                    .filter(|c| c.id != creature_id)
                                    .collect::<Vec<Creature>>();
                                
                                available_creatures.set(filtered);
                            } else {
                                log::error!("Failed to parse creatures data in initial fetch");
                                error_state.set("Failed to parse creatures data".to_string());
                            }
                        }
                        Err(e) => {
                            log::error!("Initial creature fetch failed: {:?}", e);
                            error_state.set("Failed to fetch creatures".to_string());
                        }
                }
            });
            
            || {}
        });
    }

    // Original effect for modal visibility
    {
        let available_creatures = available_creatures.clone();
        let creature_id = props.creature.id;
        let error_state = error.clone();
        let show_bind_modal_val = *show_bind_modal;
        
        use_effect_with(show_bind_modal_val, move |&should_fetch| {
            if should_fetch {
                let available_creatures = available_creatures.clone();
                let error_state = error_state.clone();
                
                spawn_local(async move {
                    let token = window()
                        .and_then(|w| w.local_storage().ok().flatten())
                        .and_then(|s| s.get_item("token").ok().flatten())
                        .or_else(|| window()
                            .and_then(|w| w.session_storage().ok().flatten())
                            .and_then(|s| s.get_item("token").ok().flatten()))
                        .unwrap_or_default();

                    if token.is_empty() {
                        error_state.set("Authentication error - please log in again".to_string());
                        return;
                    }

                    match Request::get(&format!("{}/api/creatures", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await {
                            Ok(response) => {
                                if let Ok(creatures) = response.json::<Vec<Creature>>().await {
                                    let filtered = creatures.into_iter()
                                        .filter(|c| c.id != creature_id)
                                        .collect::<Vec<Creature>>();
                                    
                                    available_creatures.set(filtered);
                                } else {
                                    log::error!("Failed to parse creatures data in modal refresh");
                                    error_state.set("Failed to parse creatures data".to_string());
                                }
                            }
                            Err(e) => {
                                log::error!("Modal creature refresh failed: {:?}", e);
                                error_state.set("Failed to fetch creatures".to_string());
                            }
                    }
                });
            }
            
            || {}
        });
    }

    let open_bind_modal = {
        let show_bind_modal = show_bind_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_bind_modal.set(true);
        })
    };

    let close_bind_modal = {
        let show_bind_modal = show_bind_modal.clone();
        Callback::from(move |_: MouseEvent| show_bind_modal.set(false))
    };

    let on_recharge_start = {
        let recharging_creatures = recharging_creatures.clone();
        Callback::from(move |creature_id: Uuid| {
            let mut current_set = (*recharging_creatures).clone();
            current_set.insert(creature_id);
            recharging_creatures.set(current_set);
        })
    };

    let on_energy_update_internal = {
        let on_energy_update_prop = props.on_energy_update.clone();
        let updated_creature_state = updated_creature.clone();
        let recharging_creatures = recharging_creatures.clone();
        Callback::from(move |(id, is_full): (Uuid, bool)| {
            if let Some(mut current_creature) = (*updated_creature_state).clone() {
                 if current_creature.id == id {
                     current_creature.energy_full = is_full;
                     if is_full {
                         current_creature.energy_recharge_complete_at = None;
                     }
                     updated_creature_state.set(Some(current_creature));
                 }
            }

            if is_full {
                let mut current_set = (*recharging_creatures).clone();
                if current_set.remove(&id) {
                     recharging_creatures.set(current_set);
                }
            }

            if let Some(cb) = on_energy_update_prop.as_ref() {
                cb.emit((id, is_full));
            }
        })
    };

    // Add creatureUpdate event listener
    {
        let override_in_chaos_realm = override_in_chaos_realm.clone();
        let force_update = force_update.clone();
        let fetch_data = props.fetch_data.clone();
        let creature_id = props.creature.id;
        let override_chaos_entry_time_for_event = override_chaos_entry_time.clone();
        
        use_effect_with((), move |_| {
            let window = window().expect("no global window exists");
            
            let listener = EventListener::new(&window, "creatureUpdate", move |event: &Event| {
                if let Some(custom_event) = event.dyn_ref::<CustomEvent>() {
                    if let Ok(detail) = custom_event.detail().into_serde::<Value>() {
                        // Check if this event contains chaos realm state changes
                        if let Some(in_realm) = detail.get("in_chaos_realm").and_then(|v| v.as_bool()) {
                            // Check if this applies to the current creature
                            if let Some(current_id) = detail.get("id").and_then(|v| v.as_str()) {
                                if let Ok(id) = Uuid::parse_str(current_id) {
                                    if id == creature_id {
                                        // Update state for both entering and exiting chaos realm
                                        override_in_chaos_realm.set(Some(in_realm));
                                        
                                        // When entering chaos realm, also capture the entry time if provided
                                        if in_realm {
                                            if let Some(entry_time) = detail.get("chaos_realm_entry_at").and_then(|v| v.as_str()) {
                                                override_chaos_entry_time_for_event.set(Some(entry_time.to_string()));
                                            }
                                        } else {
                                            // When exiting, clear the entry time
                                            override_chaos_entry_time_for_event.set(None);
                                        }
                                        
                                        // Force update to ensure re-rendering
                                        force_update.set(*force_update + 1);
                                        
                                        // Also ensure regular fetch happens in the background
                                        if let Some(fetch) = fetch_data.clone() {
                                            fetch.emit(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
            
            move || { drop(listener); }
        });
    }

    // Derive the creature to display, prioritizing the updated_creature state
    let display_creature = use_memo(
        // Dependencies tuple first - add force_update to ensure recomputation
        (props.creature.clone(), (*updated_creature).clone(), *force_update, (*override_in_chaos_realm).clone(), (*override_chaos_entry_time).clone()), 
        // Closure second
        |deps| {
            let (props_creature, updated_creature_opt, _, override_realm, override_entry_time) = deps;
            let mut final_creature = if let Some(updated) = updated_creature_opt {
                // Map CreatureResponse to Creature for display consistency
                Creature {
                    id: updated.id,
                    owner_id: props_creature.owner_id,
                    owner_username: props_creature.owner_username.clone(),
                    image_path: props_creature.image_path.clone(),
                    display_name: Some(updated.display_name.clone()),
                    original_egg_id: props_creature.original_egg_id,
                    original_egg_created_at: updated.original_egg_created_at.clone(),
                    original_egg_image_path: props_creature.original_egg_image_path.clone(),
                    egg_summoned_by_username: props_creature.egg_summoned_by_username.clone(),
                    original_egg_summoned_by: props_creature.original_egg_summoned_by,
                    hatched_at: updated.hatched_at.clone(),
                    hatched_by_username: props_creature.hatched_by_username.clone(),
                    hatched_by: props_creature.hatched_by,
                    essence: props_creature.essence.clone(),
                    animal: props_creature.animal.clone(),
                    color: props_creature.color.clone(),
                    art_style: props_creature.art_style.clone(),
                    stats: Some(serde_json::to_value(&updated.stats).unwrap_or(Value::Null)),
                    rarity: Some(updated.rarity.clone()),
                    energy_full: updated.energy_full,
                    energy_recharge_complete_at: updated.energy_recharge_complete_at.clone(),
                    in_chaos_realm: props_creature.in_chaos_realm,
                    chaos_realm_entry_at: props_creature.chaos_realm_entry_at.clone(),
                    chaos_realm_reward_claimed: props_creature.chaos_realm_reward_claimed,
                    status: props_creature.status.clone(),
                    soul: updated.soul,
                    streak: updated.streak,
                    prompt: props_creature.prompt.clone(),
                }
            } else {
                props_creature.clone()
            };
            
            // Apply override if it exists - this allows us to immediately update
            // the in_chaos_realm flag from the event without waiting for a server refresh
            if let Some(override_realm) = override_realm {
                final_creature.in_chaos_realm = *override_realm;
                
                // When exiting chaos realm, clear the entry time
                if !*override_realm {
                    final_creature.chaos_realm_entry_at = None;
                }
            }
            
            // Apply entry time override if provided
            if let Some(entry_time) = override_entry_time {
                final_creature.chaos_realm_entry_at = Some(entry_time.clone());
            }
            
            final_creature
        }
    );

    html! {
        <>
            <div class="grid grid-cols-1 md:grid-cols-[1fr_3.3fr_1fr] gap-4 relative">
                {
                    if membership.is_member {
                        html! {}
                    } else {
                        html! {}
                    }
                }

                {
                    if *rename_mode {
                        html! {
                            <div class="absolute top-[6.5rem] left-1/2 transform -translate-x-1/2 z-20 w-80">
                                <RenameCreature
                                    creature_id={props.creature.id}
                                    current_name={props.creature.display_name.clone().unwrap_or_else(|| get_creature_title(&props.creature))}
                                    on_success={
                                        let fetch_data = props.fetch_data.clone();
                                        let rename_mode = rename_mode.clone();
                                        Callback::from(move |_new_name: String| {
                                            rename_mode.set(false);
                                            if let Some(fetch) = fetch_data.clone() {
                                                fetch.emit(());
                                            }
                                        })
                                    }
                                    on_error={
                                        let error = error.clone();
                                        Callback::from(move |err| error.set(err))
                                    }
                                />
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                <div class="space-y-4 order-3 md:order-1 overflow-visible">
                    <div class={classes!(styles::FOCUS_CARD, "overflow-visible")}>
                        <h3 class={styles::FOCUS_CARD_TITLE}>{"History"}</h3>
                        <div class={styles::FOCUS_GRID_CONTENT}>
                            <div class="space-y-1">
                                <div class={styles::FOCUS_LABEL}>{"Summon"}</div>
                                <div class={styles::FOCUS_GROUP}>
                                    <div class={styles::FOCUS_VALUE}>
                                        {props.creature.egg_summoned_by_username.clone().unwrap_or_else(|| "Unknown".to_string())}
                                    </div>
                                    {if let Some(created_at) = &props.creature.original_egg_created_at {
                                        html! {
                                            <div class={styles::FOCUS_VALUE_SECONDARY}>
                                                {format_datetime(created_at)}
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                            </div>
                            <div class="space-y-1">
                                <div class={styles::FOCUS_LABEL}>{"Hatch"}</div>
                                <div class={styles::FOCUS_GROUP}>
                                    <div class={styles::FOCUS_VALUE}>
                                        {props.creature.hatched_by_username.clone().unwrap_or_else(|| "Unknown".to_string())}
                                    </div>
                                    {if let Some(hatched_at) = &props.creature.hatched_at {
                                        html! {
                                            <div class={styles::FOCUS_VALUE_SECONDARY}>
                                                {format_datetime(hatched_at)}
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="bg-white dark:bg-gray-800 rounded-2xl p-4 ring-1 ring-gray-200 dark:ring-white/10 shadow-lg">
                        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{"Attributes"}</h3>
                        <div class="divide-y divide-gray-200 dark:divide-gray-700">
                            {get_creature_stats(&*display_creature).into_iter().map(|(label, style, value)| {
                                let updated_value = if let Some(creature) = &*updated_creature {
                                    match label {
                                        "health" => creature.stats.get("health").and_then(|v| v.as_f64()).unwrap_or(value),
                                        "attack" => creature.stats.get("attack").and_then(|v| v.as_f64()).unwrap_or(value),
                                        "speed" => creature.stats.get("speed").and_then(|v| v.as_f64()).unwrap_or(value),
                                        _ => value
                                    }
                                } else { value };

                                html! {
                                    <div class="flex justify-between items-center py-3">
                                        <span class="text-sm text-gray-600 dark:text-gray-400 capitalize">{label}</span>
                                        <span class={classes!(
                                            "px-3",
                                            "py-1",
                                            "rounded-lg",
                                            "text-sm",
                                            "font-medium",
                                            style
                                        )}>
                                            {format!("{}", updated_value)}
                                        </span>
                                    </div>
                                }
                            }).collect::<Html>()}
                        </div>
                    </div>

                    {if matches!(props.mode, DisplayMode::Focus) {
                        html! {
                            <div class="bg-white dark:bg-gray-800 rounded-2xl p-4 ring-1 ring-gray-200 dark:ring-white/10 shadow-lg">
                                <div class="flex flex-col space-y-2">
                                    <div class="flex items-center justify-between">
                                        <h3 class="text-lg font-medium text-gray-900 dark:text-white">{"Soul"}</h3>
                                        <Tooltip
                                            title="Soul Info"
                                            bg_color="bg-purple-500/20"
                                            text_color="text-purple-400"
                                            content={html! {
                                                <>
                                                    <p>{"Soul points are earned through:"}</p>
                                                    <ul class="list-disc list-inside space-y-1 text-sm mt-2">
                                                        <li>{"Soul Binding: +5 points"}</li>
                                                        <li>{"Energy Recharge: +1 point"}</li>
                                                        <li>{"Entering Chaos Realm: +1 point"}</li>
                                                        <li>{"Claiming Rewards: +1 point"}</li>
                                                    </ul>
                                                </>
                                            }}
                                        />
                                    </div>
                                    <div class="flex items-center justify-center py-3">
                                        <div class="px-6 py-3 bg-gradient-to-r from-purple-500 to-fuchsia-600 rounded-lg text-2xl font-bold text-white shadow-lg">
                                            {(*display_creature).soul}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>

                <div class="space-y-4 order-1 md:order-2">
                    <div class="space-y-4">
                        <CreatureImageTabs creature={(*display_creature).clone()} mode={CreatureDisplayMode::Focus} />
                        <div class="space-y-2">
                            <div class="flex items-center justify-between">
                                <div class="flex items-center space-x-3">
                                    <h2 class="text-2xl font-bold text-gray-900 dark:text-white">
                                        {get_creature_title(&*display_creature)}
                                    </h2>
                                    {if membership.is_member {
                                        html! {
                                            <button 
                                                onclick={let rename_mode = rename_mode.clone(); move |_| rename_mode.set(!*rename_mode)}
                                                class="flex items-center justify-center w-7 h-7 rounded-full bg-blue-100 dark:bg-blue-900/50 text-blue-600 dark:text-blue-400 hover:bg-blue-200 dark:hover:bg-blue-800 hover:text-blue-700 dark:hover:text-blue-300 focus:outline-none transition-colors"
                                                title="Rename Creature"
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                                                    <path d="M13.586 3.586a2 2 0 112.828 2.828l-.793.793-2.828-2.828.793-.793zM11.379 5.793L3 14.172V17h2.828l8.38-8.379-2.83-2.828z" />
                                                </svg>
                                            </button>
                                        }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                            </div>
                            
                            {if matches!(props.mode, DisplayMode::Focus) {
                                html! {
                                    <div class="flex items-center space-x-3 mt-2">
                                        <div class={classes!(
                                            "px-3", "py-1.5", "text-sm", "font-medium", "rounded-lg", "shadow-lg", "ring-1",
                                            match (*display_creature).rarity.as_deref() {
                                                Some("Common") => "bg-gradient-to-r from-gray-500 to-gray-600 text-white ring-gray-400/30",
                                                Some("Uncommon") => "bg-gradient-to-r from-emerald-500 to-green-600 text-white ring-emerald-400/30",
                                                Some("Rare") => "bg-gradient-to-r from-blue-500 to-indigo-600 text-white ring-blue-400/30",
                                                Some("Epic") => "bg-gradient-to-r from-purple-500 to-fuchsia-600 text-white ring-purple-400/30",
                                                Some("Legendary") => "bg-gradient-to-r from-amber-400 to-yellow-500 text-white ring-amber-400/30",
                                                Some("Mythical") => "bg-gradient-to-r from-rose-500 to-pink-600 text-white ring-rose-400/30",
                                                _ => "bg-gradient-to-r from-gray-500 to-gray-600 text-white ring-gray-400/30"
                                            }
                                        )}>
                                            {(*display_creature).rarity.clone().unwrap_or_else(|| "Unknown".to_string())}
                                        </div>
                                        <SoulBindButton
                                            loading={props.loading}
                                            energy_full={(*display_creature).energy_full}
                                            can_bind={(*display_creature).rarity.as_deref().map_or(false, |r| r != "Mythical")}
                                            on_click={open_bind_modal.clone()}
                                            target_creature={(*display_creature).clone()}
                                            available_creatures={(*available_creatures).clone()}
                                            is_energy_transitioning={recharging_creatures.contains(&(*display_creature).id)}
                                            in_chaos_realm={(*display_creature).in_chaos_realm}
                                        />
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                            
                            if !props.error.is_empty() {
                                <div class="mt-2 text-sm text-red-400">{&props.error}</div>
                            }
                            if !(*error).is_empty() {
                                <div class="mt-2 text-sm text-red-400">{&*error}</div>
                            }
                        </div>
                    </div>
                </div>
                
                <div class="space-y-4 order-2 md:order-3 overflow-visible">
                    <div class={classes!(
                        "bg-white",
                        "dark:bg-gray-800",
                        "rounded-2xl",
                        "p-4",
                        "ring-1",
                        "ring-gray-200",
                        "dark:ring-white/10",
                        "shadow-lg",
                        "overflow-visible"
                    )}>
                        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2 text-center">{"Details"}</h3>
                        <div class="grid grid-cols-1 gap-2">
                            {get_creature_details(&props.creature).into_iter().map(|(label, value)| html! {
                                <div class="space-y-1 text-center">
                                    <div class="text-sm font-medium text-gray-600 dark:text-gray-400">{label}</div>
                                    <div class="text-sm text-gray-900 dark:text-white">
                                        {value.unwrap_or_else(|| "Unknown".to_string())}
                                    </div>
                                </div>
                            }).collect::<Html>()}
                        </div>
                    </div>

                    {if matches!(props.mode, DisplayMode::Focus) {
                        html! {
                            <div class="bg-white dark:bg-gray-800 rounded-2xl p-4 ring-1 ring-gray-200 dark:ring-white/10 shadow-lg">
                                <div class="flex flex-col space-y-2">
                                    <div class="flex items-center justify-between">
                                        <h3 class="text-lg font-medium text-gray-900 dark:text-white">{"Energy"}</h3>
                                        <Tooltip
                                            title="Energy Info"
                                            bg_color="bg-teal-500/20"
                                            text_color="text-teal-400"
                                            content={html! {
                                                <>
                                                    <p class="mb-2">{"Charge Cost by Rarity:"}</p>
                                                    <ul class="list-disc list-inside space-y-1 text-sm">
                                                        <li>{"Uncommon: 5 pax"}</li>
                                                        <li>{"Rare: 10 pax"}</li>
                                                        <li>{"Epic: 20 pax"}</li>
                                                        <li>{"Legendary: 30 pax"}</li>
                                                        <li>{"Mythical: 40 pax"}</li>
                                                    </ul>
                                                    <p class="mt-2">{"Takes 6 hours to fully charge."}</p>
                                                </>
                                            }}
                                        />
                                    </div>
                                    <EnergyManager
                                        creature={(*display_creature).clone()}
                                        updated_creature={(*updated_creature).clone()}
                                        fetch_data={props.fetch_data.clone()}
                                        on_energy_update={on_energy_update_internal.clone()}
                                        on_recharge_start={on_recharge_start.clone()}
                                    />
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    {if matches!(props.mode, DisplayMode::Focus) && (*display_creature).rarity.as_deref().map_or(false, |r| r != "Common") {
                        html! {
                            <div class="bg-white dark:bg-gray-800 rounded-2xl p-4 ring-1 ring-gray-200 dark:ring-white/10 shadow-lg">
                                <div class="flex flex-col space-y-2">
                                    <div class="flex items-center justify-between">
                                        <h3 class="text-lg font-medium text-gray-900 dark:text-white">{"Chaos"}</h3>
                                        <Tooltip
                                            title="Chaos Realm Info"
                                            bg_color="bg-purple-500/20"
                                            text_color="text-purple-400"
                                            content={html! {
                                                <>
                                                    <p class="mb-2">{"Reward by Rarity:"}</p>
                                                    <ul class="list-disc list-inside space-y-1 text-sm">
                                                        <li>{"Uncommon: +8 pax"}</li>
                                                        <li>{"Rare: +18 pax"}</li>
                                                        <li>{"Epic: +38 pax"}</li>
                                                        <li>{"Legendary: +68 pax"}</li>
                                                        <li>{"Mythical: +118 pax"}</li>
                                                    </ul>
                                                    <p class="mt-2">{"Must wait 23 hours to claim."}</p>
                                                </>
                                            }}
                                        />
                                    </div>
                                    <div class="space-y-4">
                                        <ChaosRealmCard
                                            creature={(*display_creature).clone()}
                                            loading_chaos={*loading_chaos}
                                            error={(*error).clone()}
                                            fetch_data={props.fetch_data.clone()}
                                            key={(*display_creature).id.to_string()}
                                            is_recharging={recharging_creatures.contains(&(*display_creature).id)}
                                        />
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>

            {if matches!(props.mode, DisplayMode::Focus) && *show_bind_modal {
                html! {
                    <BindModal
                        target={(*display_creature).clone()}
                        available_creatures={(*available_creatures).clone()}
                        on_success={
                            let close_bind_modal = close_bind_modal.clone();
                            let fetch_data = props.fetch_data.clone();
                            let updated_creature_state = updated_creature.clone();
                            Callback::from(move |creature_response: Option<CreatureResponse>| {
                                if let Some(ref updated_creature_data) = creature_response {
                                    updated_creature_state.set(Some(updated_creature_data.clone()));
                                }
                                
                                let event = MouseEvent::new("click").unwrap_or_else(|_| panic!("Failed to create event"));
                                close_bind_modal.emit(event);
                                
                                if let Some(cb) = fetch_data.as_ref() {
                                    cb.emit(());
                                }
                            })
                        }
                        on_error={Callback::from(move |_| show_bind_modal.set(false))}
                        on_close={close_bind_modal}
                        loading={props.loading}
                        fetch_data={props.fetch_data.clone()}
                    />
                }
            } else {
                html! {}
            }}
        </>
    }
}

fn format_datetime(datetime: &str) -> String {
    let date = Date::new(&JsValue::from_str(datetime));
    let options = Object::new();
    
    // Set date formatting options
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("year"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("month"), &JsValue::from_str("long"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("day"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("hour"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("minute"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("hour12"), &JsValue::from_bool(true));
    
    date.to_locale_string("en-US", &options)
        .as_string()
        .unwrap_or_else(|| datetime.to_string())
}
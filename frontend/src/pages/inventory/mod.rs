pub mod handlers;
mod state;
mod filters;

use crate::base::Base;
use crate::hooks::auth_state::{use_auth_check, use_auth_token};
use crate::components::displays::{Display, DisplayItem, DisplayMode};
use yew::prelude::*;
use yew_router::prelude::*;
use state::{HatchState, handle_session_expired, get_filtered_items};
use handlers::{handle_item_click, handle_close, handle_hatch};
use web_sys::window;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use gloo_net::http::Request;
use crate::models::{Creature, Egg, Scroll};
use wasm_bindgen_futures::spawn_local;
use filters::{FilterBar, CollectionType, SortCriteria, sort_items};
use uuid::Uuid;
use crate::hooks::use_currency::use_currency;
use crate::config::get_api_base_url;
use crate::components::GradientBackground;
use std::collections::HashMap;

#[function_component(Inventory)]
pub fn inventory() -> Html {
    use_auth_check();
    let navigator = use_navigator().unwrap();
    let loading = use_state(|| false);
    let _item_node_refs = use_mut_ref(HashMap::<Uuid, NodeRef>::new);
    let eggs = use_state(Vec::new);
    let creatures = use_state(Vec::new);
    let selected_item = use_state(|| None::<DisplayItem>);
    let token = use_auth_token();
    let hatch_state = use_state(HatchState::default);
    let _current_currency = use_currency();
    let collection_type = use_state(|| CollectionType::All);
    let sort_criteria = use_state(|| SortCriteria::Default);
    let sort_ascending = use_state(|| false);
    let scrolls = use_state(Vec::new);
    let listed_creature_ids = use_state(|| Vec::new());

    let handle_session_expired = {
        let navigator = navigator.clone();
        move || handle_session_expired(&navigator)
    };

    let fetch_data = {
        let eggs = eggs.clone();
        let creatures = creatures.clone();
        let scrolls = scrolls.clone();
        let token = token.clone();
        let handle_session_expired = handle_session_expired.clone();
        let listed_creature_ids = listed_creature_ids.clone();
        
        Callback::from(move |_: ()| {
            let eggs = eggs.clone();
            let creatures = creatures.clone();
            let scrolls = scrolls.clone();
            let token = token.clone();
            let handle_session_expired = handle_session_expired.clone();
            let listed_creature_ids = listed_creature_ids.clone();
            
            spawn_local(async move {
                // Fetch scrolls
                if let Ok(response) = Request::get(&format!("{}/api/scrolls", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await 
                {
                    match response.status() {
                        401 => handle_session_expired(),
                        200 => {
                            if let Ok(data) = response.json::<Vec<Scroll>>().await {
                                scrolls.set(data);
                            }
                        }
                        _ => log::error!("Failed to fetch scrolls"),
                    }
                }

                // Fetch creatures
                if let Ok(response) = Request::get(&format!("{}/api/creatures", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await 
                {
                    match response.status() {
                        401 => handle_session_expired(),
                        200 => {
                            if let Ok(data) = response.json::<Vec<Creature>>().await {
                                creatures.set(data);
                            }
                        }
                        _ => log::error!("Failed to fetch creatures"),
                    }
                }

                // Fetch eggs
                if let Ok(response) = Request::get(&format!("{}/api/eggs", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await 
                {
                    match response.status() {
                        401 => handle_session_expired(),
                        200 => {
                            if let Ok(data) = response.json::<Vec<Egg>>().await {
                                eggs.set(data);
                            }
                        }
                        _ => log::error!("Failed to fetch eggs"),
                    }
                }

                // Fetch market listed creatures
                if let Ok(response) = Request::get(&format!("{}/api/market/listings", get_api_base_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await 
                {
                    match response.status() {
                        401 => handle_session_expired(),
                        200 => {
                            if let Ok(listings) = response.json::<Vec<serde_json::Value>>().await {
                                // Extract creature IDs from listings
                                let creature_ids: Vec<Uuid> = listings.iter()
                                    .filter_map(|listing| {
                                        if listing["item_type"].as_str() == Some("creature") {
                                            listing["item_id"].as_str()
                                                .and_then(|id| Uuid::parse_str(id).ok())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                listed_creature_ids.set(creature_ids);
                            }
                        }
                        _ => log::error!("Failed to fetch market creatures"),
                    }
                }
            });
        })
    };

    {
        let fetch_data = fetch_data.clone();
        let token = token.clone();
        use_effect_with((), move |_| {
            if !token.is_empty() {
                fetch_data.emit(());
            }
            || ()
        });
    }

    let handle_collection_change = {
        let collection_type = collection_type.clone();
        Callback::from(move |new_type: CollectionType| {
            log::info!("Collection type changed");
            collection_type.set(new_type);
        })
    };

    let handle_sort_change = {
        let sort_criteria = sort_criteria.clone();
        Callback::from(move |new_criteria: SortCriteria| {
            log::info!("Sort criteria changed");
            sort_criteria.set(new_criteria);
        })
    };

    let handle_direction_change = {
        let sort_ascending = sort_ascending.clone();
        Callback::from(move |ascending: bool| {
            log::info!("Sort direction changed");
            sort_ascending.set(ascending);
        })
    };

    let handle_energy_update = {
        let creatures = creatures.clone();
        let selected_item = selected_item.clone();
        let fetch_data = fetch_data.clone();
        
        Callback::from(move |(creature_id, energy_full): (Uuid, bool)| {
            handlers::handle_energy(
                creature_id,
                energy_full,
                creatures.clone(),
                selected_item.clone(),
                fetch_data.clone(),
            ).emit(());
        })
    };

    let mut filtered_items = get_filtered_items(&collection_type, &eggs, &creatures, &scrolls);
    sort_items(&mut filtered_items, &sort_criteria, *sort_ascending);

    // Effect to control body scrolling when modal is open/closed
    {
        let selected_item = selected_item.clone();
        use_effect_with(selected_item, move |selected_item| {
            let maybe_body = window().and_then(|w| w.document()).and_then(|d| d.body());
            if let Some(body) = maybe_body {
                if selected_item.is_some() {
                    let _ = body.class_list().add_1("overflow-hidden");
                } else {
                    let _ = body.class_list().remove_1("overflow-hidden");
                }
            }
            // Cleanup function: Ensure overflow is removed when component unmounts or effect re-runs
            move || {
                if let Some(body) = window().and_then(|w| w.document()).and_then(|d| d.body()) {
                    let _ = body.class_list().remove_1("overflow-hidden");
                }
            }
        });
    }

    // Event listener to handle selecting items triggered from other parts (like after hatching/summoning)
    {
        let selected_item_setter = selected_item.setter();
        let creatures = creatures.clone();
        let eggs = eggs.clone();
        
        use_effect_with((), move |_| {
            let window = window().unwrap();
            let listener = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                if let Some(detail) = e.detail().as_string() {
                    if let Some((item_type, id_str)) = detail.split_once(':') {
                        if let Ok(id) = Uuid::parse_str(id_str) {
                            match item_type {
                                "creature" => {
                                    if let Some(creature) = creatures.iter().find(|c| c.id == id) {
                                        selected_item_setter.set(Some(DisplayItem::Creature(creature.clone())));
                                    }
                                },
                                "egg" => {
                                    if let Some(egg) = eggs.iter().find(|e| e.id == id) {
                                        selected_item_setter.set(Some(DisplayItem::Egg(egg.clone())));
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(web_sys::CustomEvent)>);

            window.add_event_listener_with_callback(
                "selectItem",
                listener.as_ref().unchecked_ref()
            ).unwrap();

            move || {
                window.remove_event_listener_with_callback(
                    "selectItem",
                    listener.as_ref().unchecked_ref()
                ).unwrap();
            }
        });
    }

    html! {
        <Base>
            <GradientBackground>
                <div class={classes!(
                    "min-h-screen", 
                    // Optionally re-add blur if desired: selected_item.is_some().then_some("filter blur-sm")
                )}>
                    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                        <div class="flex flex-col sm:flex-row sm:justify-between sm:items-center space-y-4 sm:space-y-0">
                            <h1 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-blue-600 to-purple-600 leading-relaxed pb-1">
                                {"Inventory"}
                            </h1>
                            <div class="flex items-center">
                                <FilterBar
                                    collection_type={(*collection_type).clone()}
                                    sort_criteria={(*sort_criteria).clone()}
                                    sort_ascending={*sort_ascending}
                                    on_collection_change={handle_collection_change}
                                    on_sort_change={handle_sort_change}
                                    on_direction_change={handle_direction_change}
                                />
                            </div>
                        </div>

                        <div class="mt-8">
                            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 relative">
                                {for filtered_items.into_iter().map(|item| {
                                    let item_clone = item.clone();
                                    let item_handle_click = handle_item_click(selected_item.clone());
                                    let all_creatures_for_card = creatures.clone();
                                    
                                    html! {
                                        <div> 
                                            <Display
                                                item={item_clone.clone()}
                                                mode={DisplayMode::Card}
                                                on_click={Some(item_handle_click)}
                                                on_close={Option::<Callback<()>>::None}
                                                on_action={Option::<Callback<()>>::None}
                                                action_label={Option::<String>::None}
                                                loading={false}
                                                error={String::new()}
                                                fetch_data={Option::<Callback<()>>::None}
                                                on_select_egg={Option::<Callback<Egg>>::None}
                                                on_energy_update={Some(handle_energy_update.clone())}
                                                available_creatures={Some((*all_creatures_for_card).clone())}
                                            />
                                        </div>
                                    }
                                })}
                            </div>
                        </div>
                    </div>
                </div>

                { if let Some(item_to_display) = selected_item.as_ref() {
                    let modal_handle_close = handle_close(selected_item.clone(), hatch_state.clone());
                    let modal_token = token.clone();
                    let modal_loading = loading.clone();
                    let modal_eggs = eggs.clone();
                    let modal_creatures = creatures.clone();
                    let modal_hatch_state = hatch_state.clone();
                    let modal_selected_item = selected_item.clone();
                    let modal_fetch_data = fetch_data.clone();
                    let modal_on_error = Callback::from(move |err: String| {
                         log::error!("Modal Action Error: {}", err);
                    });
                    let on_select_egg_eggs = modal_eggs.clone();
                    let on_select_egg_selected_item = modal_selected_item.clone();
                    let modal_on_select_egg = Callback::from(move |egg: Egg| {
                        let mut new_eggs = vec![egg.clone()];
                        new_eggs.extend((*on_select_egg_eggs).clone());
                        on_select_egg_eggs.set(new_eggs);
                        on_select_egg_selected_item.set(Some(DisplayItem::Egg(egg.clone())));
                    });

                    html! {
                        <>
                            <Display
                                item={item_to_display.clone()}
                                mode={DisplayMode::Focus}
                                on_close={Some(modal_handle_close.clone())}
                                on_click={Option::<Callback<DisplayItem>>::None}
                                on_action={
                                    match item_to_display {
                                        DisplayItem::Egg(egg) => {
                                            let egg_id = egg.id;
                                            let hatch_loading = modal_loading.clone();
                                            let hatch_eggs = modal_eggs.clone();
                                            let hatch_creatures = modal_creatures.clone();
                                            let hatch_token = modal_token.clone();
                                            let hatch_hatch_state = modal_hatch_state.clone();
                                            let hatch_selected_item = modal_selected_item.clone();
                                            let item_handle_hatch = handle_hatch(
                                                hatch_loading,
                                                hatch_eggs,
                                                hatch_creatures,
                                                hatch_token,
                                                hatch_hatch_state,
                                                hatch_selected_item,
                                            );
                                            Some(Callback::from(move |_| item_handle_hatch.emit(egg_id)))
                                        },
                                        DisplayItem::Scroll(scroll) => {
                                             let scroll_clone = scroll.clone();
                                             let summon_handle_close = modal_handle_close.clone();
                                             let summon_eggs = modal_eggs.clone();
                                             let summon_token = modal_token.clone();
                                             let summon_selected_item = modal_selected_item.clone();
                                             let summon_on_error = modal_on_error.clone();
                                             let summon_fetch_data = modal_fetch_data.clone();
                                             Some(handlers::handle_summon(
                                                scroll_clone,
                                                summon_eggs,
                                                summon_token,
                                                summon_selected_item,
                                                summon_on_error,
                                                summon_handle_close,
                                                summon_fetch_data,
                                            ))
                                        },
                                        _ => None
                                    }
                                }
                                action_label={
                                    match item_to_display {
                                        DisplayItem::Egg(_) => Some("Hatch Egg".to_string()),
                                        DisplayItem::Scroll(_) => Some("Summon Egg (100 pax)".to_string()),
                                        _ => None,
                                    }
                                }
                                loading={
                                    match item_to_display {
                                        DisplayItem::Egg(egg) => modal_hatch_state.egg_id.map_or(false, |id| id == egg.id) && *modal_loading,
                                        _ => *modal_loading
                                    }
                                }
                                error={
                                    match item_to_display {
                                        DisplayItem::Egg(egg) => {
                                            if modal_hatch_state.egg_id.map_or(false, |id| id == egg.id) {
                                                (*modal_hatch_state).error.clone()
                                            } else { String::new() }
                                        },
                                         _ => String::new()
                                    }
                                }
                                fetch_data={Some(modal_fetch_data)}
                                on_select_egg={
                                    if matches!(item_to_display, DisplayItem::Scroll(_)) {
                                        Some(modal_on_select_egg)
                                    } else { None }
                                }
                                on_energy_update={Some(handle_energy_update.clone())}
                            />
                        </>
                    }
                 } else {
                    html! {}
                 }}
            </GradientBackground>
        </Base>
    }
}
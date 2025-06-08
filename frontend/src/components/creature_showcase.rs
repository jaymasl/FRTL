use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::models::ShowcaseCreatureData;
use crate::config::get_api_base_url;
use log::error;
use web_sys::{Element, MouseEvent, TouchEvent};
use gloo_events::EventListener;
use gloo_timers::callback::{Interval, Timeout};

#[function_component(CreatureShowcase)]
pub fn creature_showcase() -> Html {
    let creatures = use_state(|| Vec::<ShowcaseCreatureData>::new());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let scroll_container_ref = use_node_ref();
    let can_scroll_left = use_state(|| false);
    let can_scroll_right = use_state(|| false);
    
    // Touch event state
    let touch_start_x = use_state(|| 0);
    let is_touching = use_state(|| false);
    
    // Scrolling state
    let is_paused = use_state(|| false);
    let scroll_interval = use_mut_ref(|| None::<Interval>);

    // Check scroll position and update nav buttons visibility
    {
        let loading = loading.clone();
        let can_scroll_left = can_scroll_left.clone();
        let can_scroll_right = can_scroll_right.clone();
        let scroll_container_ref_for_effect = scroll_container_ref.clone();

        use_effect_with(loading.clone(), move |loading| {
            let mut listener: Option<EventListener> = None;
            
            if !**loading {
                let scroll_container_ref = scroll_container_ref_for_effect.clone();
                let can_scroll_left = can_scroll_left.clone();
                let can_scroll_right = can_scroll_right.clone();
                
                let check_scroll = move || {
                    if let Some(element) = scroll_container_ref.cast::<Element>() {
                        let scroll_left = element.scroll_left();
                        let scroll_width = element.scroll_width();
                        let client_width = element.client_width();
                        
                        can_scroll_left.set(scroll_left > 5);
                        can_scroll_right.set(scroll_left < scroll_width - client_width - 5);
                    } else {
                        can_scroll_left.set(false);
                        can_scroll_right.set(false);
                    }
                };
                
                check_scroll();

                listener = {
                    if let Some(element) = scroll_container_ref_for_effect.cast::<Element>() {
                        let check_scroll_clone = check_scroll.clone();
                        Some(EventListener::new(&element, "scroll", move |_event| {
                            check_scroll_clone();
                        }))
                    } else {
                        None
                    }
                };
            }
            
            move || { drop(listener); }
        });
    }

    // Fetch creatures data
    {
        let creatures = creatures.clone();
        let loading = loading.clone();
        let error = error.clone();
        let is_paused = is_paused.clone();
        let scroll_interval = scroll_interval.clone();
        let scroll_container_ref = scroll_container_ref.clone();

        use_effect_with((), move |_| {
            let creatures = creatures.clone();
            let loading = loading.clone();
            let error = error.clone();
            
            spawn_local(async move {
                loading.set(true);
                let api_base_url = get_api_base_url();
                let showcase_url = format!("{}/api/creatures/showcase", api_base_url);
                
                match Request::get(&showcase_url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<ShowcaseCreatureData>>().await {
                                Ok(fetched_creatures) => {
                                    // Limit to 15 creatures
                                    let limited_creatures = fetched_creatures.into_iter().take(15).collect();
                                    creatures.set(limited_creatures);
                                    error.set(None);
                                    loading.set(false);
                                    
                                    // Start auto-scrolling immediately after data loads
                                    let container_ref = scroll_container_ref.clone();
                                    let is_paused = is_paused.clone();
                                    
                                    // Wait a tiny bit for the DOM to update
                                    let _ = Timeout::new(100, move || {
                                        if !*is_paused {
                                            // Clear any existing interval
                                            *scroll_interval.borrow_mut() = None;
                                            
                                            // Create a smooth scrolling interval
                                            let interval = Interval::new(50, move || {
                                                if !*is_paused {
                                                    if let Some(element) = container_ref.cast::<Element>() {
                                                        let scroll_left = element.scroll_left();
                                                        let max_scroll = element.scroll_width() - element.client_width();

                                                        // Stop scrolling if we are at or near the end
                                                        if scroll_left < max_scroll - 5 {
                                                            element.set_scroll_left(scroll_left + 6); // Increase scroll speed
                                                        } else {
                                                            // Optionally stop the interval here if we want it to stop completely
                                                            // *scroll_interval.borrow_mut() = None; 
                                                        }
                                                    }
                                                }
                                            });
                                            
                                            // Store the interval
                                            *scroll_interval.borrow_mut() = Some(interval);
                                        }
                                    });
                                }
                                Err(e) => {
                                    error!("Failed to deserialize showcase creatures: {}", e);
                                    error.set(Some("Failed to load showcase data.".to_string()));
                                    loading.set(false);
                                }
                            }
                        } else {
                            let status = response.status();
                            let status_text = response.status_text();
                            error!("Failed to fetch showcase creatures: {} {}", status, status_text);
                            error.set(Some(format!("Error fetching showcase: {} {}", status, status_text)));
                            loading.set(false);
                        }
                    }
                    Err(e) => {
                        error!("Network error fetching showcase creatures: {}", e);
                        error.set(Some("Network error fetching showcase.".to_string()));
                        loading.set(false);
                    }
                }
            });

            || ()
        });
    }

    // Handle pause/resume of scrolling
    {
        let scroll_interval = scroll_interval.clone();
        let is_paused = is_paused.clone();
        let scroll_container_ref = scroll_container_ref.clone();
        
        use_effect_with(*is_paused, move |_| {
            // If paused and we have an interval, clear it
            if *is_paused {
                *scroll_interval.borrow_mut() = None;
            } else {
                // If not paused and we don't have an interval, start one
                if scroll_interval.borrow().is_none() {
                    let container_ref = scroll_container_ref.clone();
                    let is_paused = is_paused.clone();
                    
                    // Create a smooth scrolling interval
                    let interval = Interval::new(50, move || {
                        if !*is_paused {
                            if let Some(element) = container_ref.cast::<Element>() {
                                let scroll_left = element.scroll_left();
                                let max_scroll = element.scroll_width() - element.client_width();

                                // Stop scrolling if we are at or near the end
                                if scroll_left < max_scroll - 5 {
                                    element.set_scroll_left(scroll_left + 6); // Increase scroll speed
                                } else {
                                    // Optionally stop the interval here if we want it to stop completely
                                    // *scroll_interval.borrow_mut() = None;
                                }
                            }
                        }
                    });
                    
                    // Store the interval
                    *scroll_interval.borrow_mut() = Some(interval);
                }
            }
            
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }

    let get_rarity_class = |rarity: &str| -> &str {
        match rarity {
            "Mythical" => "bg-gradient-to-r from-yellow-400 via-red-500 to-pink-500 text-white shadow-lg",
            "Legendary" => "bg-gradient-to-r from-purple-500 to-indigo-600 text-white shadow-md",
            "Epic" => "bg-gradient-to-r from-pink-500 to-rose-500 text-white shadow-sm",
            "Rare" => "bg-gradient-to-r from-blue-400 to-cyan-500 text-white shadow-sm",
            "Uncommon" => "bg-gradient-to-r from-emerald-400 to-green-500 text-white shadow-sm",
            _ => "bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-gray-200 shadow-sm",
        }
    };

    // Pause animation on user interaction
    let toggle_pause = {
        let is_paused = is_paused.clone();
        Callback::from(move |_: MouseEvent| {
            is_paused.set(!*is_paused);
        })
    };

    let scroll_container_ref_clone = scroll_container_ref.clone();
    let scroll = {
        let toggle_pause = toggle_pause.clone();
        move |scroll_offset: i32| {
            toggle_pause.emit(MouseEvent::new("click").unwrap());
            if let Some(element) = scroll_container_ref_clone.cast::<Element>() {
                element.scroll_by_with_x_and_y(scroll_offset as f64, 0.0);
            }
        }
    };

    let scroll_left = {
        let scroll = scroll.clone();
        Callback::from(move |_| scroll(-300))
    };

    let scroll_right = Callback::from(move |_| scroll(300));
    
    // Touch event handlers
    let handle_touch_start = {
        let touch_start_x = touch_start_x.clone();
        let is_touching = is_touching.clone();
        let toggle_pause = toggle_pause.clone();
        
        Callback::from(move |e: TouchEvent| {
            e.prevent_default();
            toggle_pause.emit(MouseEvent::new("click").unwrap());
            
            if let Some(touch) = e.touches().get(0) {
                touch_start_x.set(touch.client_x());
                is_touching.set(true);
            }
        })
    };
    
    let handle_touch_move = {
        let touch_start_x = *touch_start_x;
        let is_touching = *is_touching;
        let scroll_container_ref = scroll_container_ref.clone();
        
        Callback::from(move |e: TouchEvent| {
            if is_touching {
                e.prevent_default();
                
                if let Some(touch) = e.touches().get(0) {
                    let current_x = touch.client_x();
                    let diff_x = touch_start_x - current_x;
                    
                    if let Some(element) = scroll_container_ref.cast::<Element>() {
                        element.scroll_by_with_x_and_y(diff_x as f64, 0.0);
                    }
                }
            }
        })
    };
    
    let handle_touch_end = {
        let is_touching = is_touching.clone();
        
        Callback::from(move |_: TouchEvent| {
            is_touching.set(false);
        })
    };

    html! {
        <div class="relative w-full py-6">
            <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-teal-400 via-cyan-500 to-sky-600 mb-3 text-center">
                {"Showcase"}
            </h2>

            {
                if *loading {
                    html! {
                        <div class="flex justify-center items-center h-40">
                            <div class="animate-spin rounded-full h-16 w-16 border-t-4 border-b-4 border-cyan-500"></div>
                        </div>
                    }
                } else if let Some(err_msg) = &*error {
                    html! {
                        <div class="text-center text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-900/30 border border-red-300 dark:border-red-700 rounded-lg p-4">
                            { format!("Error: {}", err_msg) }
                        </div>
                    }
                } else if creatures.is_empty() {
                     html! {
                        <div class="text-center text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800/30 rounded-lg p-6">
                            {"No creatures are currently featured in the showcase."}
                        </div>
                    }
                } else {
                    html! {
                        <div class="relative group">
                            <div 
                                ref={scroll_container_ref} 
                                class="overflow-x-auto pb-4 scroll-smooth [&::-webkit-scrollbar]:h-1.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-track]:bg-transparent [&::-webkit-scrollbar-thumb]:bg-gray-300 dark:[&::-webkit-scrollbar-thumb]:bg-gray-600 hover:[&::-webkit-scrollbar-thumb]:bg-gray-400 dark:hover:[&::-webkit-scrollbar-thumb]:bg-gray-500"
                                ontouchstart={handle_touch_start}
                                ontouchmove={handle_touch_move}
                                ontouchend={handle_touch_end}
                            >
                                <div class="flex space-x-6 whitespace-nowrap px-1">
                                    { for creatures.iter().map(|creature| {
                                        html! {
                                            <div class="inline-block w-60 flex-shrink-0 bg-white dark:bg-gray-800/50 rounded-xl shadow-lg hover:shadow-xl overflow-hidden transition-all duration-300 group border border-gray-200 dark:border-gray-700/50 backdrop-blur-sm hover:scale-[1.03]">
                                                <div class="aspect-square overflow-hidden">
                                                    <img 
                                                        src={creature.image_path.clone()} 
                                                        alt={format!("Image of {}", creature.display_name)} 
                                                        class="w-full h-full object-cover transition-transform duration-500" 
                                                    />
                                                </div>
                                                <div class={classes!("inline-block", "px-3", "py-1", "text-xs", "font-medium", "rounded-full", "mb-2", get_rarity_class(&creature.rarity))}>
                                                    { creature.rarity.clone() }
                                                </div>
                                                <p class="text-sm text-gray-600 dark:text-gray-400">
                                                    { "Owner: " }
                                                    <span class="font-medium text-gray-700 dark:text-gray-300">{ creature.owner_username.clone() }</span>
                                                </p>
                                                <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                                                    { "Hatched: " }
                                                    { 
                                                        match creature.hatched_at.split_once(' ') {
                                                            Some((date_part, _)) => {
                                                                let parts: Vec<&str> = date_part.split('-').collect();
                                                                if parts.len() == 3 {
                                                                    format!("{}-{}-{}", parts[2], parts[1], parts[0])
                                                                } else {
                                                                    date_part.to_string() 
                                                                }
                                                            },
                                                            None => {
                                                                creature.hatched_at.clone() 
                                                            }
                                                        }
                                                    }
                                                </p>
                                            </div>
                                        }
                                    })}
                                </div>
                            </div>

                            { if *can_scroll_left {
                                html! {
                                    <button 
                                        onclick={scroll_left}
                                        class="absolute top-1/2 left-0 transform -translate-y-1/2 -translate-x-4 z-10
                                            bg-white/60 dark:bg-gray-900/60 backdrop-blur-sm rounded-full p-2 
                                            text-gray-700 dark:text-gray-300 hover:text-black dark:hover:text-white 
                                            shadow-md hover:shadow-lg transition-all duration-300 
                                            opacity-80 hover:opacity-100"
                                        aria-label="Scroll Left"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor" class="w-6 h-6">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
                                        </svg>
                                    </button>
                                }
                            } else {
                                html! {}
                            }}

                            { if *can_scroll_right {
                                html! {
                                    <button 
                                        onclick={scroll_right}
                                        class="absolute top-1/2 right-0 transform -translate-y-1/2 translate-x-4 z-10
                                            bg-white/60 dark:bg-gray-900/60 backdrop-blur-sm rounded-full p-2 
                                            text-gray-700 dark:text-gray-300 hover:text-black dark:hover:text-white 
                                            shadow-md hover:shadow-lg transition-all duration-300 
                                            opacity-80 hover:opacity-100"
                                        aria-label="Scroll Right"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor" class="w-6 h-6">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5" />
                                        </svg>
                                    </button>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                }
            }
        </div>
    }
}
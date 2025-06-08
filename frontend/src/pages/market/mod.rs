use yew::prelude::*;
use crate::{
    base::Base,
    hooks::auth_state::{use_auth_check, use_auth_token},
    components::displays::{Display, DisplayItem, DisplayMode},
    models::{Egg, Creature},
    config::{get_asset_url, get_api_base_url},
};
use crate::components::GradientBackground;
use gloo_net::http::Request;
use serde::Deserialize;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use log;

mod handlers;
mod refresh;
mod orderbook_modal;
use orderbook_modal::OrderbookModal;

#[derive(Debug, Deserialize, Clone)]
pub struct MarketListing {
    pub id: Uuid,
    pub seller_id: Uuid,
    pub seller_username: String,
    pub item_id: Uuid,
    pub item_type: String,
    pub price: i32,
    pub status: String,
    pub created_at: String,
    #[serde(skip)]
    pub item: Option<DisplayItem>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListingState {
    pub loading: bool,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CreateListingStep {
    Closed,
    SelectItem,
    SetPrice(DisplayItem),
}

#[function_component(Market)]
pub fn market() -> Html {
    use_auth_check();
    let token = use_auth_token();
    let listings = use_state(|| Vec::<MarketListing>::new());
    let error = use_state(String::new);
    let listing_states = use_state(|| std::collections::HashMap::<Uuid, ListingState>::new());
    let create_step = use_state(|| CreateListingStep::Closed);
    let price_input = use_state(String::new);
    let inventory = use_state(Vec::new);
    let refresh_counter = use_state(|| 0);
    let selected_item = use_state(|| None::<DisplayItem>);
    let is_refreshing = use_state(|| false);
    let animation_active = use_state(|| false);
    let show_orderbook = use_state(|| false);

    // Get current user ID from local storage and session storage
    let current_user_id = {
        let user_id = window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|storage| storage.get_item("user_id").ok().flatten())
            .or_else(|| window()
                .and_then(|w| w.session_storage().ok().flatten())
                .and_then(|storage| storage.get_item("user_id").ok().flatten()))
            .and_then(|id| {
                log::info!("Found user_id in storage: {}", &id);
                Uuid::parse_str(&id).ok()
            });
        
        if user_id.is_none() {
            log::warn!("No user_id found in local storage or session storage");
        }
        
        user_id
    };

    // Fetch market listings
    {
        let listings = listings.clone();
        let token = token.clone();
        let error = error.clone();
        let is_refreshing = is_refreshing.clone();
        let refresh_counter = refresh_counter.clone();

        use_effect_with((*refresh_counter, token.clone()), move |(_, token)| {
            if token.is_empty() {
                let cleanup: Box<dyn FnOnce()> = Box::new(|| ());
                return cleanup;
            }

            let interval = refresh::setup_refresh(token.clone(), listings, error, is_refreshing);
            let cleanup: Box<dyn FnOnce()> = Box::new(move || drop(interval));
            cleanup
        });
    }

    // Fetch inventory when creating listing
    {
        let token = token.clone();
        let inventory = inventory.clone();
        let create_step = create_step.clone();
        let listings = listings.clone();

        use_effect_with(create_step, move |step| {
            if matches!(**step, CreateListingStep::SelectItem) && !token.is_empty() {
                spawn_local(async move {
                    let mut items = Vec::new();
                    let active_listing_ids: std::collections::HashSet<_> = listings
                        .iter()
                        .filter_map(|listing| listing.item.as_ref())
                        .map(|item| match item {
                            DisplayItem::Egg(egg) => egg.id,
                            DisplayItem::Creature(creature) => creature.id,
                            DisplayItem::Scroll(scroll) => scroll.id,
                        })
                        .collect();

                    // Fetch eggs
                    if let Ok(response) = Request::get(&format!("{}/api/eggs", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(eggs) = response.json::<Vec<Egg>>().await {
                                items.extend(eggs
                                    .into_iter()
                                    .filter(|egg| !active_listing_ids.contains(&egg.id))
                                    .map(DisplayItem::Egg));
                            }
                        }
                    }

                    // Fetch creatures
                    if let Ok(response) = Request::get(&format!("{}/api/creatures", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(creatures) = response.json::<Vec<Creature>>().await {
                                items.extend(creatures
                                    .into_iter()
                                    .filter(|creature| !active_listing_ids.contains(&creature.id) && creature.status == "available")
                                    .map(DisplayItem::Creature));
                            }
                        }
                    }

                    inventory.set(items);
                });
            }
            || ()
        });
    }

    // Add effect to handle animation timing
    {
        let animation_active = animation_active.clone();
        let is_refreshing = is_refreshing.clone();
        
        use_effect_with(is_refreshing.clone(), move |is_refreshing| {
            if **is_refreshing {
                let animation_active = animation_active.clone();
                let is_refreshing = is_refreshing.clone();
                spawn_local(async move {
                    // Start the animation
                    animation_active.set(true);
                    
                    // Wait for the minimum animation duration (1.2s for the full animation cycle)
                    gloo_timers::future::TimeoutFuture::new(1200).await;
                    
                    // Only stop the animation if the refresh has completed
                    if !*is_refreshing {
                        animation_active.set(false);
                    }
                });
            }
            || ()
        });
    }

    let handle_buy = handlers::handle_buy(
        token.clone(),
        listings.clone(),
        listing_states.clone(),
    );

    let handle_create_listing = handlers::handle_create_listing(
        token.clone(),
        listings.clone(),
        create_step.clone(),
        price_input.clone(),
        inventory.clone(),
        error.clone(),
        refresh_counter.clone(),
    );

    let open_create_modal = {
        let create_step = create_step.clone();
        Callback::from(move |_| create_step.set(CreateListingStep::SelectItem))
    };

    let close_modal = {
        let create_step = create_step.clone();
        Callback::from(move |_| create_step.set(CreateListingStep::Closed))
    };

    let select_item = {
        let create_step = create_step.clone();
        Callback::from(move |item: DisplayItem| create_step.set(CreateListingStep::SetPrice(item)))
    };

    let handle_price_change = handlers::handle_price_change(price_input.clone());

    let handle_cancel = handlers::handle_cancel(
        token.clone(),
        listings.clone(),
        error.clone(),
        refresh_counter.clone(),
    );

    let handle_item_click = {
        let selected_item = selected_item.clone();
        Callback::from(move |item: DisplayItem| {
            selected_item.set(Some(item));
        })
    };

    let handle_close = {
        let selected_item = selected_item.clone();
        Callback::from(move |_| {
            selected_item.set(None);
        })
    };

    html! {
        <Base>
            <GradientBackground>
                <div class="min-h-screen p-4 sm:p-6 lg:p-8">
                    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                        <div class="flex flex-col sm:flex-row sm:justify-between sm:items-center gap-4">
                            <div class="flex items-center gap-4">
                                <h1 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-blue-600 to-purple-600">
                                    {"Marketplace"}
                                </h1>
                                <div class="flex items-center gap-3 px-4 py-2 rounded-full">
                                    <div class="relative">
                                        <div class={classes!(
                                            "w-3",
                                            "h-3",
                                            "rounded-full",
                                            "bg-green-500",
                                            "transition-opacity",
                                            "duration-300",
                                            if !*is_refreshing { "opacity-50" } else { "" }
                                        )} />
                                        {if *animation_active {
                                            html! {
                                                <>
                                                    <div class="absolute -top-1 -left-1 w-5 h-5 rounded-full bg-green-500 animate-[ping_1.2s_cubic-bezier(0,0,0.2,1)_infinite] opacity-60" />
                                                    <div class="absolute -top-2 -left-2 w-7 h-7 rounded-full bg-green-500 animate-[ping_1.2s_cubic-bezier(0,0,0.2,1)_infinite] opacity-30 delay-[150ms]" />
                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }}
                                    </div>
                                    <span class={classes!(
                                        "text-base",
                                        "font-semibold",
                                        "tracking-wide",
                                        "transition-colors",
                                        "duration-300",
                                        if *is_refreshing {
                                            "text-green-600 dark:text-green-400"
                                        } else {
                                            "text-gray-600 dark:text-gray-400"
                                        }
                                    )}>
                                        {"Live"}
                                    </span>
                                </div>
                            </div>
                            <div class="flex gap-3">
                                <button
                                    onclick={open_create_modal.clone()}
                                    class="px-4 py-2 bg-gradient-to-r from-blue-500 to-purple-500 text-white rounded-lg hover:opacity-90 transition-opacity flex-1 sm:flex-none"
                                >
                                    {"Create Listing"}
                                </button>
                                <button
                                    onclick={{
                                        let show_orderbook = show_orderbook.clone();
                                        Callback::from(move |_| show_orderbook.set(true))
                                    }}
                                    class="px-4 py-2 bg-gradient-to-r from-cyan-400 to-blue-500 text-white rounded-lg hover:opacity-90 transition-all duration-200 shadow-lg ring-1 ring-cyan-400/30 flex-1 sm:flex-none"
                                >
                                    {"Scroll Order Book"}
                                </button>
                            </div>
                        </div>

                        if !error.is_empty() {
                            <div class="mt-4 p-4 bg-red-100 text-red-700 rounded-lg">
                                {(*error).clone()}
                            </div>
                        }

                        {match *create_step {
                            CreateListingStep::Closed => html! {},
                            _ => html! {
                                <div 
                                    class="fixed inset-0 z-40 bg-white/90 dark:bg-black/90 backdrop-blur-md overflow-y-auto"
                                    onclick={close_modal.clone()}
                                >
                                    <div class="min-h-screen px-4 py-16 md:py-20 flex items-start justify-center">
                                        <div 
                                            class={classes!(
                                                "relative",
                                                "bg-white",
                                                "dark:bg-gray-800",
                                                "rounded-xl",
                                                "p-6",
                                                "shadow-xl",
                                                "w-full",
                                                match *create_step {
                                                    CreateListingStep::SelectItem => "max-w-5xl max-h-[85vh] overflow-hidden flex flex-col",
                                                    CreateListingStep::SetPrice(_) => "max-w-2xl",
                                                    CreateListingStep::Closed => unreachable!(),
                                                },
                                            )}
                                            onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                                        >
                                            <div class="flex justify-between items-center mb-4">
                                                <div class="flex items-center gap-4">
                                                    {match *create_step {
                                                        CreateListingStep::SetPrice(_) => html! {
                                                            <button
                                                                onclick={open_create_modal.clone()}
                                                                class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                                            >
                                                                <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                                                                </svg>
                                                            </button>
                                                        },
                                                        _ => html! {}
                                                    }}
                                                    <h2 class="text-2xl font-bold text-gray-900 dark:text-white">
                                                        {match *create_step {
                                                            CreateListingStep::SelectItem => "Select Item to List",
                                                            CreateListingStep::SetPrice(_) => "Set Price",
                                                            CreateListingStep::Closed => unreachable!(),
                                                        }}
                                                    </h2>
                                                </div>
                                                <button
                                                    onclick={close_modal}
                                                    class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                                >
                                                    {"✕"}
                                                </button>
                                            </div>

                                            {match &*create_step {
                                                CreateListingStep::SelectItem => html! {
                                                    <div class="overflow-y-auto flex-grow pr-2">
                                                        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                                            {for inventory.iter().map(|item| {
                                                                let item_clone = item.clone();
                                                                html! {
                                                                    <div 
                                                                        onclick={select_item.reform(move |_| item_clone.clone())}
                                                                        class="cursor-pointer"
                                                                    >
                                                                        <Display
                                                                            item={item.clone()}
                                                                            mode={DisplayMode::Card}
                                                                            on_click={None::<Callback<DisplayItem>>}
                                                                            on_close={None::<Callback<()>>}
                                                                            on_action={None::<Callback<()>>}
                                                                            action_label={None::<String>}
                                                                            loading={false}
                                                                            error={"".to_string()}
                                                                            handle_hatch={None::<Callback<Uuid>>}
                                                                            fetch_data={None::<Callback<()>>}
                                                                            on_energy_update={None::<Callback<(Uuid, bool)>>}
                                                                        />
                                                                    </div>
                                                                }
                                                            })}
                                                        </div>
                                                    </div>
                                                },
                                                CreateListingStep::SetPrice(item) => {
                                                    let item_clone = item.clone();
                                                    html! {
                                                        <div class="space-y-4">
                                                            <div class="max-w-md mx-auto">
                                                                <Display
                                                                    item={item.clone()}
                                                                    mode={DisplayMode::Card}
                                                                    on_click={None::<Callback<DisplayItem>>}
                                                                    on_close={None::<Callback<()>>}
                                                                    on_action={None::<Callback<()>>}
                                                                    action_label={None::<String>}
                                                                    loading={false}
                                                                    error={"".to_string()}
                                                                    handle_hatch={None::<Callback<Uuid>>}
                                                                    fetch_data={None::<Callback<()>>}
                                                                    on_energy_update={None::<Callback<(Uuid, bool)>>}
                                                                />
                                                                <div class="flex items-center gap-4 mt-4">
                                                                    <input
                                                                        type="number"
                                                                        min="1"
                                                                        placeholder="Enter price in pax"
                                                                        value={(*price_input).clone()}
                                                                        oninput={handle_price_change.clone()}
                                                                        class="flex-1 px-4 py-2 border rounded-lg"
                                                                    />
                                                                    <button
                                                                        onclick={handle_create_listing.reform(move |_| item_clone.clone())}
                                                                        disabled={price_input.parse::<i32>().unwrap_or(0) <= 0}
                                                                        class="px-4 py-2 bg-gradient-to-r from-blue-500 to-purple-500 text-white rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed"
                                                                    >
                                                                        {"Create Listing"}
                                                                    </button>
                                                                </div>
                                                                if !error.is_empty() {
                                                                    <div class="mt-2 text-sm text-red-600 dark:text-red-400">
                                                                        {(*error).clone()}
                                                                    </div>
                                                                }
                                                                <div class="mt-2 text-sm text-gray-500 dark:text-gray-400">
                                                                    {"A flat fee of 5 pax will be charged for creating this listing."}
                                                                </div>
                                                            </div>
                                                        </div>
                                                    }
                                                },
                                                CreateListingStep::Closed => html! {},
                                            }}
                                        </div>
                                    </div>
                                </div>
                            }
                        }}

                        <div class="mt-8">
                            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
                                {for (*listings).iter().filter_map(|listing| {
                                    let listing_id = listing.id;
                                    let state = (*listing_states).get(&listing_id)
                                        .cloned()
                                        .unwrap_or_default();

                                    listing.item.as_ref().map(|item| {
                                        let is_owner = current_user_id.map_or(false, |id| {
                                            let ownership_match = listing.seller_id == id;
                                            log::info!(
                                                "Checking ownership - Listing seller_id: {}, Current user_id: {}, Is owner: {}", 
                                                listing.seller_id, 
                                                id,
                                                ownership_match
                                            );
                                            ownership_match
                                        });

                                        let item_clone = item.clone();
                                        let item_for_click = item.clone();

                                        html! {
                                            <div class="relative flex flex-col h-full">
                                                <div class="flex-grow">
                                                    if is_owner {
                                                        <button 
                                                            onclick={handle_cancel.reform(move |_| listing_id)}
                                                            class="absolute top-2 right-2 z-20 p-1 rounded-full bg-red-100 dark:bg-red-900 hover:bg-red-200 dark:hover:bg-red-800 text-red-600 dark:text-red-400"
                                                            title="Cancel Listing"
                                                        >
                                                            <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                                                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
                                                            </svg>
                                                        </button>
                                                    }
                                                    <Display
                                                        item={item_clone.clone()}
                                                        mode={if selected_item.as_ref().map_or(false, |selected| match (selected, item) {
                                                            (DisplayItem::Egg(selected_egg), DisplayItem::Egg(egg)) => 
                                                                selected_egg.id == egg.id,
                                                            (DisplayItem::Creature(selected_creature), DisplayItem::Creature(creature)) => 
                                                                selected_creature.id == creature.id,
                                                            _ => false
                                                        }) { DisplayMode::Market } else { DisplayMode::Card }}
                                                        on_click={handle_item_click.reform(move |_| item_for_click.clone())}
                                                        on_close={Some(handle_close.clone())}
                                                        on_action={if !is_owner { Some(handle_buy.reform(move |_| listing_id)) } else { None }}
                                                        action_label={if !is_owner { Some("Buy Now".to_string()) } else { None }}
                                                        loading={state.loading}
                                                        error={state.error}
                                                        handle_hatch={None::<Callback<Uuid>>}
                                                        fetch_data={None::<Callback<()>>}
                                                        on_energy_update={None::<Callback<(Uuid, bool)>>}
                                                    />
                                                </div>
                                                <div class="mt-1 p-1 bg-black bg-opacity-75 text-white rounded-lg">
                                                    <div class="flex justify-between items-center">
                                                        <span class="text-sm">{"Listed by: "}{&listing.seller_username}</span>
                                                        <span class="font-bold flex items-center">
                                                            {listing.price}
                                                            <span class="ml-1">
                                                                <img 
                                                                    src={get_asset_url("/static/images/pax-icon-white-0.png")}
                                                                    alt="pax icon" 
                                                                    class="block w-5 h-5" 
                                                                />
                                                            </span>
                                                        </span>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })
                                })}
                            </div>
                        </div>
                    </div>
                </div>
            </GradientBackground>
            { if *show_orderbook {
                html! {
                    <OrderbookModal on_close={{
                        let show_orderbook = show_orderbook.clone();
                        Callback::from(move |_| show_orderbook.set(false))
                    }} />
                }
            } else { html! {} } }
        </Base>
    }
}
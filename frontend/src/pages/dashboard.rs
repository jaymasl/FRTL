use yew::prelude::*;
use crate::hooks::auth_state::{use_auth_state, use_auth_token};
use crate::{Route, base::Base};
use yew_router::prelude::Link;
use crate::hooks::use_currency::use_currency;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::models::{Creature, Egg, Scroll};
use crate::config::get_api_base_url;
use crate::components::GradientBackground;

#[function_component(Dashboard)]
pub fn dashboard() -> Html {
    let auth_state = use_auth_state();
    let pax_balance = use_currency();
    let token = use_auth_token();
    let creatures_count = use_state(|| 0);
    let eggs_count = use_state(|| 0);
    let scrolls_count = use_state(|| 0);

    // Fetch creatures and eggs counts
    {
        let token = token.clone();
        let creatures_count = creatures_count.clone();
        let eggs_count = eggs_count.clone();
        let scrolls_count = scrolls_count.clone();
        
        use_effect_with((), move |_| {
            if !token.is_empty() {
                // Fetch creatures count
                let token_clone = token.clone();
                let creatures_count = creatures_count.clone();
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/creatures", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token_clone))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<Vec<Creature>>().await {
                                creatures_count.set(data.len());
                            }
                        }
                    }
                });

                // Fetch eggs count
                let token_clone = token.clone();
                let eggs_count = eggs_count.clone();
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/eggs", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token_clone))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<Vec<Egg>>().await {
                                eggs_count.set(data.len());
                            }
                        }
                    }
                });
                
                // Fetch scrolls count
                let token_clone = token.clone();
                let scrolls_count = scrolls_count.clone();
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/scrolls", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token_clone))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<Vec<Scroll>>().await {
                                // Sum up the quantities of all scrolls instead of just counting entries
                                let total_scrolls = data.iter().map(|scroll| scroll.quantity).sum::<i32>();
                                scrolls_count.set(total_scrolls as usize);
                            }
                        }
                    }
                });
            }
            || ()
        });
    }

    html! {
        <Base>
            <GradientBackground>
                <div class="relative z-10 w-full px-4 sm:px-6 lg:px-8">
                    if auth_state {
                        <div class="relative z-10 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
                            <div class="flex flex-col md:flex-row justify-between items-start md:items-center gap-6 mb-12">
                                <div>
                                    <h1 class="text-5xl font-black bg-clip-text text-transparent bg-gradient-to-r from-blue-600 via-purple-600 to-violet-600 tracking-tight mb-2">
                                        {"Dashboard"}
                                    </h1>
                                </div>
                            </div>

                            <div class="flex flex-col lg:flex-row gap-6">
                                // Status cards - stacked on the left
                                <div class="flex flex-col gap-4 lg:w-1/6">
                                    {status_card("Pax Balance", &format!("{}", *pax_balance), "", "bg-gradient-to-br from-amber-50 to-amber-100 dark:from-amber-900/40 dark:to-amber-800/20", "text-amber-600 dark:text-amber-400", "💎")}
                                    {status_card("Creatures", &format!("{}", *creatures_count), "", "bg-gradient-to-br from-emerald-50 to-emerald-100 dark:from-emerald-900/40 dark:to-emerald-800/20", "text-emerald-600 dark:text-emerald-400", "🐉")}
                                    {status_card("Eggs", &format!("{}", *eggs_count), "", "bg-gradient-to-br from-violet-50 to-violet-100 dark:from-violet-900/40 dark:to-violet-800/20", "text-violet-600 dark:text-violet-400", "🥚")}
                                    {status_card("Scrolls", &format!("{}", *scrolls_count), "", "bg-gradient-to-br from-blue-50 to-blue-100 dark:from-blue-900/40 dark:to-blue-800/20", "text-blue-600 dark:text-blue-400", "📜")}
                                </div>
                                
                                // Dashboard cards - taking up more space on the right
                                <div class="lg:w-5/6">
                                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                                        {dashboard_card_enhanced(Route::Inventory, "Inventory", "View and manage your growing collection", "View Inventory", "emoji-inventory", "📦")}
                                        {dashboard_card_enhanced(Route::Profile, "Profile", "Customize your avatar and update personal settings", "View Profile", "emoji-profile", "👤")}
                                        {dashboard_card_enhanced(Route::Games, "Games", "Play games and earn rewards in the arcade", "Play Games", "emoji-games", "🎮")}
                                        {dashboard_card_enhanced(Route::Market, "Market", "Discover, trade and collect new magical creatures", "Visit Market", "emoji-market", "🛒")}
                                    </div>
                                </div>
                            </div>
                        </div>
                    } else {
                        <div class="flex flex-col items-center justify-center py-20">
                            <div class="bg-white/70 dark:bg-gray-800/60 backdrop-blur-sm rounded-2xl p-8 shadow-lg border border-white/30 dark:border-gray-700/30 text-center max-w-lg">
                                <h2 class="text-3xl font-bold text-gray-900 dark:text-white mb-4">{"Please Log In"}</h2>
                                <p class="text-gray-600 dark:text-gray-300 mb-8">{"You need to be logged in to view your dashboard."}</p>
                                <div class="flex justify-center gap-4">
                                    <Link<Route> to={Route::Login}>
                                        <button class="px-8 py-3 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-lg">
                                            {"Log In"}
                                        </button>
                                    </Link<Route>>
                                    <Link<Route> to={Route::Register}>
                                        <button class="px-8 py-3 border border-blue-600 text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/20 font-medium rounded-lg">
                                            {"Sign Up"}
                                        </button>
                                    </Link<Route>>
                                </div>
                            </div>
                        </div>
                    }
                </div>
            </GradientBackground>
        </Base>
    }
}

// Enhanced dashboard cards with more sophisticated visual effects
fn dashboard_card_enhanced(route: Route, title: &str, description: &str, button_text: &str, card_type: &str, emoji: &str) -> Html {
    let (gradient_from, gradient_to, button_gradient, icon_bg, hover_effect, _accent_light) = match card_type {
        "emoji-inventory" => ("from-blue-500", "to-blue-600", "from-blue-600 to-blue-700", "bg-blue-100 dark:bg-blue-900/40", "group-hover:shadow-blue-200/50 dark:group-hover:shadow-blue-500/30", "bg-blue-500/10"),
        "emoji-profile" => ("from-purple-500", "to-purple-600", "from-purple-600 to-purple-700", "bg-purple-100 dark:bg-purple-900/40", "group-hover:shadow-purple-200/50 dark:group-hover:shadow-purple-500/30", "bg-purple-500/10"),
        "emoji-games" => ("from-amber-500", "to-amber-600", "from-amber-600 to-amber-700", "bg-amber-100 dark:bg-amber-900/40", "group-hover:shadow-amber-200/50 dark:group-hover:shadow-amber-500/30", "bg-amber-500/10"),
        "emoji-market" => ("from-emerald-500", "to-emerald-600", "from-emerald-600 to-emerald-700", "bg-emerald-100 dark:bg-emerald-900/40", "group-hover:shadow-emerald-200/50 dark:group-hover:shadow-emerald-500/30", "bg-emerald-500/10"),
        _ => ("from-gray-500", "to-gray-600", "from-gray-600 to-gray-700", "bg-gray-100 dark:bg-gray-900/40", "group-hover:shadow-gray-200/50 dark:group-hover:shadow-gray-500/30", "bg-gray-500/10"),
    };

    html! {
        <div class="group perspective-1000">
            <div class={format!("backdrop-blur-sm rounded-2xl p-6 shadow-lg bg-white/70 dark:bg-gray-800/60 border border-white/30 dark:border-gray-700/30 relative overflow-hidden h-full flex flex-col justify-between transition-all duration-500 {}", hover_effect)}>
                // Subtle background glow
                <div class={format!("absolute inset-0 opacity-0 group-hover:opacity-20 transition-opacity duration-500 bg-gradient-to-br {} {}", gradient_from, gradient_to)}></div>
                
                <div class="relative z-10">
                    // Centered emoji at the top
                    <div class="flex justify-center items-center mb-6">
                        <div class={format!("w-20 h-20 rounded-2xl flex items-center justify-center text-5xl shadow-sm {} transition-all duration-500", icon_bg)}>
                            {emoji}
                        </div>
                    </div>
                    
                    <div class="text-center mb-4">
                        <span class={format!("text-xs font-semibold px-3 py-1 rounded-full text-white bg-gradient-to-r {} {} shadow-sm transform transition-transform duration-300 group-hover:scale-110 inline-block", gradient_from, gradient_to)}>
                            {title}
                        </span>
                    </div>
                    
                    <p class="text-sm text-gray-600 dark:text-gray-400 mb-8 text-center">{description}</p>
                </div>
                
                <Link<Route> to={route} classes="block mt-auto">
                    <button class={format!("w-full py-3 rounded-xl text-white font-semibold transition-all bg-gradient-to-r {} shadow-sm hover:shadow-md group-hover:shadow-lg relative overflow-hidden group", button_gradient)}>
                        <span class="relative z-10 flex items-center justify-center gap-1">
                            {button_text}
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 transition-transform duration-300 group-hover:translate-x-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                            </svg>
                        </span>
                        <span class="absolute inset-0 w-full h-full transition-all duration-300 ease-out transform translate-x-full bg-black/10 group-hover:translate-x-0"></span>
                    </button>
                </Link<Route>>
            </div>
        </div>
    }
}

// Enhanced status card component for the dashboard with centered text
fn status_card(title: &str, value: &str, _subtitle: &str, bg_style: &str, text_style: &str, icon: &str) -> Html {
    html! {
        <div class={format!("rounded-xl p-4 border border-white/30 dark:border-gray-700/30 shadow-md hover:shadow-lg transition-all duration-300 {} backdrop-blur-sm flex flex-col items-center text-center", bg_style)}>
            <div class={format!("w-12 h-12 rounded-lg {} flex items-center justify-center text-2xl mb-2", text_style)}>
                {icon}
            </div>
            <p class="text-sm font-medium text-gray-600 dark:text-gray-400">{title}</p>
            <h3 class="text-2xl font-bold text-gray-900 dark:text-white">{value}</h3>
        </div>
    }
} 
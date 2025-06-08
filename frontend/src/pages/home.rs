use yew::prelude::*;
use crate::hooks::auth_state::use_auth_state;
use crate::components::GradientBackground;
use crate::components::magic_button::MagicButton;
use crate::components::CreatureShowcase;
use crate::models::GlobalStats;
use crate::hooks::auth_state::use_auth_token;
use crate::{Route, base::Base};
use yew_router::prelude::Link;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::config::get_api_base_url;
use crate::models::{Creature, Egg, Scroll};
use crate::components::user_leaderboard::UserLeaderboard;

// Patreon Banner Component
#[function_component(PatreonBanner)]
fn patreon_banner() -> Html {
    html! {
        <div class="w-full bg-gradient-to-r from-orange-50 to-orange-100 dark:from-orange-900/30 dark:to-orange-800/40 backdrop-blur-sm border-b border-orange-200 dark:border-orange-800/30 shadow-sm relative overflow-hidden">
            <div class="absolute inset-0 overflow-hidden">
                <div class="absolute top-0 left-1/4 w-64 h-64 bg-orange-400/10 rounded-full filter blur-3xl animate-blob-move"></div>
                <div class="absolute bottom-0 right-1/4 w-64 h-64 bg-orange-500/10 rounded-full filter blur-3xl animate-blob-move-2"></div>
            </div>
            
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-3 flex flex-col sm:flex-row items-center justify-between relative z-10">
                <div class="flex items-center space-x-3 mb-2 sm:mb-0">
                    <div>
                        <h3 class="text-sm font-semibold text-orange-800 dark:text-orange-300">{"Support FRTL on Patreon"}</h3>
                        <p class="text-xs text-orange-700/80 dark:text-orange-400/80">{"Get exclusive benefits & support development"}</p>
                    </div>
                </div>
                
                <div class="flex items-center">
                    // Badge showing member benefits
                    <div class="hidden md:flex items-center mr-4 px-3 py-1 bg-orange-100 dark:bg-orange-900/40 rounded-full">
                        <span class="text-xs font-medium text-orange-700 dark:text-orange-300">{"✨ Daily rewards & exclusive features"}</span>
                    </div>
                    
                    <a 
                        href="https://www.patreon.com/FRTL" 
                        target="_blank" 
                        rel="noopener noreferrer"
                        class="inline-flex items-center px-4 py-2 bg-gradient-to-r from-orange-500 to-orange-600 text-white text-sm font-medium rounded-lg shadow-sm hover:shadow-md transition-all duration-300 hover:scale-105 group relative overflow-hidden"
                    >
                        // Add subtle shine effect on hover
                        <div class="absolute inset-0 w-full h-full bg-gradient-to-r from-transparent via-white/20 to-transparent opacity-0 group-hover:opacity-100 -translate-x-full group-hover:translate-x-full transition-all duration-1000 ease-out"></div>
                        
                        <span class="mr-2 relative z-10">{"Become a Member"}</span>
                        <svg class="w-5 h-5 group-hover:translate-x-0.5 transition-transform relative z-10" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <path d="M14 5L21 12M21 12L14 19M21 12H3" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                    </a>
                </div>
            </div>
        </div>
    }
}

// Function to fetch global stats
fn fetch_global_stats(stats_setter: UseStateHandle<Option<GlobalStats>>, loading_setter: UseStateHandle<bool>) {
    loading_setter.set(true);
    
    spawn_local(async move {
        match Request::get(&format!("{}/api/stats/global", get_api_base_url()))
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == 200 {
                    if let Ok(data) = response.json::<GlobalStats>().await {
                        stats_setter.set(Some(data));
                    }
                }
                loading_setter.set(false);
            }
            Err(_) => {
                loading_setter.set(false);
            }
        }
    });
}

#[function_component(Home)]
pub fn home() -> Html {
    let auth_state = use_auth_state();
    
    // Create state for global stats
    let global_stats = use_state(|| None::<GlobalStats>);
    let loading_global_stats = use_state(|| true);
    
    // For authenticated users, also fetch their personal stats
    let token = use_auth_token();
    let personal_stats = use_state(|| GlobalStats {
        scrolls_count: 0,
        eggs_count: 0,
        creatures_count: 0,
        total_soul: 0,
    });
    
    // Initial fetch of global stats when the page loads
    {
        let global_stats = global_stats.clone();
        let loading_global_stats = loading_global_stats.clone();
        
        use_effect_with((), move |_| {
            // Initial fetch
            fetch_global_stats(global_stats.clone(), loading_global_stats.clone());
            || ()
        });
    }
    
    // Fetch personal stats for authenticated users
    {
        let token = token.clone();
        let personal_stats_clone = personal_stats.clone();
        
        use_effect_with((), move |_| {
            if !token.is_empty() {
                // Fetch scrolls count
                let token_clone = token.clone();
                let personal_stats = personal_stats_clone.clone();
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/scrolls", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token_clone))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<Vec<Scroll>>().await {
                                personal_stats.set(GlobalStats {
                                    scrolls_count: data.len() as i64,
                                    eggs_count: personal_stats.eggs_count,
                                    creatures_count: personal_stats.creatures_count,
                                    total_soul: personal_stats.total_soul,
                                });
                            }
                        }
                    }
                });
                
                // Fetch eggs count
                let token_clone = token.clone();
                let personal_stats = personal_stats_clone.clone();
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/eggs", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token_clone))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<Vec<Egg>>().await {
                                personal_stats.set(GlobalStats {
                                    scrolls_count: personal_stats.scrolls_count,
                                    eggs_count: data.len() as i64,
                                    creatures_count: personal_stats.creatures_count,
                                    total_soul: personal_stats.total_soul,
                                });
                            }
                        }
                    }
                });
                
                // Fetch creatures count
                let token_clone = token.clone();
                let personal_stats = personal_stats_clone.clone();
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/creatures", get_api_base_url()))
                        .header("Authorization", &format!("Bearer {}", token_clone))
                        .send()
                        .await 
                    {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<Vec<Creature>>().await {
                                personal_stats.set(GlobalStats {
                                    scrolls_count: personal_stats.scrolls_count,
                                    eggs_count: personal_stats.eggs_count,
                                    creatures_count: data.len() as i64,
                                    total_soul: personal_stats.total_soul,
                                });
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
                // Add Patreon Banner at the top for both logged-in and logged-out users
                <PatreonBanner />
                
                <div class="relative z-10 w-full px-4 sm:px-6 lg:px-8">
                    if auth_state {
                        // Enhanced welcome message with magic button first
                        <div class="relative z-10 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 pt-6 pb-4 text-center">
                            
                            // Magic Button Section moved up, reduced bottom margin
                            <div class="bg-white/30 dark:bg-gray-900/30 backdrop-blur-sm rounded-xl shadow-sm p-6 mb-4">
                                <div class="w-full mx-auto">
                                    <MagicButton />
                                </div>
                            </div>

                            // User Leaderboard - moved below Magic Button, reduced top/bottom margins
                            <div class="max-w-5xl mx-auto mt-4 mb-4">
                                <UserLeaderboard />
                            </div>
                            
                            // Creature Showcase - reduced spacing & uniform width
                            <div class="mb-4 max-w-5xl mx-auto">
                                <CreatureShowcase />
                            </div>
                            
                            // Global Stats Section - Reduce mb here
                            <div class="mb-8">
                                <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-indigo-600 to-violet-600 mb-4 text-center">
                                    {"Statistics"}
                                </h2>
                                if !(*loading_global_stats) && global_stats.is_some() {
                                    <div class="grid md:grid-cols-3 lg:grid-cols-4 gap-8 max-w-5xl mx-auto mt-4">
                                        {stats_card("Total Scrolls", (*global_stats).clone().unwrap().scrolls_count.to_string(), "📜", "feature-card-blue")}
                                        {stats_card("Total Eggs", (*global_stats).clone().unwrap().eggs_count.to_string(), "🥚", "feature-card-indigo")}
                                        {stats_card("Total Creatures", (*global_stats).clone().unwrap().creatures_count.to_string(), "🐉", "feature-card-purple")}
                                        {stats_card("Total Soul", (*global_stats).clone().unwrap().total_soul.to_string(), "🔮", "feature-card-pink")}
                                    </div>
                                } else {
                                    <div class="flex justify-center items-center h-24 mt-4">
                                        <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-indigo-500"></div>
                                    </div>
                                }
                            </div>
                            
                            // Platform Features Section Title
                            <div class="text-center">
                                <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-indigo-600 to-violet-600 mb-4">
                                    {"Features"}
                                </h2>
                                // Enhanced feature cards with better 3D effects
                                <div class="grid md:grid-cols-2 lg:grid-cols-4 gap-8 max-w-5xl mx-auto mt-4 mb-8">
                                    {feature_card_enhanced("Earn Pax", "Play games to earn pax and summon eggs", "💎", "feature-card-blue")}
                                    {feature_card_enhanced("Summon Eggs", "Discover unique creatures", "🥚", "feature-card-indigo")}
                                    {feature_card_enhanced("Soul Bind", "Enhance your favorite creatures to earn rewards", "🧬", "feature-card-purple")}
                                    {feature_card_enhanced("Market", "Trade and collect rare creatures with others", "🛒", "feature-card-emerald")}
                                </div>
                            </div>

                            // Member Benefits Section
                            <div class="mb-16">
                                <div class="text-center">
                                    <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-amber-500 to-orange-500 mb-4">
                                        {"Members"}
                                    </h2>
                                    <div class="grid md:grid-cols-2 lg:grid-cols-4 gap-8 max-w-5xl mx-auto mt-4">
                                        {member_benefit_card("Daily Claim", "Claim free rewards each day", "🎁", "benefit-card-amber")}
                                        {member_benefit_card("Daily Wheel", "Spin the wheel to win varying prizes", "🎡", "benefit-card-orange")}
                                        {member_benefit_card("Word Game", "Guess the daily word for a scroll", "📝", "benefit-card-red")}
                                        {member_benefit_card("Renaming", "Customize creature names", "✏️", "benefit-card-pink")}
                                    </div>
                                </div>
                            </div>

                            // Extra padding at the bottom to ensure proper spacing from the footer
                            <div class="pb-28"></div>
                        </div>
                    } else {
                        <div class="relative z-10 container mx-auto px-4 py-12 max-w-6xl mt-4">
                            <div class="space-y-10 text-center">
                                // Hero section with floating elements
                                <div class="relative">
                                    // Main hero content
                                    <div class="relative">
                                        <p class="mb-2 text-lg font-semibold text-purple-700 dark:text-purple-300">
                                            {"Free-to-play creature collector game"}
                                        </p>
                                        <div class="inline-flex items-center p-3 px-5 mb-6 rounded-xl bg-gradient-to-r from-indigo-50/80 to-purple-50/80 dark:from-indigo-900/30 dark:to-purple-900/30 text-indigo-700 dark:text-indigo-300 text-sm font-medium shadow-sm border border-indigo-100/50 dark:border-indigo-700/30 backdrop-blur-sm group hover:shadow-md transition-all duration-300">
                                            <span class="mr-2">{"Explanation Video"}</span>
                                            <a 
                                                href="https://www.youtube.com/watch?v=6D3e3sbGQlw" 
                                                target="_blank" 
                                                rel="noopener noreferrer"
                                                class="ml-2 px-3 py-1 bg-gradient-to-r from-red-500 to-red-600 text-white text-xs font-medium rounded-lg shadow-sm hover:shadow-md transition-all duration-300 hover:scale-105 flex items-center"
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 mr-1" viewBox="0 0 24 24" fill="currentColor">
                                                    <path d="M19.615 3.184c-3.604-.246-11.631-.245-15.23 0-3.897.266-4.356 2.62-4.385 8.816.029 6.185.484 8.549 4.385 8.816 3.6.245 11.626.246 15.23 0 3.897-.266 4.356-2.62 4.385-8.816-.029-6.185-.484-8.549-4.385-8.816zm-10.615 12.816v-8l8 3.993-8 4.007z"/>
                                                </svg>
                                                {"Watch"}
                                            </a>
                                            <a 
                                                href="https://discord.gg/zcskw8zjTq" 
                                                target="_blank" 
                                                rel="noopener noreferrer"
                                                class="flex items-center justify-center w-8 h-8 ml-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 shadow-md hover:shadow-lg transition-all duration-300 hover:scale-110 group"
                                                title="Join our Discord community"
                                            >
                                                <img 
                                                    src="/static/images/discord-icon.png" 
                                                    alt="Discord" 
                                                    class="w-5 h-5 group-hover:scale-110 transition-transform duration-300" 
                                                />
                                            </a>
                                        </div>
                                        
                                        <h1 class="text-8xl font-black bg-clip-text text-transparent bg-gradient-to-br from-indigo-600 via-purple-600 to-violet-600 mb-6 tracking-tighter drop-shadow-sm">
                                            {"FRTL"}
                                        </h1>
                                        
                                        // Change text and apply modern purple gradient styling
                                        <p class="text-3xl font-semibold bg-clip-text text-transparent bg-gradient-to-br from-purple-400 via-violet-500 to-fuchsia-500 max-w-3xl mx-auto leading-relaxed mb-12 drop-shadow-sm">
                                            {"What will you find in the chaos?"}
                                        </p>
                                    </div>
                                </div>

                                // Enhanced CTA section with cleaner design - moved up above stats
                                <div class="flex flex-col items-center space-y-3 mb-4 relative">
                                    <div class="absolute inset-0 bg-gradient-radial from-indigo-400/10 via-purple-400/5 to-transparent"></div>
                                    
                                    <div class="flex flex-col sm:flex-row justify-center items-center space-y-2 sm:space-y-0 sm:space-x-6 relative z-10">
                                        <Link<Route> to={Route::Register}>
                                            <button class="relative px-12 py-5 text-lg bg-gradient-to-r from-indigo-600 to-purple-600 text-white font-bold rounded-xl shadow-lg overflow-hidden transition-all duration-300 hover:scale-110 hover:shadow-[0_0_25px_rgba(99,102,241,0.75)]">
                                                <span class="relative flex items-center">
                                                    {"Play Now"}
                                                    <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 ml-2 transform group-hover:translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                                                    </svg>
                                                </span>
                                            </button>
                                        </Link<Route>>
                                        <Link<Route> to={Route::Login}>
                                            <button class="px-8 py-4 border-2 border-indigo-500 text-indigo-600 dark:text-indigo-400 font-bold rounded-xl hover:bg-indigo-50 dark:hover:bg-indigo-900/30 transition-all duration-300 shadow-sm hover:shadow-md backdrop-blur-sm flex items-center gap-2">
                                                {"Sign In"}
                                            </button>
                                        </Link<Route>>
                                    </div>
                                </div>

                                // Creature Showcase - reduced spacing & uniform width
                                <div class="mb-4 max-w-5xl mx-auto">
                                    <CreatureShowcase />
                                </div>

                                // Global Stats Section - Reduce mb here
                                <div class="mb-8">
                                    <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-indigo-600 to-violet-600 mb-4 text-center">
                                        {"Statistics"}
                                    </h2>
                                    if !(*loading_global_stats) && global_stats.is_some() {
                                        <div class="grid md:grid-cols-3 lg:grid-cols-4 gap-8 max-w-5xl mx-auto mt-4">
                                            {stats_card("Total Scrolls", (*global_stats).clone().unwrap().scrolls_count.to_string(), "📜", "feature-card-blue")}
                                            {stats_card("Total Eggs", (*global_stats).clone().unwrap().eggs_count.to_string(), "🥚", "feature-card-indigo")}
                                            {stats_card("Total Creatures", (*global_stats).clone().unwrap().creatures_count.to_string(), "🐉", "feature-card-purple")}
                                            {stats_card("Total Soul", (*global_stats).clone().unwrap().total_soul.to_string(), "🔮", "feature-card-pink")}
                                        </div>
                                    } else {
                                        <div class="flex justify-center items-center h-24 mt-4">
                                            <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-indigo-500"></div>
                                        </div>
                                    }
                                </div>

                                // Platform Features Section Title
                                <div class="text-center">
                                    <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-indigo-600 to-violet-600 mb-4">
                                        {"Features"}
                                    </h2>
                                    // Enhanced feature cards with better 3D effects - Reduce mb here
                                    <div class="grid md:grid-cols-2 lg:grid-cols-4 gap-8 max-w-5xl mx-auto mt-4 mb-8">
                                        {feature_card_enhanced("Earn Pax", "Play games to earn pax and summon eggs", "💎", "feature-card-blue")}
                                        {feature_card_enhanced("Summon Eggs", "Discover unique creatures with various traits", "🥚", "feature-card-indigo")}
                                        {feature_card_enhanced("Soul Bind", "Enhance your favorite creatures to earn rewards", "🧬", "feature-card-purple")}
                                        {feature_card_enhanced("Market", "Trade and collect rare creatures with other players", "🛒", "feature-card-emerald")}
                                    </div>
                                </div>

                                // Member Benefits Section - Reduce mb here
                                <div class="mb-10">
                                    <div class="text-center">
                                        <h2 class="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-amber-500 to-orange-500 mb-4">
                                            {"Members"}
                                        </h2>
                                        <div class="grid md:grid-cols-2 lg:grid-cols-4 gap-8 max-w-5xl mx-auto mt-4">
                                            {member_benefit_card("Daily Claim", "Claim free rewards each day", "🎁", "benefit-card-amber")}
                                            {member_benefit_card("Daily Wheel", "Spin the wheel to win varying prizes", "🎡", "benefit-card-orange")}
                                            {member_benefit_card("Word Game", "Guess the daily word for a scroll and pax", "📝", "benefit-card-red")}
                                            {member_benefit_card("Renaming", "Customize creature names", "✏️", "benefit-card-pink")}
                                        </div>
                                    </div>
                                </div>

                                // Extra padding at the bottom to ensure proper spacing from the footer
                                <div class="pb-28"></div>
                            </div>
                        </div>
                    }
                </div>
            </GradientBackground>
        </Base>
    }
}

// Enhanced feature cards with advanced glassmorphism and micro-interactions
fn feature_card_enhanced(title: &str, description: &str, emoji: &str, card_style: &str) -> Html {
    // Define card-specific colors based on the card_style
    let (bg_gradient, icon_bg, text_gradient, hover_glow, accent_curve) = match card_style {
        "feature-card-indigo" => (
            "bg-gradient-to-br from-indigo-50/90 to-indigo-100/80 dark:from-indigo-900/40 dark:to-indigo-800/50", 
            "bg-indigo-500/20 dark:bg-indigo-500/40", 
            "from-indigo-600 to-indigo-800 dark:from-indigo-400 dark:to-indigo-300",
            "hover:shadow-indigo-200/50 dark:hover:shadow-indigo-500/30",
            "stroke-indigo-500/50 dark:stroke-indigo-400/30"
        ),
        "feature-card-blue" => (
            "bg-gradient-to-br from-blue-50/90 to-blue-100/80 dark:from-blue-900/40 dark:to-blue-800/50", 
            "bg-blue-500/20 dark:bg-blue-500/40", 
            "from-blue-600 to-blue-800 dark:from-blue-400 dark:to-blue-300",
            "hover:shadow-blue-200/50 dark:hover:shadow-blue-500/30",
            "stroke-blue-500/50 dark:stroke-blue-400/30"
        ),
        "feature-card-purple" => (
            "bg-gradient-to-br from-purple-50/90 to-purple-100/80 dark:from-purple-900/40 dark:to-purple-800/50", 
            "bg-purple-500/20 dark:bg-purple-500/40", 
            "from-purple-600 to-purple-800 dark:from-purple-400 dark:to-purple-300",
            "hover:shadow-purple-200/50 dark:hover:shadow-purple-500/30",
            "stroke-purple-500/50 dark:stroke-purple-400/30"
        ),
        "feature-card-emerald" => (
            "bg-gradient-to-br from-emerald-50/90 to-emerald-100/80 dark:from-emerald-900/40 dark:to-emerald-800/50", 
            "bg-emerald-500/20 dark:bg-emerald-500/40", 
            "from-emerald-600 to-emerald-800 dark:from-emerald-400 dark:to-emerald-300",
            "hover:shadow-emerald-200/50 dark:hover:shadow-emerald-500/30",
            "stroke-emerald-500/50 dark:stroke-emerald-400/30"
        ),
        "feature-card-pink" => (
            "bg-gradient-to-br from-pink-50/90 via-purple-50/90 to-pink-100/80 dark:from-pink-900/40 dark:via-purple-900/40 dark:to-pink-800/50", 
            "bg-gradient-to-r from-pink-500/20 to-purple-500/20 dark:from-pink-500/40 dark:to-purple-500/40", 
            "from-pink-600 via-purple-600 to-fuchsia-700 dark:from-pink-400 dark:via-purple-400 dark:to-fuchsia-400",
            "hover:shadow-pink-200/50 dark:hover:shadow-purple-500/30",
            "stroke-pink-500/50 dark:stroke-purple-400/30"
        ),
        _ => (
            "bg-gradient-to-br from-gray-50/90 to-gray-100/80 dark:from-gray-900/40 dark:to-gray-800/50", 
            "bg-gray-500/20 dark:bg-gray-500/40", 
            "from-gray-600 to-gray-800 dark:from-gray-400 dark:to-gray-300",
            "hover:shadow-gray-200/50 dark:hover:shadow-gray-500/30",
            "stroke-gray-500/50 dark:stroke-gray-400/30"
        ),
    };

    html! {
        <div class={format!("h-full rounded-2xl p-6 shadow-lg hover:shadow-xl border border-white/40 dark:border-gray-700/40 transition-all duration-500 backdrop-blur-sm {} {} relative overflow-hidden group", bg_gradient, hover_glow)}>
            // Decorative curve accent
            <svg class="absolute -bottom-1 -right-1 w-32 h-32 opacity-30 group-hover:opacity-70 transition-opacity duration-500" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                <path d="M100,0 Q50,0 30,30 T0,100" fill="none" class={accent_curve} stroke-width="2" />
            </svg>
            
            <div class="flex flex-col space-y-4">
                // Centered emoji at the top
                <div class="flex justify-center items-center">
                    <div class={format!("text-5xl rounded-2xl p-4 shadow-md transition-all duration-500 w-20 h-20 flex items-center justify-center {}", icon_bg)}>
                        {emoji}
                    </div>
                </div>
                <div class="text-center">
                    <h4 class={format!("text-xl font-bold mb-3 bg-clip-text text-transparent bg-gradient-to-r {}", text_gradient)}>
                        {title}
                    </h4>
                    <p class="text-sm text-gray-700 dark:text-gray-300 relative z-10">{description}</p>
                </div>
            </div>
        </div>
    }
}

fn member_benefit_card(title: &str, description: &str, emoji: &str, card_style: &str) -> Html {
    let (bg_gradient, icon_bg, text_gradient, hover_glow, accent_curve) = match card_style {
        "benefit-card-amber" => (
            "bg-gradient-to-br from-amber-50/90 to-amber-100/80 dark:from-amber-900/40 dark:to-amber-800/50", 
            "bg-amber-500/20 dark:bg-amber-500/40", 
            "from-amber-600 to-amber-800 dark:from-amber-400 dark:to-amber-300",
            "hover:shadow-amber-200/50 dark:hover:shadow-amber-500/30",
            "stroke-amber-500/50 dark:stroke-amber-400/30"
        ),
        "benefit-card-orange" => (
            "bg-gradient-to-br from-orange-50/90 to-orange-100/80 dark:from-orange-900/40 dark:to-orange-800/50", 
            "bg-orange-500/20 dark:bg-orange-500/40", 
            "from-orange-600 to-orange-800 dark:from-orange-400 dark:to-orange-300",
            "hover:shadow-orange-200/50 dark:hover:shadow-orange-500/30",
            "stroke-orange-500/50 dark:stroke-orange-400/30"
        ),
        "benefit-card-red" => (
            "bg-gradient-to-br from-red-50/90 to-red-100/80 dark:from-red-900/40 dark:to-red-800/50", 
            "bg-red-500/20 dark:bg-red-500/40", 
            "from-red-600 to-red-800 dark:from-red-400 dark:to-red-300",
            "hover:shadow-red-200/50 dark:hover:shadow-red-500/30",
            "stroke-red-500/50 dark:stroke-red-400/30"
        ),
        "benefit-card-pink" => (
            "bg-gradient-to-br from-pink-50/90 to-pink-100/80 dark:from-pink-900/40 dark:to-pink-800/50", 
            "bg-pink-500/20 dark:bg-pink-500/40", 
            "from-pink-600 to-pink-800 dark:from-pink-400 dark:to-pink-300",
            "hover:shadow-pink-200/50 dark:hover:shadow-pink-500/30",
            "stroke-pink-500/50 dark:stroke-pink-400/30"
        ),
        _ => (
            "bg-gradient-to-br from-gray-50/90 to-gray-100/80 dark:from-gray-900/40 dark:to-gray-800/50", 
            "bg-gray-500/20 dark:bg-gray-500/40", 
            "from-gray-600 to-gray-800 dark:from-gray-400 dark:to-gray-300",
            "hover:shadow-gray-200/50 dark:hover:shadow-gray-500/30",
            "stroke-gray-500/50 dark:stroke-gray-400/30"
        ),
    };

    html! {
        <div class={format!("h-full rounded-2xl p-6 shadow-lg hover:shadow-xl border border-white/40 dark:border-gray-700/40 transition-all duration-500 backdrop-blur-sm {} {} relative overflow-hidden group", bg_gradient, hover_glow)}>
            // Decorative curve accent
            <svg class="absolute -bottom-1 -right-1 w-32 h-32 opacity-30 group-hover:opacity-70 transition-opacity duration-500" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                <path d="M100,0 Q50,0 30,30 T0,100" fill="none" class={accent_curve} stroke-width="2" />
            </svg>
            
            <div class="flex flex-col space-y-4">
                // Centered emoji at the top
                <div class="flex justify-center items-center">
                    <div class={format!("text-5xl rounded-2xl p-4 shadow-md transition-all duration-500 w-20 h-20 flex items-center justify-center {}", icon_bg)}>
                        {emoji}
                    </div>
                </div>
                <div class="text-center">
                    <h4 class={format!("text-xl font-bold mb-3 bg-clip-text text-transparent bg-gradient-to-r {}", text_gradient)}>
                        {title}
                    </h4>
                    <p class="text-sm text-gray-700 dark:text-gray-300 relative z-10">{description}</p>
                </div>
            </div>
        </div>
    }
}

// Stats card for displaying global statistics
fn stats_card(title: &str, value: String, emoji: &str, card_style: &str) -> Html {
    // Define card-specific colors based on the card_style
    let (bg_gradient, icon_bg, text_gradient, hover_glow, accent_curve) = match card_style {
        "feature-card-indigo" => (
            "bg-gradient-to-br from-indigo-50/90 to-indigo-100/80 dark:from-indigo-900/40 dark:to-indigo-800/50", 
            "bg-indigo-500/20 dark:bg-indigo-500/40", 
            "from-indigo-600 to-indigo-800 dark:from-indigo-400 dark:to-indigo-300",
            "hover:shadow-indigo-200/50 dark:hover:shadow-indigo-500/30",
            "stroke-indigo-500/50 dark:stroke-indigo-400/30"
        ),
        "feature-card-blue" => (
            "bg-gradient-to-br from-blue-50/90 to-blue-100/80 dark:from-blue-900/40 dark:to-blue-800/50", 
            "bg-blue-500/20 dark:bg-blue-500/40", 
            "from-blue-600 to-blue-800 dark:from-blue-400 dark:to-blue-300",
            "hover:shadow-blue-200/50 dark:hover:shadow-blue-500/30",
            "stroke-blue-500/50 dark:stroke-blue-400/30"
        ),
        "feature-card-purple" => (
            "bg-gradient-to-br from-purple-50/90 to-purple-100/80 dark:from-purple-900/40 dark:to-purple-800/50", 
            "bg-purple-500/20 dark:bg-purple-500/40", 
            "from-purple-600 to-purple-800 dark:from-purple-400 dark:to-purple-300",
            "hover:shadow-purple-200/50 dark:hover:shadow-purple-500/30",
            "stroke-purple-500/50 dark:stroke-purple-400/30"
        ),
        "feature-card-emerald" => (
            "bg-gradient-to-br from-emerald-50/90 to-emerald-100/80 dark:from-emerald-900/40 dark:to-emerald-800/50", 
            "bg-emerald-500/20 dark:bg-emerald-500/40", 
            "from-emerald-600 to-emerald-800 dark:from-emerald-400 dark:to-emerald-300",
            "hover:shadow-emerald-200/50 dark:hover:shadow-emerald-500/30",
            "stroke-emerald-500/50 dark:stroke-emerald-400/30"
        ),
        "feature-card-pink" => (
            "bg-gradient-to-br from-pink-50/90 via-purple-50/90 to-pink-100/80 dark:from-pink-900/40 dark:via-purple-900/40 dark:to-pink-800/50", 
            "bg-gradient-to-r from-pink-500/20 to-purple-500/20 dark:from-pink-500/40 dark:to-purple-500/40", 
            "from-pink-600 via-purple-600 to-fuchsia-700 dark:from-pink-400 dark:via-purple-400 dark:to-fuchsia-400",
            "hover:shadow-pink-200/50 dark:hover:shadow-purple-500/30",
            "stroke-pink-500/50 dark:stroke-purple-400/30"
        ),
        _ => (
            "bg-gradient-to-br from-gray-50/90 to-gray-100/80 dark:from-gray-900/40 dark:to-gray-800/50", 
            "bg-gray-500/20 dark:bg-gray-500/40", 
            "from-gray-600 to-gray-800 dark:from-gray-400 dark:to-gray-300",
            "hover:shadow-gray-200/50 dark:hover:shadow-gray-500/30",
            "stroke-gray-500/50 dark:stroke-gray-400/30"
        ),
    };

    html! {
        <div class={format!("h-full rounded-2xl p-6 shadow-lg hover:shadow-xl border border-white/40 dark:border-gray-700/40 transition-all duration-500 backdrop-blur-sm {} {} relative overflow-hidden group", bg_gradient, hover_glow)}>
            // Decorative curve accent
            <svg class="absolute -bottom-1 -right-1 w-32 h-32 opacity-30 group-hover:opacity-70 transition-opacity duration-500" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                <path d="M100,0 Q50,0 30,30 T0,100" fill="none" class={accent_curve} stroke-width="2" />
            </svg>
            
            <div class="flex flex-col space-y-4">
                // Centered emoji at the top
                <div class="flex justify-center items-center">
                    <div class={format!("text-5xl rounded-2xl p-4 shadow-md transition-all duration-500 w-20 h-20 flex items-center justify-center {}", icon_bg)}>
                        {emoji}
                    </div>
                </div>
                <div class="text-center">
                    <h4 class={format!("text-xl font-bold mb-3 bg-clip-text text-transparent bg-gradient-to-r {}", text_gradient)}>
                        {title}
                    </h4>
                    <p class={format!("text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r {} relative z-10", text_gradient)}>{value}</p>
                </div>
            </div>
        </div>
    }
}
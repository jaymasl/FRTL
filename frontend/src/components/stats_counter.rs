use yew::prelude::*;
use crate::models::GlobalStats;

#[derive(Properties, PartialEq)]
pub struct StatsCounterProps {
    pub stats: GlobalStats,
    #[prop_or(false)]
    pub is_personal: bool,
    #[prop_or(false)]
    pub vertical: bool,
}

#[function_component(StatsCounter)]
pub fn stats_counter(props: &StatsCounterProps) -> Html {
    let stats = &props.stats;

    html! {
        <div class={if props.vertical {
            "flex flex-col space-y-4 w-full max-w-md mx-auto"
        } else {
            "grid grid-cols-1 md:grid-cols-3 gap-4 w-full max-w-4xl mx-auto"
        }}>
            // Scrolls Counter
            <div class="rounded-2xl p-6 shadow-lg hover:shadow-xl border border-white/40 dark:border-gray-700/40 transition-all duration-500 backdrop-blur-sm bg-gradient-to-br from-blue-50/90 to-blue-100/80 dark:from-blue-900/40 dark:to-blue-800/50 hover:shadow-blue-200/50 dark:hover:shadow-blue-500/30 relative overflow-hidden group">
                // Decorative curve accent
                <svg class="absolute -bottom-1 -right-1 w-32 h-32 opacity-30 group-hover:opacity-70 transition-opacity duration-500" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                    <path d="M100,0 Q50,0 30,30 T0,100" fill="none" class="stroke-blue-500/50 dark:stroke-blue-400/30" stroke-width="2" />
                </svg>
                
                <div class="flex flex-col space-y-4">
                    // Centered emoji
                    <div class="flex justify-center items-center">
                        <div class="text-5xl rounded-2xl p-4 shadow-md transition-all duration-500 w-20 h-20 flex items-center justify-center bg-blue-500/20 dark:bg-blue-500/40">
                            {"📜"}
                        </div>
                    </div>
                    <div class="text-center">
                        <h4 class="text-xl font-bold mb-3 bg-clip-text text-transparent bg-gradient-to-r from-blue-600 to-blue-800 dark:from-blue-400 dark:to-blue-300">
                            {if props.is_personal { "Your Scrolls" } else { "Total Scrolls" }}
                        </h4>
                        <p class="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-blue-600 to-blue-800 dark:from-blue-400 dark:to-blue-300 relative z-10">
                            {stats.scrolls_count}
                        </p>
                    </div>
                </div>
            </div>

            // Eggs Counter
            <div class="rounded-2xl p-6 shadow-lg hover:shadow-xl border border-white/40 dark:border-gray-700/40 transition-all duration-500 backdrop-blur-sm bg-gradient-to-br from-amber-50/90 to-amber-100/80 dark:from-amber-900/40 dark:to-amber-800/50 hover:shadow-amber-200/50 dark:hover:shadow-amber-500/30 relative overflow-hidden group">
                // Decorative curve accent
                <svg class="absolute -bottom-1 -right-1 w-32 h-32 opacity-30 group-hover:opacity-70 transition-opacity duration-500" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                    <path d="M100,0 Q50,0 30,30 T0,100" fill="none" class="stroke-amber-500/50 dark:stroke-amber-400/30" stroke-width="2" />
                </svg>
                
                <div class="flex flex-col space-y-4">
                    // Centered emoji
                    <div class="flex justify-center items-center">
                        <div class="text-5xl rounded-2xl p-4 shadow-md transition-all duration-500 w-20 h-20 flex items-center justify-center bg-amber-500/20 dark:bg-amber-500/40">
                            {"🥚"}
                        </div>
                    </div>
                    <div class="text-center">
                        <h4 class="text-xl font-bold mb-3 bg-clip-text text-transparent bg-gradient-to-r from-amber-600 to-amber-800 dark:from-amber-400 dark:to-amber-300">
                            {if props.is_personal { "Your Eggs" } else { "Total Eggs" }}
                        </h4>
                        <p class="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-amber-600 to-amber-800 dark:from-amber-400 dark:to-amber-300 relative z-10">
                            {stats.eggs_count}
                        </p>
                    </div>
                </div>
            </div>

            // Creatures Counter
            <div class="rounded-2xl p-6 shadow-lg hover:shadow-xl border border-white/40 dark:border-gray-700/40 transition-all duration-500 backdrop-blur-sm bg-gradient-to-br from-purple-50/90 to-purple-100/80 dark:from-purple-900/40 dark:to-purple-800/50 hover:shadow-purple-200/50 dark:hover:shadow-purple-500/30 relative overflow-hidden group">
                // Decorative curve accent
                <svg class="absolute -bottom-1 -right-1 w-32 h-32 opacity-30 group-hover:opacity-70 transition-opacity duration-500" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                    <path d="M100,0 Q50,0 30,30 T0,100" fill="none" class="stroke-purple-500/50 dark:stroke-purple-400/30" stroke-width="2" />
                </svg>
                
                <div class="flex flex-col space-y-4">
                    // Centered emoji
                    <div class="flex justify-center items-center">
                        <div class="text-5xl rounded-2xl p-4 shadow-md transition-all duration-500 w-20 h-20 flex items-center justify-center bg-purple-500/20 dark:bg-purple-500/40">
                            {"🐉"}
                        </div>
                    </div>
                    <div class="text-center">
                        <h4 class="text-xl font-bold mb-3 bg-clip-text text-transparent bg-gradient-to-r from-purple-600 to-purple-800 dark:from-purple-400 dark:to-purple-300">
                            {if props.is_personal { "Your Creatures" } else { "Total Creatures" }}
                        </h4>
                        <p class="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-purple-600 to-purple-800 dark:from-purple-400 dark:to-purple-300 relative z-10">
                            {stats.creatures_count}
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
} 
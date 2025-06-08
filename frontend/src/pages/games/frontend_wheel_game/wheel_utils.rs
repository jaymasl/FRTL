use yew::prelude::*;
use web_sys::window;
use shared::shared_wheel_game::*;

// Format time for cooldown display
pub fn format_time(seconds: i32) -> String {
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

// Get auth token from storage
pub fn get_auth_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| window()
            .and_then(|w| w.session_storage().ok().flatten())
            .and_then(|s| s.get_item("token").ok().flatten()))
}

// Result display component
#[derive(Properties, PartialEq)]
pub struct ResultDisplayProps {
    pub reward_type: Option<RewardType>,
    pub show_result: bool,
    pub result_number: Option<f64>,
}

#[function_component(ResultDisplay)]
pub fn result_display(props: &ResultDisplayProps) -> Html {
    if !props.show_result {
        return html! {};
    }

    if let Some(reward_type) = &props.reward_type {
        let (message, gradient_classes, animation_class) = match reward_type {
            RewardType::Scroll => (
                "You won a scroll!", 
                "from-orange-400 to-orange-600 border-orange-300",
                "animate-bounce"
            ),
            RewardType::BigPax => (
                "You won 50 pax!", 
                "from-blue-400 to-blue-600 border-blue-300",
                "animate-pulse"
            ),
            RewardType::SmallPax => (
                "You won 20 pax!", 
                "from-violet-400 to-violet-600 border-violet-300",
                "animate-pulse"
            ),
            RewardType::TinyPax => (
                "You won 10 pax!", 
                "from-pink-400 to-pink-600 border-pink-300",
                "animate-pulse"
            ),
        };

        return html! {
            <div class="mt-8 mb-4 flex flex-col items-center justify-center">
                <div class={classes!(
                    "flex", 
                    "items-center", 
                    "justify-center", 
                    "px-6", 
                    "py-4", 
                    "rounded-xl", 
                    "bg-gradient-to-r", 
                    "text-white", 
                    "font-bold", 
                    "text-xl",
                    "shadow-lg",
                    "border-2",
                    "transform",
                    "transition-all",
                    "duration-500",
                    animation_class,
                    gradient_classes
                )}>
                    <span>{message}</span>
                </div>
                {
                    if let Some(number) = props.result_number {
                        html! {
                            <div class="text-sm text-gray-600 dark:text-gray-400 mt-3 bg-gray-100 dark:bg-gray-800 px-4 py-2 rounded-full">
                                {format!("Result: {:.2}", number)}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        };
    }

    html! {}
}

// Spin button component
#[derive(Properties, PartialEq)]
pub struct SpinButtonProps {
    pub is_spinning: bool,
    pub is_on_cooldown: bool,
    pub cooldown_seconds: i32,
    pub has_enough_balance: bool,
    pub onclick: Callback<MouseEvent>,
}

#[function_component(SpinButton)]
pub fn spin_button(props: &SpinButtonProps) -> Html {
    let button_text = if props.is_spinning {
        "Spinning...".to_string()
    } else if props.is_on_cooldown {
        format!("Cooldown: {}", format_time(props.cooldown_seconds))
    } else {
        "Spin (Free)".to_string()
    };

    let is_disabled = props.is_spinning || props.is_on_cooldown;
    
    // Enhanced button styling with gradient and animation effects
    let button_class = if is_disabled {
        if props.is_on_cooldown {
            // Cooldown state - blue/gray gradient
            "bg-gradient-to-r from-blue-400 to-gray-400 opacity-80 cursor-not-allowed text-white"
        } else {
            // Other disabled states - gray gradient
            "bg-gradient-to-r from-gray-400 to-gray-500 opacity-75 cursor-not-allowed text-white"
        }
    } else {
        // Active state - vibrant gold/orange gradient
        "bg-gradient-to-r from-yellow-400 to-orange-500 hover:from-yellow-500 hover:to-orange-600 text-white shadow-lg hover:shadow-xl transform hover:-translate-y-0.5 active:translate-y-0"
    };

    // Add a pulsing animation when button is active
    let animation_class = if !is_disabled && !props.is_spinning {
        "animate-pulse-subtle"
    } else {
        ""
    };

    // Add a spinning animation when spinning
    let spin_icon_class = if props.is_spinning {
        "inline-block mr-2 animate-spin"
    } else {
        "hidden"
    };

    html! {
        <div class="relative">
            // Wrapper div to ensure gradient extends to edges properly
            <div class={classes!(
                "relative",
                "overflow-hidden",
                "rounded-full",
                "w-full",
                button_class,
                animation_class
            )}>
                <button
                    onclick={props.onclick.clone()}
                    disabled={is_disabled}
                    class={classes!(
                        "relative",
                        "w-full",
                        "px-8",
                        "py-4",
                        "font-bold",
                        "text-lg",
                        "transition-all",
                        "duration-300",
                        "border-2",
                        "border-transparent",
                        "hover:border-white",
                        "focus:outline-none",
                        "focus:ring-4",
                        "focus:ring-yellow-300",
                        "focus:ring-opacity-50",
                        "bg-transparent",
                    )}
                >
                    <div class="flex items-center justify-center relative z-10">
                        <svg class={spin_icon_class} xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <circle cx="12" cy="12" r="10" />
                            <path d="M12 6v6l4 2" />
                        </svg>
                        <span>{button_text}</span>
                    </div>
                </button>
            </div>
            
            // Add a subtle glow effect behind the button
            <div class={classes!(
                "absolute",
                "inset-0",
                "rounded-full",
                "filter",
                "blur-md",
                "opacity-30",
                "bg-yellow-400",
                "pointer-events-none",
                "transition-opacity",
                "duration-300",
                if is_disabled { "opacity-0" } else { "opacity-30" }
            )}></div>
        </div>
    }
} 
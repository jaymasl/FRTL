use yew::prelude::*;
use web_sys::{MouseEvent, Element};
use super::DisplayItem;
use crate::config::get_api_base_url;

#[derive(Properties, PartialEq)]
pub struct StatsBarProps {
    pub label: String,
    pub value: f64,
    pub max_value: f64,
    pub color_class: String,
    pub show_percentage: bool,
}

#[function_component(StatsBar)]
pub fn stats_bar(props: &StatsBarProps) -> Html {
    html! {
        <div class="space-y-1.5 h-[30px]">
            <div class="flex justify-between text-xs">
                <span class="font-medium text-gray-700 dark:text-gray-200">
                    {&props.label}
                </span>
                {if props.show_percentage {
                    html! {
                        <span class="font-medium text-gray-600 dark:text-gray-300">
                            {format!("{}%", ((props.value / props.max_value) * 100.0) as i32)}
                        </span>
                    }
                } else {
                    html! {}
                }}
            </div>
            <div class="h-2 relative rounded-full overflow-hidden bg-gray-100 dark:bg-gray-800 backdrop-blur-sm">
                <div 
                    class={classes!(
                        "absolute", "inset-y-0", "left-0", "transition-all", "duration-500",
                        &props.color_class
                    )}
                    style={format!("width: {}%", ((props.value / props.max_value) * 100.0) as i32)}
                />
                {if props.color_class.contains("chaos-pulse") {
                    html! {
                        <>
                            // Base glow effect inside the bar
                            <div class="absolute inset-0 bg-gradient-to-r from-purple-500/20 to-fuchsia-600/20 animate-pulse" />
                            
                            // Random pulse effects with varied positions and timings
                            <div class="absolute inset-0 overflow-hidden">
                                // Horizontal pulses
                                <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-purple-500/50 to-transparent left-[7%] animate-chaos-pulse-1" />
                                <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-fuchsia-500/50 to-transparent left-[28%] animate-chaos-pulse-2" />
                                <div class="absolute h-full w-20 bg-gradient-to-r from-transparent via-purple-400/50 to-transparent left-[46%] animate-chaos-pulse-3" />
                                <div class="absolute h-full w-12 bg-gradient-to-r from-transparent via-fuchsia-400/50 to-transparent left-[67%] animate-chaos-pulse-4" />
                                <div class="absolute h-full w-16 bg-gradient-to-r from-transparent via-purple-500/50 to-transparent left-[88%] animate-chaos-pulse-5" />
                            </div>
                        </>
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}

pub fn get_image_url(item: &DisplayItem) -> Option<String> {
    match item {
        DisplayItem::Creature(c) => c.image_path.clone(),
        DisplayItem::Egg(e) => e.image_path.clone(),
        DisplayItem::Scroll(s) => s.image_path.clone()
            .or_else(|| Some("/static/images/scroll-default.avif".to_string())),
    }.map(|p| if p.starts_with("http") { p } else { format!("{}{}", get_api_base_url(), p) })
}

#[derive(Properties, PartialEq)]
pub struct CardBaseProps {
    pub item: DisplayItem,
    pub title: Html,
    pub stats: Vec<(String, f64, f64, String)>,
    pub action_label: Option<String>,
    pub on_action: Option<Callback<()>>,
    pub on_click: Option<Callback<DisplayItem>>,
    pub loading: bool,
    #[prop_or_default]
    pub error: String,
    #[prop_or_default]
    pub is_compact: bool,
    #[prop_or_default]
    pub overlay_tag: Option<Html>,
}

#[function_component(CardBase)]
pub fn card_base(props: &CardBaseProps) -> Html {
    let card_ref = use_node_ref();
    let rotation_x = use_state(|| 0.0);
    let rotation_y = use_state(|| 0.0);
    let is_hovering = use_state(|| false);

    let handle_mouse_move = {
        let card_ref = card_ref.clone();
        let rotation_x = rotation_x.clone();
        let rotation_y = rotation_y.clone();
        let is_hovering = is_hovering.clone();
        
        Callback::from(move |e: MouseEvent| {
            if *is_hovering {
                if let Some(element) = card_ref.cast::<Element>() {
                    let rect = element.get_bounding_client_rect();
                    let x = e.client_x() as f64 - rect.left();
                    let y = e.client_y() as f64 - rect.top();
                    
                    let rx = ((y / rect.height() - 0.5) * -16.0).clamp(-8.0, 8.0);
                    let ry = ((x / rect.width() - 0.5) * 16.0).clamp(-8.0, 8.0);
                    
                    rotation_x.set(rx);
                    rotation_y.set(ry);
                }
            }
        })
    };

    let handle_mouse_enter = {
        let is_hovering = is_hovering.clone();
        Callback::from(move |_| {
            is_hovering.set(true);
        })
    };

    let handle_mouse_leave = {
        let rotation_x = rotation_x.clone();
        let rotation_y = rotation_y.clone();
        let is_hovering = is_hovering.clone();
        
        Callback::from(move |_| {
            rotation_x.set(0.0);
            rotation_y.set(0.0);
            is_hovering.set(false);
        })
    };

    let handle_click = {
        let item = props.item.clone();
        let on_click = props.on_click.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target_unchecked_into::<web_sys::Element>();
            if !target.closest(".card-action-button").map_or(false, |el| el.is_some()) {
                if let Some(callback) = on_click.clone() {
                    callback.emit(item.clone());
                }
            }
        })
    };

    let handle_action = if let Some(action) = &props.on_action {
        let action = action.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            action.emit(())
        })
    } else {
        Callback::from(|_| ())
    };

    let transform_style = format!(
        "transform: perspective(1000px) rotateX({}deg) rotateY({}deg);",
        *rotation_x, *rotation_y
    );

    html! {
        <div 
            ref={card_ref}
            onmouseenter={handle_mouse_enter}
            onmousemove={handle_mouse_move}
            onmouseleave={handle_mouse_leave}
            class="relative group/card"
        >
            <div 
                onclick={handle_click}
                class="group/card transform-gpu transition-transform duration-300 ease-out preserve-3d"
                style={transform_style}
            >
                <div 
                    class="absolute rounded-2xl transition-opacity duration-300 opacity-0 group-hover/card:opacity-100 bg-transparent"
                    style="transform: translateZ(-1px)"
                />
                <div 
                    class="pointer-events-none absolute inset-0 rounded-xl opacity-0 transition-all duration-300 group-hover/card:opacity-100 group-hover/card:shadow-[0_0_20px_2px_rgba(139,92,246,0.2),0_0_30px_4px_rgba(236,72,153,0.2)]"
                />
                <div 
                    class={classes!(
                        "relative",
                        "bg-white/90",
                        "dark:bg-gray-950/90",
                        "backdrop-blur-xl",
                        "rounded-xl",
                        if props.is_compact { "p-3" } else { "px-6 pt-6 pb-4" },
                        "flex",
                        "flex-col",
                        "border",
                        "border-gray-100/20",
                        "dark:border-gray-800/20",
                        "shadow-[0_8px_32px_-8px_rgba(0,0,0,0.1)]",
                        "dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)]",
                        "transition-all",
                        "duration-300"
                    )}
                >
                    <div class={classes!(
                        "relative",
                        "w-[calc(100%+10px)]",
                        "h-[calc(100%+10px)]",
                        "-mx-[5px]",
                        "-my-[5px]",
                        if props.is_compact { "mb-2" } else { "mb-3" },
                        "overflow-hidden",
                        "transform",
                        "transition-all",
                        "duration-300",
                        "rounded-lg"
                    )} style="transform: translateZ(20px)">
                        {if let Some(tag) = &props.overlay_tag {
                            html! { <div class="absolute top-2 left-2 z-10">{tag.clone()}</div> }
                        } else {
                            html! {}
                        }}
                        <div class="absolute inset-0 bg-gradient-to-br from-violet-500/10 via-fuchsia-500/10 to-pink-500/10 opacity-0 group-hover/card:opacity-50 transition-opacity duration-500" />
                        {match get_image_url(&props.item) {
                            Some(url) => html! {
                                <img
                                    src={url}
                                    class="w-full h-full object-cover rounded-lg transform select-none"
                                    alt="Item"
                                    draggable="false"
                                    onmousedown={Callback::from(|e: MouseEvent| e.prevent_default())}
                                />
                            },
                            None => html! {
                                <div class="w-full h-full flex items-center justify-center rounded-lg bg-gradient-to-br from-gray-50 to-gray-100 dark:from-gray-800 dark:to-gray-900">
                                    <span class="text-gray-400 dark:text-gray-500">{"No image"}</span>
                                </div>
                            }
                        }}
                    </div>

                    <h3 
                        class={classes!(
                            "text-sm",
                            "font-semibold",
                            "text-gray-900",
                            "dark:text-gray-100",
                            if props.is_compact { "mb-1" } else { "mb-2" }
                        )}
                        style="transform: translateZ(15px)"
                    >
                        {props.title.clone()}
                    </h3>

                    <div 
                        class={classes!(
                            "space-y-3",
                            if !props.stats.is_empty() { "flex-grow min-h-[40px]" } else { "" }
                        )}
                        style="transform: translateZ(10px)"
                    >
                        {for props.stats.iter().map(|(label, value, max, color_class)| {
                            html! {
                                <StatsBar
                                    label={label.clone()}
                                    value={*value}
                                    max_value={*max}
                                    color_class={color_class.clone()}
                                    show_percentage=true
                                />
                            }
                        })}
                    </div>

                    {if !props.error.is_empty() {
                        html! {
                            <div 
                                class="mt-3 text-sm text-red-500/90 dark:text-red-400/90"
                                style="transform: translateZ(10px)"
                            >
                                {&props.error}
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    {if let Some(label) = &props.action_label {
                        html! {
                            <button
                                onclick={handle_action}
                                disabled={props.loading}
                                style="transform: translateZ(25px)"
                                class={classes!(
                                    "card-action-button",
                                    "mt-3",
                                    "px-4",
                                    "py-2",
                                    "rounded-lg",
                                    "text-sm",
                                    "font-medium",
                                    "text-white",
                                    "bg-gradient-to-r",
                                    "from-violet-500",
                                    "via-fuchsia-500",
                                    "to-pink-500",
                                    "hover:from-violet-600",
                                    "hover:via-fuchsia-600",
                                    "hover:to-pink-600",
                                    "disabled:opacity-50",
                                    "disabled:cursor-not-allowed",
                                    "transition-all",
                                    "duration-500",
                                    "hover:scale-[1.02]",
                                    "hover:shadow-[0_10px_40px_-10px_rgba(0,0,0,0.3)]",
                                    "dark:hover:shadow-[0_6px_16px_-6px_rgba(255,255,255,0.06)]"
                                )}
                            >
                                {if props.loading {
                                    "Loading..."
                                } else {
                                    label
                                }}
                            </button>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        </div>
    }
}
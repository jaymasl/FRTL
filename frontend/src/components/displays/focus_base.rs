use yew::prelude::*;
use web_sys::MouseEvent;
use crate::models::Egg;
use super::{DisplayItem, DisplayMode, focus_creature::CreatureFocus, focus_egg::EggFocus, focus_scroll::ScrollFocus};
use uuid::Uuid;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub item: DisplayItem,
    pub mode: DisplayMode,
    pub on_close: Option<Callback<()>>,
    pub on_action: Option<Callback<()>>,
    pub action_label: Option<String>,
    pub loading: bool,
    pub error: String,
    pub fetch_data: Option<Callback<()>>,
    #[prop_or_default]
    pub on_select_egg: Option<Callback<Egg>>,
    #[prop_or_default]
    pub on_energy_update: Option<Callback<(Uuid, bool)>>,
}

#[function_component(FocusTemplate)]
pub fn focus_template(props: &Props) -> Html {
    let handle_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            if let Some(callback) = &on_close {
                callback.emit(());
            }
        })
    };

    let handle_backdrop_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(callback) = &on_close {
                callback.emit(());
            }
        })
    };

    // Determine if we're showing a scroll to adjust the width
    let is_scroll = matches!(&props.item, DisplayItem::Scroll(_));
    let _modal_width_class = if is_scroll { "max-w-3xl" } else { "max-w-5xl" };
    
    html! {
        <div 
            // This outer div is the full screen backdrop
            class="fixed inset-0 z-[1100] bg-white/90 dark:bg-black/90 backdrop-blur-md flex items-center justify-center overflow-hidden" 
            onclick={handle_backdrop_click}
        >
            <style>
                {r#"
                .scroll-focus-wrapper {
                    width: 100% !important;
                    display: flex !important;
                    justify-content: center !important;
                    align-items: center !important;
                }
                .scroll-focus-container {
                    width: 100% !important;
                    max-width: 600px !important;
                    margin: 0 auto !important;
                    display: flex !important;
                    flex-direction: column !important;
                    align-items: center !important;
                }
                .modal-content {
                    width: 100%;
                    overflow-y: auto;
                    padding: 1rem;
                }
                "#}
            </style>
            <div 
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                class={classes!(
                    // This is the actual modal container with properly constrained width
                    "mt-20", // add top margin to avoid navbar
                    "relative",
                    "w-11/12", "md:w-4/5", "lg:w-3/5", "xl:w-1/2", // Responsive width constraints
                    "max-w-4xl", // Maximum width cap
                    "max-h-[calc(80vh-5rem)]", // Height constraint factoring in top margin, reduced further to ensure visibility
                    "overflow-y-auto", // Enable vertical scrolling
                    "rounded-lg", 
                    "bg-white", 
                    "dark:bg-gray-900", 
                    "text-left", 
                    "shadow-xl", 
                    "transition-all",
                    if is_scroll { "scroll-focus-container" } else { "" }
                )}
            >
                <button
                    type="button"
                    onclick={handle_close}
                    class="absolute top-3 right-3 z-10 flex items-center justify-center w-8 h-8 rounded-full bg-red-100 dark:bg-red-900/50 text-red-600 dark:text-red-400 hover:bg-red-200 dark:hover:bg-red-800 hover:text-red-700 dark:hover:text-red-300 focus:outline-none transition-colors"
                >
                    <span class="sr-only">{"Close"}</span>
                    <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>

                <div class="pt-4 px-6 w-full">
                    {
                        if let DisplayItem::Scroll(scroll) = &props.item {
                            html! {
                                <h2 class="text-2xl font-bold text-gray-900 dark:text-white">
                                    {format!("{} ({}x)", &scroll.display_name, scroll.quantity)}
                                </h2>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                <div class="px-6 pb-6 flex-grow">
                    {match &props.item {
                        DisplayItem::Creature(creature) => html! {
                            <CreatureFocus
                                creature={creature.clone()}
                                on_action={props.on_action.clone()}
                                action_label={props.action_label.clone()}
                                loading={props.loading}
                                error={props.error.clone()}
                                mode={props.mode.clone()}
                                fetch_data={props.fetch_data.clone()}
                                on_energy_update={props.on_energy_update.clone()}
                            />
                        },
                        DisplayItem::Egg(egg) => html! {
                            <EggFocus
                                egg={egg.clone()}
                                on_action={props.on_action.clone()}
                                action_label={props.action_label.clone()}
                                loading={props.loading}
                                error={props.error.clone()}
                                mode={props.mode.clone()}
                            />
                        },
                        DisplayItem::Scroll(scroll) => html! {
                            <div class="scroll-focus-wrapper flex justify-center items-center">
                                <ScrollFocus
                                    scroll={scroll.clone()}
                                    on_close={props.on_close.clone().unwrap_or_else(|| Callback::from(|_| ()))}
                                    on_action={props.on_action.clone()}
                                    action_label={props.action_label.clone()}
                                    loading={props.loading}
                                    error={props.error.clone()}
                                    mode={props.mode.clone()}
                                    fetch_data={props.fetch_data.clone()}
                                    on_select_egg={props.on_select_egg.clone()}
                                />
                            </div>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
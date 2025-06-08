use yew::prelude::*;
use web_sys::MouseEvent;
use uuid::Uuid;
use crate::models::Creature;
use crate::pages::inventory::handlers::{
    CreatureResponse, 
    handle_bind_select,
    handle_bind
};
use crate::config::get_api_base_url;

#[derive(Properties, PartialEq)]
pub struct BindModalProps {
    pub target: Creature,
    pub available_creatures: Vec<Creature>,
    pub on_success: Callback<Option<CreatureResponse>>,
    pub on_close: Callback<MouseEvent>,
    pub on_error: Callback<String>,
    pub loading: bool,
    pub fetch_data: Option<Callback<()>>,
}

#[function_component(BindModal)]
pub fn bind_modal(props: &BindModalProps) -> Html {
    let selected = use_state(|| None::<Uuid>);
    let error = use_state(String::new);
    let filtered_creatures = props.available_creatures.iter()
        .filter(|c| c.essence == props.target.essence && c.rarity == props.target.rarity)
        .collect::<Vec<_>>();

    let handle_select = handle_bind_select(selected.clone());

    let handle_bind_action = {
        let fetch_data = props.fetch_data.clone();
        let target = props.target.clone();
        let on_success = props.on_success.clone();
        let on_close = props.on_close.clone();
        let error = error.clone();
        let on_close_for_void = on_close.clone();

        let void_callback = Callback::from(move |_| {
            if let Some(e) = MouseEvent::new("click").ok() {
                on_close_for_void.emit(e);
            }
        });

        let on_error = Callback::from(move |err: String| {
            error.set(err);
        });

        handle_bind(
            selected.clone(),
            target,
            use_state(|| Vec::new()),
            on_success,
            on_error,
            void_callback,
            fetch_data.unwrap_or_else(|| Callback::from(|_| ())),
        )
    };

    let button_state = {
        let error = (*error).clone();
        let selected = *selected;
        let loading = props.loading;

        let is_disabled = selected.is_none() || loading || error.contains("Not enough Pax") || error.contains("Too Many Requests");
        let button_text = if loading {
            "Processing...".to_string()
        } else if error.contains("Not enough Pax") {
            "Not Enough Pax".to_string()
        } else if error.contains("Too Many Requests") {
            "Too Many Requests".to_string()
        } else {
            "Confirm Soul Bind (55 Pax)".to_string()
        };
        let button_class = if is_disabled {
            "bg-gray-600 cursor-not-allowed"
        } else {
            "bg-gradient-to-r from-purple-500 to-pink-600 hover:from-purple-600 hover:to-pink-700"
        };

        (is_disabled, button_text, button_class)
    };

    let onclick = {
        let error = error.clone();
        let handle_bind_action = handle_bind_action.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            error.set(String::new());
            handle_bind_action.emit(());
        })
    };

    html! {
        <div 
            class="fixed inset-0 z-[1100] bg-black/90 backdrop-blur-md overflow-y-auto" 
            onclick={props.on_close.clone()}
        >
            <div class="flex min-h-full items-end justify-center p-4 text-center sm:items-center sm:p-0">
                <div 
                    onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                    class="relative transform overflow-hidden rounded-lg bg-gray-900 text-left shadow-xl transition-all sm:my-8 sm:w-full sm:max-w-3xl"
                >
                    <div class="p-6">
                        <div class="text-center sm:text-left">
                            <h3 class="text-2xl font-semibold text-white mb-2">{"Select Creature to Soul Bind"}</h3>
                            <p class="text-gray-400 mb-4">
                                {format!("Choose another {} {} essence creature to soul bind. This creature will be absorbed.", 
                                    props.target.rarity.clone().unwrap_or_else(|| "Unknown".to_string()),
                                    props.target.essence.clone().unwrap_or_else(|| "Unknown".to_string()))}
                            </p>

                            if !(*error).is_empty() {
                                <div class="mb-4 p-4 bg-red-500/10 border border-red-500/20 rounded-lg">
                                    <p class="text-red-500">{(*error).clone()}</p>
                                </div>
                            }
                            
                            if filtered_creatures.is_empty() {
                                <div class="text-center py-8">
                                    <p class="text-gray-400">{"No eligible creatures available."}</p>
                                </div>
                            } else {
                                <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mt-4">
                                    {for filtered_creatures.iter().map(|creature| {
                                        let is_selected = *selected == Some(creature.id);
                                        let handle_select = handle_select.clone();
                                        let id = creature.id;
                                        let image_path = creature.image_path.clone().unwrap_or_default();
                                        let display_name = creature.display_name.clone().unwrap_or_default();
                                        
                                        html! {
                                            <div 
                                                onclick={move |_| handle_select.emit(id)}
                                                class={classes!(
                                                    "relative", "rounded-lg", "p-4", "cursor-pointer", 
                                                    "transition-all", "duration-300", "border-2",
                                                    if is_selected { 
                                                        "bg-gray-800 border-blue-500"
                                                    } else {
                                                        "bg-gray-800/50 border-gray-700 hover:border-gray-600"
                                                    }
                                                )}
                                            >
                                                <div class="aspect-square rounded-lg mb-2 overflow-hidden bg-gray-800">
                                                    <img 
                                                        src={format!("{}{}", get_api_base_url(), image_path)} 
                                                        class="w-full h-full object-cover select-none"
                                                        alt={display_name.clone()}
                                                        draggable="false"
                                                        onmousedown={Callback::from(|e: MouseEvent| e.prevent_default())}
                                                    />
                                                </div>
                                                <div class="text-sm font-medium text-white">{display_name}</div>
                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        </div>
                    </div>

                    <div class="bg-gray-800 px-4 py-3 sm:flex sm:flex-row-reverse sm:px-6">
                        <button
                            type="button"
                            disabled={button_state.0}
                            {onclick}
                            class={classes!(
                                "inline-flex", "w-full", "justify-center", "rounded-lg",
                                "px-4", "py-2", "text-sm", "font-semibold", "text-white",
                                "sm:ml-3", "sm:w-auto", "transition-all", "duration-300",
                                button_state.2
                            )}
                        >
                            {button_state.1}
                        </button>
                        <button
                            type="button"
                            onclick={props.on_close.clone()}
                            class="mt-3 inline-flex w-full justify-center rounded-lg px-4 py-2 text-sm font-semibold text-gray-300 sm:mt-0 sm:w-auto hover:bg-gray-700 hover:text-white border border-gray-600 transition-all duration-300"
                        >
                            {"Cancel"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
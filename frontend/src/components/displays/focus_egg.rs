use js_sys::Date;
use wasm_bindgen::JsValue;
use yew::prelude::*;
use web_sys::MouseEvent;
use crate::models::Egg;
use crate::styles;
use super::{get_image_url, DisplayItem, DisplayMode, get_egg_details};

#[derive(Properties, PartialEq)]
pub struct EggFocusProps {
    pub egg: Egg,
    pub on_action: Option<Callback<()>>,
    pub action_label: Option<String>,
    pub loading: bool,
    pub error: String,
    pub mode: DisplayMode,
}

#[derive(Clone, PartialEq)]
struct IncubationStatus {
    is_incubating: bool,
    seconds_remaining: i64,
    progress_percent: f64,
}

fn format_time_remaining(seconds: i64) -> String {
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

#[function_component(EggFocus)]
pub fn egg_focus(props: &EggFocusProps) -> Html {
    let incubation_status = {
        let mut status = None;
        if let Some(ends_at) = &props.egg.incubation_ends_at {
            let now = Date::new_0();
            let incubation_ends = if ends_at.contains('T') {
                Date::new(&JsValue::from_str(ends_at))
            } else {
                Date::new(&JsValue::from_str(&format!("{}T00:00:00Z", ends_at)))
            };

            let remaining_ms = incubation_ends.get_time() - now.get_time();
            let seconds_remaining = (remaining_ms / 1000.0).floor() as i64;
            let total_incubation = 82800.0; // 23 hours (82800 seconds)
            let progress_percent = ((total_incubation - seconds_remaining as f64) / total_incubation * 100.0).max(0.0).min(100.0);
            
            status = Some(IncubationStatus {
                is_incubating: seconds_remaining > 0,
                seconds_remaining: seconds_remaining.abs(),
                progress_percent,
            });
        }
        use_state(|| status)
    };

    {
        let incubation_status = incubation_status.clone();
        let incubation_ends_at = props.egg.incubation_ends_at.clone();

        use_effect_with((), move |_| {
            let calculate_time = {
                let incubation_status = incubation_status.clone();
                let incubation_ends_at = incubation_ends_at.clone();
                
                move || {
                    if let Some(ends_at) = &incubation_ends_at {
                        let now = Date::new_0();
                        let incubation_ends = if ends_at.contains('T') {
                            Date::new(&JsValue::from_str(ends_at))
                        } else {
                            Date::new(&JsValue::from_str(&format!("{}T00:00:00Z", ends_at)))
                        };

                        let remaining_ms = incubation_ends.get_time() - now.get_time();
                        let seconds_remaining = (remaining_ms / 1000.0).floor() as i64;
                        let total_incubation = 82800.0; // 23 hours (82800 seconds)
                        let progress_percent = ((total_incubation - seconds_remaining as f64) / total_incubation * 100.0).max(0.0).min(100.0);

                        incubation_status.set(Some(IncubationStatus {
                            is_incubating: seconds_remaining > 0,
                            seconds_remaining: seconds_remaining.abs(),
                            progress_percent,
                        }));
                    }
                }
            };

            calculate_time();
            let interval = gloo_timers::callback::Interval::new(1000, move || {
                calculate_time();
            });

            move || drop(interval)
        });
    }

    html! {
        <div class="grid grid-cols-1 md:grid-cols-[1fr_3.3fr_1fr] gap-4">
            // Left column - History
            <div class="space-y-4 order-2 md:order-1">
                <div class={styles::FOCUS_CARD}>
                    <h3 class={styles::FOCUS_CARD_TITLE}>{"History"}</h3>
                    <div class={styles::FOCUS_GRID_CONTENT}>
                        <div class="space-y-1">
                            <div class={styles::FOCUS_LABEL}>{"Summon"}</div>
                            <div class={styles::FOCUS_GROUP}>
                                <div class={styles::FOCUS_VALUE}>
                                    {props.egg.summoned_by_username.clone().unwrap_or_else(|| "Unknown".to_string())}
                                </div>
                                {if let Some(created_at) = &props.egg.created_at {
                                    html! {
                                        <div class={styles::FOCUS_VALUE_SECONDARY}>
                                            {format_datetime(created_at)}
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // Middle column - Image and Title
            <div class="space-y-4 order-1 md:order-2">
                <div class="aspect-square w-full mx-auto flex items-center justify-center bg-gray-50 dark:bg-gray-800 rounded-lg overflow-hidden ring-1 ring-gray-200 dark:ring-white/10">
                    {if let Some(url) = get_image_url(&DisplayItem::Egg(props.egg.clone())) {
                        html! { <img src={url} draggable="false" class="w-full h-full object-contain select-none" alt="Egg" /> }
                    } else {
                        html! { <div class="w-full h-full flex items-center justify-center"><span class="text-6xl">{"🥚"}</span></div> }
                    }}
                </div>
                <div class="mt-4">
                    <div class="flex items-center justify-between">
                        <h2 class={styles::FOCUS_TITLE}>
                            {format!(
                                "{} Egg", 
                                props.egg.essence.clone().unwrap_or_else(|| "Magical".to_string())
                            )}
                        </h2>
                        {if let Some(label) = &props.action_label {
                            if let Some(status) = &*incubation_status {
                                if !status.is_incubating {
                                    html! {
                                        <button 
                                            onclick={props.on_action.clone().map(|callback| {
                                                let callback = callback.clone();
                                                Callback::from(move |_: MouseEvent| callback.emit(()))
                                            })}
                                            disabled={props.loading || props.error.contains("Too Many Requests")}
                                            class={classes!(
                                                styles::FOCUS_BUTTON,
                                                if props.loading || props.error.contains("Too Many Requests") {
                                                    "bg-gradient-to-r from-red-500 to-red-600 text-white cursor-not-allowed"
                                                } else {
                                                    "bg-gradient-to-r from-blue-500 to-purple-500 text-white hover:opacity-90"
                                                }
                                            )}
                                        >
                                            {if props.loading {
                                                html! {
                                                    <div class="flex items-center justify-center">
                                                        <svg class="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                        </svg>
                                                        {&label}
                                                    </div>
                                                }
                                            } else if props.error.contains("Too Many Requests") {
                                                html! { {"Too Many Requests"} }
                                            } else {
                                                html! { {&label} }
                                            }}
                                        </button>
                                    }
                                } else {
                                    html! {}
                                }
                            } else {
                                html! {}
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>
            </div>
            
            // Right column - Details and Incubation
            <div class="space-y-4 order-3">
                <div class={styles::FOCUS_CARD}>
                    <h3 class={classes!(styles::FOCUS_CARD_TITLE, "text-center")}>{"Details"}</h3>
                    <div class={styles::FOCUS_GRID_CONTENT}>
                        {for get_egg_details(&props.egg).into_iter().map(|(label, value)| {
                            html! {
                                <div class="space-y-1 text-center">
                                    <div class={styles::FOCUS_LABEL}>{label}</div>
                                    <div class={styles::FOCUS_VALUE}>
                                        {value.unwrap_or_else(|| "Unknown".to_string())}
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>

                <div class={styles::FOCUS_CARD}>
                    <div class="flex flex-col space-y-4">
                        <h3 class={styles::FOCUS_CARD_TITLE}>{"Incubation"}</h3>
                        <div class="space-y-2">
                            {if let Some(status) = &*incubation_status {
                                html! {
                                    <>
                                        <div class="flex justify-between items-center">
                                            <span class={styles::FOCUS_VALUE}>
                                                {if status.is_incubating {
                                                    format!("{}", format_time_remaining(status.seconds_remaining))
                                                } else {
                                                    "Ready to hatch!".to_string()
                                                }}
                                            </span>
                                        </div>
                                        <div class="relative h-2 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden ring-1 ring-gray-200 dark:ring-white/10">
                                            <div class={classes!(
                                                "absolute", "top-0", "left-0", "h-full", "rounded-full", "transition-all", "duration-300",
                                                if status.is_incubating {
                                                    "bg-gradient-to-r from-blue-500 to-purple-500"
                                                } else {
                                                    "bg-gradient-to-r from-green-500 to-green-600"
                                                }
                                            )}
                                            style={format!("width: {}%", status.progress_percent)} />
                                        </div>
                                    </>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    </div>
                </div>
            </div>

            {if !props.error.is_empty() && !props.error.contains("Too Many Requests") {
                html! {
                    <div class="mt-4">
                        <div class="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300">
                            {&props.error}
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

fn format_datetime(datetime: &str) -> String {
    let date = Date::new(&JsValue::from_str(datetime));
    let options = js_sys::Object::new();
    
    // Set date formatting options
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("year"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("month"), &JsValue::from_str("long"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("day"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("hour"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("minute"), &JsValue::from_str("numeric"));
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("hour12"), &JsValue::from_bool(true));
    
    date.to_locale_string("en-US", &options)
        .as_string()
        .unwrap_or_else(|| datetime.to_string())
}
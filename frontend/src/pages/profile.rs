use yew::prelude::*;
use gloo_net::http::Request;
use serde::Deserialize;
use web_sys::window;
use yew_router::prelude::*;
use crate::{Route, base::Base, styles};
use web_sys::js_sys::{Date, Object, Reflect};
use wasm_bindgen::JsValue;
use crate::config::get_asset_url;
use crate::config::get_api_base_url;
use crate::components::GradientBackground;

#[derive(Deserialize, Clone, PartialEq)]
pub struct UserProfile {
    username: String,
    email: String,
    currency_balance: i32,
    experience: i32,
    rank: Option<String>,
    last_login: Option<String>,
    created_at: String,
    is_member: bool,
}

fn format_datetime(datetime_str: &str) -> String {
    let date = Date::new(&JsValue::from_str(&(datetime_str.replace(" ", "T") + "Z")));
    
    let options = Object::new();
    Reflect::set(&options, &"month".into(), &"long".into()).ok();
    Reflect::set(&options, &"day".into(), &"numeric".into()).ok();
    Reflect::set(&options, &"year".into(), &"numeric".into()).ok();
    Reflect::set(&options, &"hour".into(), &"numeric".into()).ok();
    Reflect::set(&options, &"minute".into(), &"numeric".into()).ok();
    Reflect::set(&options, &"second".into(), &"numeric".into()).ok();
    Reflect::set(&options, &"hour12".into(), &true.into()).ok();
    
    date.to_locale_string("en-US", &options)
        .as_string()
        .unwrap_or_else(|| datetime_str.to_string())
}

#[function_component(Profile)]
pub fn profile() -> Html {
    let profile = use_state(|| None::<UserProfile>);
    let error = use_state(String::new);
    let navigator = use_navigator().unwrap();

    {
        let profile = profile.clone();
        let error = error.clone();
        let navigator = navigator.clone();
     
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let token = window()
                    .and_then(|w| w.local_storage().ok().flatten())
                    .and_then(|s| s.get_item("token").ok().flatten())
                    .or_else(|| {
                        window()
                            .and_then(|w| w.session_storage().ok().flatten())
                            .and_then(|s| s.get_item("token").ok().flatten())
                    });
     
                match token {
                    Some(token) => {
                        match Request::get(&format!("{}/api/profile", get_api_base_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await {
                                Ok(response) => {
                                    if response.status() == 401 {
                                        error.set("Session expired. Please login again.".to_string());
                                        navigator.push(&Route::Login);
                                    } else if response.ok() {
                                        match response.json::<UserProfile>().await {
                                            Ok(data) => profile.set(Some(data)),
                                            Err(e) => error.set(format!("Failed to parse profile data: {}", e)),
                                        }
                                    } else {
                                        error.set(format!("Server error: {}", response.status()));
                                    }
                                }
                                Err(e) => error.set(format!("Network error: {}", e)),
                            }
                    }
                    None => {
                        error.set("Please login to view your profile".to_string());
                        navigator.push(&Route::Login);
                    }
                }
            });
            || ()
        });
    }

    let handle_settings = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Settings);
        })
    };

    html! {
        <Base>
            <GradientBackground>
                <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
                    if let Some(user) = (*profile).clone() {
                        <div class="max-w-lg mx-auto">
                            <div class={format!("{} {}", styles::HERO_FEATURES, "mb-8 p-8")}>
                                <div class="flex flex-col items-center text-center space-y-4">
                                    <div class="relative">
                                        <div class={format!("{} {}", styles::ICON_WRAPPER_BLUE, "w-24 h-24")}>
                                            <svg class="w-14 h-14" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                                                    d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                                            </svg>
                                        </div>
                                        <div class="absolute -bottom-1 -right-1 bg-green-500 rounded-full w-5 h-5 border-2 border-white dark:border-gray-800"></div>
                                    </div>
                                    <div class="text-center">
                                        <h1 class="text-3xl font-bold text-gray-900 dark:text-white truncate max-w-[300px]">{user.username}</h1>
                                        <p class={format!("{} {}", styles::TEXT_SECONDARY, "mt-1")}>{user.email}</p>
                                    </div>

                                    <div class="w-full max-w-xs mx-auto border-t border-gray-200 dark:border-gray-700 my-8"></div>

                                    <div class="w-full space-y-6">
                                        <div class="flex justify-center">
                                            <div class="inline-flex items-center justify-between gap-2 p-4 rounded-xl bg-gradient-to-r from-green-100/50 to-blue-100/50 dark:from-green-900/30 dark:to-blue-900/30 border border-green-200 dark:border-green-800">
                                                <span class={format!("{} {}", styles::TEXT_H2, "text-green-600 dark:text-green-400")}>{user.currency_balance}</span>
                                                <div class="flex items-center">
                                                    <img src={get_asset_url("/static/images/pax-icon-black-0.png")} alt="pax icon" class="block dark:hidden w-10 h-10" />
                                                    <img src={get_asset_url("/static/images/pax-icon-white-0.png")} alt="pax icon" class="hidden dark:block w-10 h-10" />
                                                </div>
                                            </div>
                                        </div>

                                        <div class="space-y-4">
                                            <div class="flex items-center justify-between">
                                                <div class="flex items-center space-x-4">
                                                    <div class={styles::ICON_WRAPPER_PURPLE}>
                                                        <svg class={styles::ICON} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                                                                d="M9 12l2 2 4-4M7.835 4.697a3.42 3.42 0 001.946-.806 3.42 3.42 0 014.438 0 3.42 3.42 0 001.946.806 3.42 3.42 0 013.138 3.138 3.42 3.42 0 00.806 1.946 3.42 3.42 0 010 4.438 3.42 3.42 0 00-.806 1.946 3.42 3.42 0 01-3.138 3.138 3.42 3.42 0 00-1.946.806 3.42 3.42 0 01-4.438 0 3.42 3.42 0 00-1.946-.806 3.42 3.42 0 01-3.138-3.138 3.42 3.42 0 00-.806-1.946 3.42 3.42 0 010-4.438 3.42 3.42 0 00.806-1.946 3.42 3.42 0 013.138-3.138z" />
                                                        </svg>
                                                    </div>
                                                    <span class={styles::TEXT_BODY}>{"Current Rank"}</span>
                                                </div>
                                                <span class={format!("{} {}", styles::TEXT_H3, "text-purple-600 dark:text-purple-400")}>
                                                    {user.rank.as_deref().unwrap_or("Unranked")}
                                                </span>
                                            </div>
                                            <div class="space-y-2">
                                                <div class="flex justify-between items-center">
                                                    <span class={styles::TEXT_SMALL}>{"Progress to Next Rank"}</span>
                                                    <span class={format!("{} {}", styles::TEXT_SMALL, "text-purple-600 dark:text-purple-400")}>
                                                        {format!("{} XP", user.experience)}
                                                    </span>
                                                </div>
                                                <div class="w-full bg-purple-100 dark:bg-purple-900/50 rounded-full h-2">
                                                    <div 
                                                        class="bg-gradient-to-r from-purple-600 to-blue-600 h-2 rounded-full transition-all duration-500"
                                                        style={format!("width: {}%", user.experience % 100)}
                                                    />
                                                </div>
                                            </div>
                                        </div>

                                        <div class="space-y-4">
                                            <div class="flex items-center space-x-4">
                                                <div class={styles::ICON_WRAPPER_BLUE}>
                                                    <svg class={styles::ICON} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                                                            d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                                                    </svg>
                                                </div>
                                                <span class={styles::TEXT_BODY}>{"Account Activity"}</span>
                                            </div>
                                            <div class="space-y-4">
                                                <div class="p-4 rounded-xl bg-blue-100/50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800">
                                                    <p class={styles::TEXT_SMALL}>{"Last Login"}</p>
                                                    <p class={format!("{} {}", styles::TEXT_BODY, "mt-1")}>
                                                        {user.last_login.as_deref().map(format_datetime).unwrap_or_else(|| "Never".to_string())}
                                                    </p>
                                                </div>
                                                <div class="p-4 rounded-xl bg-blue-100/50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800">
                                                    <p class={styles::TEXT_SMALL}>{"Account Created"}</p>
                                                    <p class={format!("{} {}", styles::TEXT_BODY, "mt-1")}>
                                                        {format_datetime(&user.created_at)}
                                                    </p>
                                                </div>
                                                <div class="p-4 rounded-xl bg-blue-100/50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800">
                                                    <p class={styles::TEXT_SMALL}>{"Membership Status"}</p>
                                                    <div class="flex justify-center mt-1">
                                                        <p class={format!("{} {}", 
                                                            styles::TEXT_BODY, 
                                                            if user.is_member { "text-green-600 dark:text-green-400" } else { "text-gray-600 dark:text-gray-400" }
                                                        )}>
                                                            {if user.is_member { "Active Member" } else { "Free User" }}
                                                        </p>
                                                    </div>
                                                </div>
                                                
                                                <button 
                                                    onclick={handle_settings}
                                                    class="w-full flex items-center justify-center px-4 py-3 rounded-xl bg-blue-500 hover:bg-blue-600 dark:bg-blue-600 dark:hover:bg-blue-700 text-white transition-all duration-200 hover:scale-[1.02] group"
                                                >
                                                    <span class="font-medium">{"Settings"}</span>
                                                    <svg class="w-4 h-4 ml-2 group-hover:translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                                                            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                                                    </svg>
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    } else if !(*error).is_empty() {
                        <div class={styles::ALERT_ERROR}>{&*error}</div>
                    } else {
                        <div class="flex justify-center items-center min-h-[50vh]">
                            <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500" />
                        </div>
                    }
                </div>
            </GradientBackground>
        </Base>
    }
}
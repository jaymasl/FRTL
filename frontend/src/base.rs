use yew::prelude::*;
use yew_router::prelude::*;
use gloo_net::http::Request;
use serde::Deserialize;
use crate::{Route, styles};
use web_sys::{window, MouseEvent, Event, CustomEvent, CustomEventInit};
use wasm_bindgen::{JsValue, JsCast};
use js_sys;
use crate::hooks::use_currency::use_currency;
use gloo::events::EventListener;
use crate::config::get_api_base_url;

const CURRENCY_UPDATE_EVENT: &str = "currencyUpdate";
pub const MEMBERSHIP_UPDATE_EVENT: &str = "membershipUpdate";
const NOTIFICATION_EVENT: &str = "notification";

#[derive(Deserialize)]
struct UserProfile {
    currency_balance: i32,
    is_member: bool,
}

#[derive(Properties, PartialEq)]
pub struct BaseProps {
    pub children: Html,
}

fn dispatch_currency_event(amount: i32) {
    if let Some(window) = window() {
        let event_init = CustomEventInit::new();
        event_init.set_detail(&JsValue::from_f64(amount as f64));
        let event = CustomEvent::new_with_event_init_dict(
            CURRENCY_UPDATE_EVENT,
            &event_init
        ).unwrap();
        window.dispatch_event(&event).unwrap();
    }
}

pub fn dispatch_membership_event(is_member: bool) {
    if let Some(window) = window() {
        let event_init = CustomEventInit::new();
        event_init.set_detail(&JsValue::from_bool(is_member));
        let event = CustomEvent::new_with_event_init_dict(
            MEMBERSHIP_UPDATE_EVENT,
            &event_init
        ).unwrap();
        window.dispatch_event(&event).unwrap();
        
        if let Some(storage) = window.local_storage().ok().flatten() {
            let _ = storage.set_item("is_member", &is_member.to_string());
        }
    }
}

async fn check_auth_async() -> (bool, String, i32) {
    let window = window().unwrap();
    let local = window.local_storage().ok().flatten();
    let session = window.session_storage().ok().flatten();
    
    for storage in [local, session].into_iter().flatten() {
        let token = storage.get_item("token").ok().flatten();
        let username = storage.get_item("username").ok().flatten();

        if let (Some(token), Some(username)) = (token.clone(), username.clone()) {
            match Request::get(&format!("{}/api/profile", get_api_base_url()))
                .header("Authorization", &format!("Bearer {}", token))
                .send()
                .await 
            {
                Ok(response) if response.status() == 200 => {
                    if let Ok(profile) = response.json::<UserProfile>().await {
                        if let Some(storage) = window.local_storage().ok().flatten() {
                            let _ = storage.set_item("currency", &profile.currency_balance.to_string());
                            let _ = storage.set_item("is_member", &profile.is_member.to_string());
                        }
                        dispatch_membership_event(profile.is_member);
                        return (true, username, profile.currency_balance);
                    }
                },
                Ok(response) if response.status() == 401 => {
                    storage.remove_item("token").ok();
                    storage.remove_item("username").ok();
                    return (false, String::new(), 0);
                },
                _ => {}
            }
        }
    }
    (false, String::new(), 0)
}

fn check_auth() -> (bool, String, i32) {
    let window = window().unwrap();
    let local = window.local_storage().ok().flatten();
    let session = window.session_storage().ok().flatten();
    
    for storage in [local, session].into_iter().flatten() {
        let token = storage.get_item("token").ok().flatten();
        let username = storage.get_item("username").ok().flatten();
        if let (Some(_), Some(username)) = (token, username.clone()) {
            return (true, username, 0);
        }
    }
    (false, String::new(), 0)
}

fn handle_theme_toggle(dark_mode: bool) {
    if let Some(document) = window().and_then(|w| Some(w.document()?)) {
        if let Some(html) = document.document_element() {
            html.set_class_name(if dark_mode { "dark" } else { "light" });
            window().unwrap().local_storage().unwrap().unwrap()
                .set_item("theme", if dark_mode { "dark" } else { "light" }).unwrap();
        }
    }
}

#[function_component(Base)]
pub fn base(props: &BaseProps) -> Html {
    let dark_mode = use_state(|| window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("theme").ok().flatten())
        .map_or(true, |theme| theme == "dark")
    );
    
    let navigator = use_navigator().unwrap();
    let (logged_in, username, _) = check_auth();
    let auth_state = use_state(|| (logged_in, username.clone(), 0));
    let show_dropdown = use_state(|| false);
    let current_currency = use_currency();
    let is_member = use_state(|| window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("is_member").ok().flatten())
        .map_or(false, |status| status == "true")
    );

    let global_error = use_state(|| None::<String>);

    // Add notification state
    let notification = use_state(|| None::<String>);
    let show_notification = use_state(|| false);

    {
        let global_error = global_error.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().expect("no global window exists");
            let listener = EventListener::new(&window, "ipRateLimitError", move |event: &Event| {
                if let Some(custom_event) = event.dyn_ref::<web_sys::CustomEvent>() {
                    let error_message = custom_event.detail().as_string().unwrap_or_else(|| "Rate limit exceeded.".to_string());
                    global_error.set(Some(error_message));
                }
            });
            || drop(listener)
        });
    }

    {
        let is_member = is_member.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().expect("no global window exists");
            
            // Listen for membership update events
            let is_member_for_event = is_member.clone();
            let listener = EventListener::new(&window, MEMBERSHIP_UPDATE_EVENT, move |event: &Event| {
                if let Some(custom_event) = event.dyn_ref::<web_sys::CustomEvent>() {
                    if let Some(detail) = custom_event.detail().as_bool() {
                        is_member_for_event.set(detail);
                    }
                }
            });
            
            // Check membership status immediately and periodically
            let is_member_clone = is_member.clone();
            let check_membership = move || {
                if let Some(window) = web_sys::window() {
                    if let Some(storage) = window.local_storage().ok().flatten() {
                        if let Ok(Some(member_status)) = storage.get_item("is_member") {
                            is_member_clone.set(member_status == "true");
                        }
                    }
                }
            };
            
            // Check immediately
            check_membership();
            
            // Set up interval to check periodically (every 5 seconds)
            let interval = gloo_timers::callback::Interval::new(5000, check_membership);
            
            move || {
                drop(listener);
                drop(interval);
            }
        });
    }

    {
        let current_currency = current_currency.clone();
        let auth_state = auth_state.clone();
        let is_member = is_member.clone();
        use_effect_with(auth_state.clone(), move |auth_state| {
            if auth_state.0 {
                wasm_bindgen_futures::spawn_local(async move {
                    let (_, _, currency) = check_auth_async().await;
                    current_currency.set(currency);
                    dispatch_currency_event(currency);
                    
                    // Update membership status from local storage
                    if let Some(window) = window() {
                        if let Some(storage) = window.local_storage().ok().flatten() {
                            if let Ok(Some(member_status)) = storage.get_item("is_member") {
                                is_member.set(member_status == "true");
                            }
                        }
                    }
                });
            }
            || ()
        });
    }

    {
        let show_dropdown = show_dropdown.clone();
        use_effect_with((), move |_| {
            let document = window().unwrap().document().unwrap();
            let event_listener = gloo::events::EventListener::new(&document, "click", move |event: &Event| {
                if let Some(target) = event.target() {
                    if let Ok(element) = target.dyn_into::<web_sys::Element>() {
                        if let Ok(matches) = element.matches(".dropdown-container, .dropdown-container *") {
                            if !matches {
                                show_dropdown.set(false);
                            }
                        }
                    }
                }
            });
            || drop(event_listener)
        });
    }

    {
        let auth_state = auth_state.clone();
        use_effect_with((), move |_| {
            let (logged_in, username, currency) = check_auth();
            auth_state.set((logged_in, username, currency));
            || ()
        });
    }

    // Add notification event listener
    {
        let notification = notification.clone();
        let show_notification = show_notification.clone();
        
        use_effect_with((), move |_| {
            let window = web_sys::window().expect("no global window exists");
            
            // Listen for notification events
            let notification_clone = notification.clone();
            let show_notification_clone = show_notification.clone();
            
            let listener = EventListener::new(&window, NOTIFICATION_EVENT, move |event: &Event| {
                if let Some(custom_event) = event.dyn_ref::<web_sys::CustomEvent>() {
                    if let Some(message) = custom_event.detail().as_string() {
                        notification_clone.set(Some(message));
                        show_notification_clone.set(true);
                        
                        // Auto-hide after 3 seconds
                        let show_notification_inner = show_notification_clone.clone();
                        let timeout = gloo_timers::callback::Timeout::new(3000, move || {
                            show_notification_inner.set(false);
                        });
                        let _timeout_handle = timeout.forget();
                    }
                }
            });
            
            move || {
                drop(listener);
            }
        });
    }

    let toggle_dropdown = {
        let show_dropdown = show_dropdown.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            show_dropdown.set(!*show_dropdown);
        })
    };

    let toggle_theme = {
        let dark_mode = dark_mode.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let new_mode = !*dark_mode;
            handle_theme_toggle(new_mode);
            dark_mode.set(new_mode);
        })
    };

    let handle_logout = {
        let navigator = navigator.clone();
        let auth_state = auth_state.clone();
        let show_dropdown = show_dropdown.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            for storage in [window().unwrap().local_storage().unwrap().unwrap(),
                          window().unwrap().session_storage().unwrap().unwrap()].iter() {
                storage.remove_item("token").ok();
                storage.remove_item("username").ok();
                storage.remove_item("csrf_token").ok();
                storage.remove_item("currency").ok();
                storage.remove_item("user_id").ok();
                storage.remove_item("is_member").ok();
            }

            if let Some(window) = window() {
                let _ = js_sys::Reflect::set(
                    &window,
                    &JsValue::from_str("hcaptchaToken"),
                    &JsValue::null()
                );
            }

            auth_state.set((false, String::new(), 0));
            show_dropdown.set(false);
            navigator.push(&Route::Login);
        })
    };

    let theme_icon = if *dark_mode { html! { "☀️" } } else { html! { "🌙" } };

    html! {
        <div class={if *dark_mode { "dark h-full bg-gray-900" } else { "h-full bg-gray-50" }}>
            <nav class={styles::NAV}>
                <div class="w-full mx-auto px-4 sm:px-6 lg:px-8">
                    <div class="h-16 flex items-center justify-between relative">
                        <div class="flex items-center">
                            <Link<Route> to={Route::Home} classes={styles::NAV_BRAND}>{"FRTL"}</Link<Route>>
                            if (*auth_state).0 {
                                <div class="hidden md:flex items-center ml-8 space-x-4" />
                            }
                        </div>
                        
                        <div class="absolute left-0 right-0 hidden md:flex justify-center pointer-events-none">
                        </div>
                        
                        <div class={styles::NAV_ITEMS}>
                            if (*auth_state).0 {
                                <div class="relative dropdown-container">
                                    <div class="flex items-center space-x-4">
                                        <div class="flex items-center space-x-1 pr-1 pl-2 py-1 bg-gray-100 dark:bg-gray-700 rounded-lg">
                                            <span class="text-sm font-medium text-blue-700 dark:text-blue-300">
                                                {*current_currency}
                                            </span>
                                            <img src="/static/images/pax-icon-black-0.png" alt="pax icon" class="block dark:hidden w-5 h-5" />
                                            <img src="/static/images/pax-icon-white-0.png" alt="pax icon" class="hidden dark:block w-5 h-5" />
                                        </div>
                                        <div class={format!("flex items-center px-3 py-1 h-[28px] rounded-lg text-sm font-medium {}", 
                                            if *is_member {
                                                "bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400"
                                            } else {
                                                "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-400"
                                            }
                                        )}>
                                            {if *is_member { "Member" } else { "Free" }}
                                        </div>
                                        <div class="relative">
                                            <button onclick={toggle_dropdown.clone()} class={styles::NAV_LINK}>
                                                {(*auth_state).1.clone()}
                                            </button>
                                            if *show_dropdown {
                                                <div class={classes!(styles::DROPDOWN, "min-w-[160px]", "top-full", "mt-2")}>
                                                    <Link<Route> to={Route::Home} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Home"}
                                                    </Link<Route>>
                                                    <Link<Route> to={Route::Dashboard} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Dashboard"}
                                                    </Link<Route>>
                                                    <Link<Route> to={Route::Profile} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Profile"}
                                                    </Link<Route>>
                                                    <Link<Route> to={Route::Inventory} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Inventory"}
                                                    </Link<Route>>
                                                    <Link<Route> to={Route::Games} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Games"}
                                                    </Link<Route>>
                                                    <Link<Route> to={Route::Market} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Market"}
                                                    </Link<Route>>
                                                    <Link<Route> to={Route::Settings} classes={classes!(styles::DROPDOWN_BUTTON, "flex", "justify-center")}>
                                                        {"Settings"}
                                                    </Link<Route>>
                                                    <button onclick={handle_logout} 
                                                        class={classes!(
                                                            styles::DROPDOWN_BUTTON, 
                                                            "flex", 
                                                            "justify-center", 
                                                            "text-red-700",
                                                            "dark:text-red-400",
                                                            "hover:text-red-800",
                                                            "dark:hover:text-red-300",
                                                            "hover:bg-red-50",
                                                            "dark:hover:bg-red-900/20"
                                                        )}>
                                                        {"Logout"}
                                                    </button>
                                                </div>
                                            }
                                        </div>
                                    </div>
                                </div>
                            } else {
                                <Link<Route> to={Route::Login} classes={styles::NAV_LINK}>{"Login"}</Link<Route>>
                                <Link<Route> to={Route::Register} classes={styles::NAV_LINK}>{"Register"}</Link<Route>>
                            }
                            <button onclick={toggle_theme} class={styles::BUTTON_ICON}>{theme_icon}</button>
                        </div>
                    </div>
                </div>
            </nav>
            { if let Some(error_msg) = &*global_error {
                html! { <div class="error-banner bg-red-500 text-white p-2 text-center">{ error_msg }</div> }
            } else { html! {} } }
            { if *show_notification {
                html!{
                    <div class="fixed top-6 right-6 z-50 max-w-md bg-green-50 border border-green-200 text-green-800 px-4 py-3 rounded-lg shadow-lg animate-fade-in flex items-center">
                        <svg class="h-5 w-5 text-green-400 mr-2" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                        </svg>
                        <div>
                            {(*notification).as_deref().unwrap_or("")}
                        </div>
                    </div>
                }
            } else { html! {} } }
            <main class="pt-16">{props.children.clone()}</main>
            <footer class="w-full bg-white/80 dark:bg-gray-900/80 backdrop-blur-md border-t border-gray-200/50 dark:border-gray-700/50 relative z-0">
                <div class="w-full mx-auto px-4 sm:px-6 lg:px-8">
                    <div class="h-16 flex items-center justify-center">
                        <a href="https://jaykrown.com" 
                           target="_blank" 
                           rel="noopener noreferrer"
                           class="text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-200">
                            {"jaykrown.com"}
                        </a>
                    </div>
                </div>
            </footer>
        </div>
    }
}
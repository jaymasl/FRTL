pub mod base;
pub mod styles;
pub mod hooks;
pub mod models;
pub mod components;
pub mod pages;
pub mod config;

use yew::prelude::*;
use yew_router::prelude::*;
use crate::pages::{
   auth::{Auth, AuthMode},
   home::Home,
   profile::Profile,
   inventory::Inventory,
   market::Market,
   settings::Settings,
   games::Games,
   dashboard::Dashboard,
   verify_magic_link::VerifyMagicLink,
   verify_delete_account::VerifyDeleteAccount,
};

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
   #[at("/")] Home,
   #[at("/login")] Login,
   #[at("/register")] Register,
   #[at("/verify-magic-link")] VerifyMagicLink,
   #[at("/verify-delete-account")] VerifyDeleteAccount,
   #[at("/profile")] Profile,
   #[at("/inventory")] Inventory,
   #[at("/market")] Market,
   #[at("/settings")] Settings,
   #[at("/games")] Games,
   #[at("/dashboard")] Dashboard,
}

#[derive(Properties, PartialEq, Default)]
pub struct AppProps {
    pub callback: Option<Callback<()>>,
}

#[function_component(App)]
pub fn app() -> Html {
    // Check membership status on mount and periodically
    {
        use_effect_with((), move |_| {
            // Function to check membership status
            let check_membership = move || {
                if let Some(window) = web_sys::window() {
                    if let Some(storage) = window.local_storage().ok().flatten() {
                        // Check if we need to refresh membership status
                        if let Ok(Some(member_until_str)) = storage.get_item("membership_expiry") {
                            if let Ok(expiry_time) = member_until_str.parse::<f64>() {
                                let now = js_sys::Date::now();
                                
                                // If expiry time is in the past, update membership status
                                if expiry_time <= now {
                                    if let Ok(Some(is_member)) = storage.get_item("is_member") {
                                        if is_member == "true" {
                                            // Membership has expired, update status
                                            let _ = storage.set_item("is_member", "false");
                                            
                                            // Dispatch event to update UI
                                            let event_init = web_sys::CustomEventInit::new();
                                            event_init.set_detail(&wasm_bindgen::JsValue::from_bool(false));
                                            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                                base::MEMBERSHIP_UPDATE_EVENT,
                                                &event_init
                                            ) {
                                                let _ = window.dispatch_event(&event);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
            
            // Check immediately
            check_membership();
            
            // Set up interval to check periodically (every 5 seconds)
            let interval = gloo_timers::callback::Interval::new(5000, check_membership);
            
            move || {
                drop(interval);
            }
        });
    }
    
    html! {
        <BrowserRouter>
            <div class="min-h-screen w-full">
                <div class="mx-auto">
                    <Switch<Route> render={switch} />
                </div>
            </div>
        </BrowserRouter>
    }
}

pub fn switch(route: Route) -> Html {
   match route {
       Route::Home => html! { <Home /> },
       Route::Login => html! { <Auth mode={AuthMode::Login} /> },
       Route::Register => html! { <Auth mode={AuthMode::Register} /> },
       Route::VerifyMagicLink => html! { <VerifyMagicLink /> },
       Route::VerifyDeleteAccount => html! { <VerifyDeleteAccount /> },
       Route::Profile => html! { <Profile /> },
       Route::Inventory => html! { <Inventory /> },
       Route::Market => html! { <Market /> },
       Route::Settings => html! { <Settings /> },
       Route::Games => html! { <Games /> },
       Route::Dashboard => html! { <Dashboard /> },
   }
}
use yew::prelude::*;
use web_sys::window;
use yew_router::prelude::*;
use crate::Route;

fn get_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
        .or_else(|| {
            window()
                .and_then(|w| w.session_storage().ok().flatten())
                .and_then(|s| s.get_item("token").ok().flatten())
        })
}

fn has_valid_token() -> bool {
    get_token().is_some()
}

#[hook]
pub fn use_auth_token() -> String {
    let token = use_state(|| get_token().unwrap_or_default());

    {
        let token = token.clone();
        use_effect_with((), move |_| {
            token.set(get_token().unwrap_or_default());
            || ()
        });
    }

    (*token).clone()
}

#[hook]
pub fn use_auth_state() -> bool {
    let logged_in = use_state(|| false);

    {
        let logged_in = logged_in.clone();
        use_effect_with((), move |_| {
            logged_in.set(has_valid_token());
            || ()
        });
    }

    *logged_in
}

#[hook]
pub fn use_auth_check() {
    let navigator = use_navigator().expect("Navigator not available");

    let check_auth = {
        let navigator = navigator.clone();
        move || {
            if !has_valid_token() {
                if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        storage.remove_item("token").ok();
                    }
                    if let Ok(Some(storage)) = window.session_storage() {
                        storage.remove_item("token").ok();
                    }
                }
                navigator.push(&Route::Login);
            }
        }
    };

    {
        let check_auth = check_auth.clone();
        use_effect_with((), move |_| {
            check_auth();
            let interval = gloo_timers::callback::Interval::new(30_000, move || {
                check_auth();
            });
            move || drop(interval)
        });
    }
}
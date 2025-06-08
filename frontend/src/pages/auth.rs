use yew::prelude::*;
use yew_router::prelude::*;
use web_sys::window;

use crate::{
    Route,
    base::Base,
    components::auth::{LoginForm, RegisterForm},
    styles,
};
use crate::components::GradientBackground;

#[derive(Clone, PartialEq)]
pub enum AuthMode {
    Login,
    Register,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub mode: AuthMode,
}

#[function_component(Auth)]
pub fn auth(props: &Props) -> Html {
    let logged_in = window().unwrap().local_storage().unwrap().unwrap().get_item("token").unwrap().is_some() || 
                    window().unwrap().session_storage().unwrap().unwrap().get_item("token").unwrap().is_some();

    let navigator = use_navigator().unwrap();

    let on_login_success = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            navigator.push(&Route::Home);
        })
    };

    let on_register_success = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            navigator.push(&Route::Login);
        })
    };

    if logged_in {
        navigator.push(&Route::Home);
        return html! {};
    }

    html! {
        <Base>
            <GradientBackground>
                <div class="min-h-screen w-full px-4 sm:px-6 lg:px-8">
                    <div class="max-w-md mx-auto px-4 sm:px-6 py-4">
                        <div class={styles::CARD}>
                            {
                                match props.mode {
                                    AuthMode::Login => html! {
                                        <>
                                            <LoginForm on_success={on_login_success} />
                                            <div class="mt-4 text-center space-y-2">
                                                <p class={styles::TEXT_SECONDARY}>
                                                    {"Don't have an account? "}
                                                    <Link<Route> to={Route::Register} classes={styles::LINK}>
                                                        {"Register"}
                                                    </Link<Route>>
                                                </p>
                                            </div>
                                        </>
                                    },
                                    AuthMode::Register => html! {
                                        <>
                                            <RegisterForm on_success={on_register_success} />
                                            <div class="mt-4 text-center">
                                                <p class={styles::TEXT_SECONDARY}>
                                                    {"Already have an account? "}
                                                    <Link<Route> to={Route::Login} classes={styles::LINK}>
                                                        {"Login"}
                                                    </Link<Route>>
                                                </p>
                                            </div>
                                        </>
                                    },
                                }
                            }
                        </div>
                    </div>
                </div>
            </GradientBackground>
        </Base>
    }
}
use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, console};
use yew::prelude::*;
use yew_router::prelude::*;
use crate::styles;
use crate::config::get_api_base_url;

#[derive(Serialize)]
struct DeleteAccountRequest {
    email: String,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub on_success: Option<Callback<()>>,
    pub on_error: Callback<String>,
}

#[function_component(AccountManagement)]
pub fn account_management(props: &Props) -> Html {
    let show_confirm = use_state(|| false);
    let email = use_state(String::new);
    let error = use_state(String::new);
    let success = use_state(String::new);
    let loading = use_state(|| false);
    let email_sent = use_state(|| false);
    let _navigator = use_navigator().unwrap();

    let handle_submit = {
        let email = email.clone();
        let error = error.clone();
        let success = success.clone();
        let loading = loading.clone();
        let email_sent = email_sent.clone();
        let on_error = props.on_error.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            loading.set(true);
            error.set(String::new());
            success.set(String::new());

            let email_value = (*email).clone();
            let success_state = success.clone();
            let loading_state = loading.clone();
            let error_state = error.clone();
            let email_sent_state = email_sent.clone();
            let on_error = on_error.clone();

            spawn_local(async move {
                console::log_1(&"Requesting account deletion...".into());
                
                let token = {
                    let window = window().unwrap();
                    let local_storage = window.local_storage().unwrap().unwrap();
                    let session_storage = window.session_storage().unwrap().unwrap();
                    
                    let local_token = local_storage.get_item("token").unwrap();
                    let session_token = session_storage.get_item("token").unwrap();
                    
                    match (local_token, session_token) {
                        (Some(token), _) | (None, Some(token)) if !token.is_empty() => token,
                        _ => {
                            console::error_1(&"No token found".into());
                            error_state.set("Not authenticated".to_string());
                            on_error.emit("Not authenticated".to_string());
                            loading_state.set(false);
                            return;
                        }
                    }
                };

                console::log_1(&"Token found".into());

                let request = DeleteAccountRequest {
                    email: email_value,
                };

                match Request::post(&format!("{}/api/users/me/delete-request", get_api_base_url()))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await 
                {
                    Ok(response) => {
                        console::log_2(&"Response status:".into(), &response.status().into());
                        if response.status() == 200 {
                            // Show success message
                            success_state.set("A confirmation link has been sent to your email. Please check your inbox to complete the account deletion.".to_string());
                            email_sent_state.set(true);
                        } else {
                            let error_text = response.text().await.unwrap_or_else(|_| 
                                "Failed to request account deletion".to_string()
                            );
                            console::error_1(&format!("Delete request failed: {}", error_text).into());
                            error_state.set(error_text.clone());
                            on_error.emit(error_text);
                        }
                    }
                    Err(e) => {
                        let error_msg = "Failed to send request".to_string();
                        console::error_2(&"Request error:".into(), &format!("{:?}", e).into());
                        error_state.set(error_msg.clone());
                        on_error.emit(error_msg);
                    }
                }
                loading_state.set(false);
            });
        })
    };

    let toggle_confirm = {
        let show_confirm = show_confirm.clone();
        let error = error.clone();
        Callback::from(move |_| {
            show_confirm.set(!*show_confirm);
            error.set(String::new());
        })
    };

    html! {
        <div class="space-y-4">
            <h3 class={styles::TEXT_H3}>{"Delete Account"}</h3>
            if *show_confirm {
                if *email_sent {
                    <div class={styles::CARD_SUCCESS}>
                        <p>{&*success}</p>
                    </div>
                } else {
                    <form onsubmit={handle_submit} class="space-y-4">
                        <div class={styles::CARD_ERROR}>
                            <p>{"This action cannot be undone. Please enter your email address to confirm."}</p>
                        </div>
                        <div>
                            <label class={styles::TEXT_LABEL}>{"Email Address"}</label>
                            <input
                                type="email"
                                required=true
                                class={styles::INPUT}
                                onchange={let email = email.clone(); move |e: Event| {
                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    email.set(input.value());
                                }}
                            />
                        </div>
                        <div class="flex space-x-4">
                            if *loading {
                                <button type="button" class={styles::BUTTON_DANGER} disabled=true>
                                    <span class="flex items-center justify-center">
                                        <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                        </svg>
                                        {"Sending..."}
                                    </span>
                                </button>
                            } else {
                                <button type="submit" class={styles::BUTTON_DANGER}>{"Send Confirmation Email"}</button>
                            }
                            <button 
                                type="button" 
                                onclick={toggle_confirm.clone()}
                                class={styles::BUTTON_SECONDARY}
                                disabled={*loading}
                            >
                                {"Cancel"}
                            </button>
                        </div>
                        {
                            (*loading).then(|| html! {
                                <p class="text-xs text-center text-gray-500 mt-2">
                                    {"Please be patient. Email delivery may take up to 30 seconds to complete."}
                                </p>
                            })
                        }
                    </form>
                }
            } else {
                <button 
                    onclick={toggle_confirm}
                    class={styles::BUTTON_DANGER}
                >
                    {"Delete Account"}
                </button>
            }
            if !(*error).is_empty() {
                <div class={styles::CARD_ERROR}>
                    <p>{&*error}</p>
                </div>
            }
        </div>
    }
}
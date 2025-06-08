use yew::prelude::*;
use web_sys::{HtmlInputElement, window, InputEvent};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use uuid::Uuid;
use crate::styles;
use crate::config::get_api_base_url;
use crate::hooks::use_membership::use_membership;
use crate::components::membership_required::MembershipRequired;
use shared::profanity::ProfanityFilter;

#[derive(Properties, PartialEq)]
pub struct RenameCreatureProps {
    pub creature_id: Uuid,
    pub current_name: String,
    pub on_success: Callback<String>,
    pub on_error: Callback<String>,
}

#[function_component(RenameCreature)]
pub fn rename_creature(props: &RenameCreatureProps) -> Html {
    let membership = use_membership();
    let new_name = use_state(|| props.current_name.clone());
    let loading = use_state(|| false);
    let error = use_state(String::new);
    
    let handle_submit = {
        let new_name = new_name.clone();
        let loading = loading.clone();
        let error = error.clone();
        let on_success = props.on_success.clone();
        let on_error = props.on_error.clone();
        let creature_id = props.creature_id;
        
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            
            if (*new_name).is_empty() {
                error.set("Name cannot be empty".to_string());
                return;
            }
            
            if (*new_name).len() > 20 {
                error.set("Name cannot exceed 20 characters".to_string());
                return;
            }
            
            // Check for profanity
            if let Err(msg) = ProfanityFilter::validate_username(&*new_name) {
                error.set(msg);
                return;
            }
            
            loading.set(true);
            error.set(String::new());
            
            let token = match window().and_then(|w| w.local_storage().ok()).flatten()
                .and_then(|storage| storage.get_item("token").ok()).flatten() {
                Some(token) => token,
                None => {
                    error.set("Not authenticated".to_string());
                    on_error.emit("Not authenticated".to_string());
                    loading.set(false);
                    return;
                }
            };
            
            let new_name_value = (*new_name).clone();
            let error_state = error.clone();
            let loading_state = loading.clone();
            let on_success = on_success.clone();
            let on_error = on_error.clone();
            
            spawn_local(async move {
                let request_body = serde_json::json!({
                    "new_name": new_name_value
                });
                
                match Request::post(&format!("{}/api/creatures/{}/rename", get_api_base_url(), creature_id))
                    .header("Content-Type", "application/json")
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request_body)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                // Update currency in local storage
                                if let Some(new_balance) = data.get("new_balance").and_then(|b| b.as_i64()) {
                                    if let Some(window) = window() {
                                        if let Some(storage) = window.local_storage().ok().flatten() {
                                            let _ = storage.set_item("currency", &new_balance.to_string());
                                        }
                                        
                                        // Dispatch currency update event
                                        let event_init = web_sys::CustomEventInit::new();
                                        event_init.set_detail(&wasm_bindgen::JsValue::from_f64(new_balance as f64));
                                        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                                            "currencyUpdate",
                                            &event_init
                                        ) {
                                            let _ = window.dispatch_event(&event);
                                        }
                                    }
                                }
                                
                                on_success.emit(new_name_value);
                            } else {
                                error_state.set("Failed to parse response".to_string());
                                on_error.emit("Failed to parse response".to_string());
                            }
                        } else {
                            let error_text = response.text().await.unwrap_or_else(|_| 
                                "Failed to rename creature".to_string()
                            );
                            error_state.set(error_text.clone());
                            on_error.emit(error_text);
                        }
                    },
                    Err(e) => {
                        let error_msg = format!("Failed to send request: {:?}", e);
                        error_state.set(error_msg.clone());
                        on_error.emit(error_msg);
                    }
                }
                
                loading_state.set(false);
            });
        })
    };
    
    if !membership.is_member {
        return html! {
            <MembershipRequired feature_name="Creature Renaming" />
        };
    }
    
    html! {
        <div class="mt-4 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
            <h3 class={styles::TEXT_H3}>{"Rename Creature"}</h3>
            <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
                {"Rename your creature for 100 pax (max 20 characters). This requires an active membership."}
            </p>
            
            <form onsubmit={handle_submit} class="space-y-4">
                <div>
                    <label for="creature-name" class={styles::TEXT_LABEL}>{"New Name"}</label>
                    <input
                        id="creature-name"
                        type="text"
                        value={(*new_name).clone()}
                        class={styles::INPUT}
                        maxlength="20"
                        onchange={let new_name = new_name.clone(); move |e: Event| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            new_name.set(input.value());
                        }}
                        oninput={let new_name = new_name.clone(); move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            new_name.set(input.value());
                        }}
                    />
                </div>
                
                if !(*error).is_empty() {
                    <div class={styles::CARD_ERROR}>
                        <p>{&*error}</p>
                    </div>
                }
                
                <div>
                    if *loading {
                        <div class={styles::LOADING_SPINNER}></div>
                    } else {
                        <button type="submit" class={styles::BUTTON_PRIMARY}>
                            {"Rename for 100 pax"}
                        </button>
                    }
                </div>
            </form>
        </div>
    }
} 
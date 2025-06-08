use yew::prelude::*;

#[derive(Clone)]
pub struct FormState {
    pub error: String,
    pub success: String,
    pub handle_success: Callback<String>,
    pub handle_error: Callback<String>,
}

#[hook]
pub fn use_form_state() -> FormState {
    let error = use_state(String::new);
    let success = use_state(String::new);

    let handle_success = {
        let success = success.clone();
        let error = error.clone();
        Callback::from(move |msg: String| {
            success.set(msg);
            error.set(String::new());
        })
    };

    let handle_error = {
        let error = error.clone();
        let success = success.clone();
        Callback::from(move |msg: String| {
            error.set(msg);
            success.set(String::new());
        })
    };

    FormState {
        error: (*error).clone(),
        success: (*success).clone(),
        handle_success,
        handle_error,
    }
}

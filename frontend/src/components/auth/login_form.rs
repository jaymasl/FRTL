use yew::prelude::*;
use super::magic_link_form::MagicLinkForm;

#[function_component(LoginForm)]
pub fn login_form(props: &LoginFormProps) -> Html {
    let on_success = props.on_success.clone();
    
    html! {
        <MagicLinkForm 
            on_success={on_success}
            on_cancel={Callback::from(|_| {})}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct LoginFormProps {
    pub on_success: Callback<()>,
}
use yew::prelude::*;
use crate::models::Scroll;
use super::{CardBase, DisplayItem};

#[derive(Properties, PartialEq)]
pub struct ScrollCardProps {
    pub scroll: Scroll,
    pub on_click: Option<Callback<DisplayItem>>,
    pub on_action: Option<Callback<()>>,
    pub action_label: Option<String>,
    pub loading: bool,
    pub error: String,
}

#[function_component(ScrollCard)]
pub fn scroll_card(props: &ScrollCardProps) -> Html {
    let on_click = props.on_click.clone().map(|cb| {
        let scroll = props.scroll.clone();
        Callback::from(move |_| {
            cb.emit(DisplayItem::Scroll(scroll.clone()));
        })
    });

    let stats = Vec::new(); // Empty vector for stats since we don't need them

    html! {
        <div class="scroll-card">
            <style>
                {r#"
                .scroll-card img {
                    object-fit: contain !important;
                    padding: 10px !important;
                }
                "#}
            </style>
            <CardBase
                item={DisplayItem::Scroll(props.scroll.clone())}
                title={html! {
                    <div class="flex items-center gap-1 pl-2">
                        <span>{props.scroll.display_name.clone()}</span>
                        <span class="text-xs text-gray-500">
                            {format!("({}x)", props.scroll.quantity)}
                        </span>
                    </div>
                }}
                stats={stats}
                on_click={on_click}
                on_action={props.on_action.clone()}
                action_label={props.action_label.clone()}
                loading={props.loading}
                error={props.error.clone()}
                is_compact=true
            />
        </div>
    }
} 
use yew::prelude::*;
use crate::models::Egg;
use super::{
    card_base::CardBase,
    DisplayItem,
    get_egg_title,
    get_egg_card_stats,
};

#[derive(Properties, PartialEq)]
pub struct EggCardProps {
    pub egg: Egg,
    pub action_label: Option<String>,
    pub on_action: Option<Callback<()>>,
    pub on_click: Option<Callback<DisplayItem>>,
    pub loading: bool,
    #[prop_or_default]
    pub error: String,
}

#[function_component(EggCard)]
pub fn egg_card(props: &EggCardProps) -> Html {
    let update_counter = use_state(|| 0);

    {
        let update_counter = update_counter.clone();
        use_effect_with((), move |_| {
            let interval = gloo_timers::callback::Interval::new(1000, move || {
                update_counter.set(*update_counter + 1);
            });
            || drop(interval)
        });
    }

    let item = DisplayItem::Egg(props.egg.clone());
    let stats = get_egg_card_stats(&props.egg);
    let title = get_egg_title(&props.egg);
    
    let converted_stats: Vec<(String, f64, f64, String)> = stats
        .into_iter()
        .map(|(label, value, max, color)| 
            (label.to_string(), value, max, color.to_string()))
        .collect();

    let title_html = html! { 
        <div class="flex items-center justify-between">
            <span>{title}</span>
            <div class="invisible px-1.5 py-0.5 text-[10px] font-medium rounded-md shadow-lg ring-1">
                {"Placeholder"}
            </div>
        </div>
    };

    html! {
        <CardBase
            item={item}
            title={title_html}
            stats={converted_stats}
            action_label={props.action_label.clone()}
            on_action={props.on_action.clone()}
            on_click={props.on_click.clone()}
            loading={props.loading}
            error={props.error.clone()}
        />
    }
}
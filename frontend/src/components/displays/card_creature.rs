use yew::prelude::*;
use crate::models::Creature;
use super::{
    card_base::CardBase,
    DisplayItem,
    get_creature_title,
    get_creature_card_stats,
    get_creature_rarity_style,
};

#[derive(Properties, PartialEq)]
pub struct CreatureCardProps {
    pub creature: Creature,
    pub action_label: Option<String>,
    pub on_action: Option<Callback<()>>,
    pub on_click: Option<Callback<DisplayItem>>,
    pub loading: bool,
    #[prop_or_default]
    pub available_creatures: Option<Vec<Creature>>,
}

#[function_component(CreatureCard)]
pub fn creature_card(props: &CreatureCardProps) -> Html {
    let item = DisplayItem::Creature(props.creature.clone());
    let update_counter = use_state(|| 0);

    // Add interval to force updates more frequently
    {
        let update_counter = update_counter.clone();
        use_effect_with((), move |_| {
            let interval = gloo_timers::callback::Interval::new(100, move || {
                update_counter.set(*update_counter + 1);
            });
            move || drop(interval)
        });
    }

    let mut stats = get_creature_card_stats(&props.creature);
    
    // Check if energy is currently recharging and update the label
    if let Some(energy_stat) = stats.iter_mut().find(|(label, _, _, _)| *label == "Energy") {
        if props.creature.energy_recharge_complete_at.is_some() {
            // State 1: Charging
            energy_stat.0 = "Charging";
        } else if props.creature.energy_full {
            // State 2: Full Energy
            energy_stat.0 = "Ready";
        } else if energy_stat.1 <= 0.0 { // Assuming energy_stat.1 is the current value
            // State 3: Empty Energy (value is 0 or less)
            energy_stat.0 = "Empty";
        } else {
            // State 4: Default "Energy" label (already set, but good to be explicit)
            energy_stat.0 = "Energy";
        }
    }
    
    // Logic to check if Soul Bind is available
    let is_bind_available = props.available_creatures.as_ref().map_or(false, |creatures| {
        props.creature.energy_full &&
        props.creature.rarity.as_deref() != Some("Mythical") &&
        creatures.iter().any(|c| 
            c.id != props.creature.id &&
            c.rarity == props.creature.rarity && 
            c.essence == props.creature.essence
        )
    });

    // Create the overlay tag if bind is available
    let overlay_tag = if is_bind_available {
        Some(html! {
            <span class="px-2 py-0.5 text-xs font-semibold text-white bg-gradient-to-r from-purple-500 to-pink-600 rounded-md shadow">
                {"Soul Bind"}
            </span>
        })
    } else {
        None
    };

    let title = get_creature_title(&props.creature);
    
    let converted_stats: Vec<(String, f64, f64, String)> = stats
        .into_iter()
        .map(|(label, value, max, color)| 
            (label.to_string(), value, max, color.to_string()))
        .collect();

    let title_with_rarity = html! {
        <div class="flex items-center justify-between">
            <span>{title}</span>
            <div class={classes!(
                "px-1.5", 
                "py-0.5", 
                "text-[10px]", 
                "font-medium", 
                "rounded-md", 
                "shadow-lg", 
                "ring-1",
                get_creature_rarity_style(&props.creature)
            )}>
                {&props.creature.rarity.clone().unwrap_or_else(|| "Unknown".to_string())}
            </div>
        </div>
    };

    html! {
        <CardBase
            item={item}
            title={title_with_rarity}
            stats={converted_stats}
            action_label={props.action_label.clone()}
            on_action={props.on_action.clone()}
            on_click={props.on_click.clone()}
            loading={props.loading}
            overlay_tag={overlay_tag}
        />
    }
}
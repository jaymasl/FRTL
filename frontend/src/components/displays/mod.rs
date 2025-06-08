use yew::prelude::*;
use crate::models::{Creature, Egg, Scroll};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

mod base_creature;
mod base_egg;
mod card_base;
mod card_creature;
mod card_egg;
mod focus_base;
mod focus_egg;
mod focus_creature;
mod bind_modal;
mod chaos_realm_card;
mod soul_bind_button;
mod energy_manager;
mod card_scroll;
mod focus_scroll;
mod rename_creature;

pub use base_creature::*;
pub use base_egg::{get_egg_title, get_egg_stats, get_egg_card_stats, get_egg_description, get_egg_details};
pub use card_base::{CardBase, CardBaseProps, StatsBar, StatsBarProps, get_image_url};
pub use card_creature::{CreatureCard, CreatureCardProps};
pub use card_egg::{EggCard, EggCardProps};
pub use focus_base::{FocusTemplate, Props as FocusTemplateProps};
pub use focus_egg::{EggFocus, EggFocusProps};
pub use focus_creature::{CreatureFocus, CreatureFocusProps};
pub use bind_modal::{BindModal, BindModalProps};
pub use chaos_realm_card::*;
pub use soul_bind_button::{SoulBindButton, SoulBindButtonProps};
pub use energy_manager::{EnergyManager, EnergyManagerProps};
pub use card_scroll::{ScrollCard, ScrollCardProps};
pub use focus_scroll::{ScrollFocus, ScrollFocusProps};
pub use rename_creature::*;

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayMode {
    Card,
    Focus,
    Market,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DisplayItem {
    Egg(Egg),
    Creature(Creature),
    Scroll(Scroll),
}

#[derive(Properties, PartialEq)]
pub struct CardProps {
    pub item: DisplayItem,
    pub on_click: Option<Callback<DisplayItem>>,
    pub on_action: Option<Callback<()>>,
    pub action_label: Option<String>,
    pub loading: bool,
    pub error: String,
}

#[derive(Properties, PartialEq)]
pub struct DisplayProps {
    pub item: DisplayItem,
    pub mode: DisplayMode,
    pub on_click: Option<Callback<DisplayItem>>,
    pub on_close: Option<Callback<()>>,
    pub on_action: Option<Callback<()>>,
    pub action_label: Option<String>,
    pub loading: bool,
    pub error: String,
    #[prop_or_default]
    pub handle_hatch: Option<Callback<Uuid>>,
    pub fetch_data: Option<Callback<()>>,
    #[prop_or_default]
    pub on_select_egg: Option<Callback<Egg>>,
    pub on_energy_update: Option<Callback<(Uuid, bool)>>,
    #[prop_or_default]
    pub available_creatures: Option<Vec<Creature>>,
}

#[function_component(Display)]
pub fn display(props: &DisplayProps) -> Html {
    match props.mode {
        DisplayMode::Card => {
            match &props.item {
                DisplayItem::Creature(creature) => {
                    html! {
                        <CreatureCard
                            creature={creature.clone()}
                            action_label={props.action_label.clone()}
                            on_action={props.on_action.clone()}
                            on_click={props.on_click.clone()}
                            loading={props.loading}
                            available_creatures={props.available_creatures.clone()}
                        />
                    }
                }
                DisplayItem::Egg(egg) => {
                    html! {
                        <EggCard
                            egg={egg.clone()}
                            action_label={props.action_label.clone()}
                            on_action={props.on_action.clone()}
                            on_click={props.on_click.clone()}
                            loading={props.loading}
                            error={props.error.clone()}
                        />
                    }
                }
                DisplayItem::Scroll(scroll) => {
                    html! {
                        <ScrollCard
                            scroll={scroll.clone()}
                            action_label={props.action_label.clone()}
                            on_action={props.on_action.clone()}
                            on_click={props.on_click.clone()}
                            loading={props.loading}
                            error={props.error.clone()}
                        />
                    }
                }
            }
        }
        DisplayMode::Market | DisplayMode::Focus => {
            let egg_id = match &props.item {
                DisplayItem::Egg(egg) => Some(egg.id),
                _ => None,
            };
            html! {
                <FocusTemplate
                    item={props.item.clone()}
                    mode={props.mode.clone()}
                    on_close={props.on_close.clone()}
                    on_action={
                        if let (DisplayItem::Egg(_), Some(id), Some(hatch_cb)) = (&props.item, egg_id, props.handle_hatch.clone()) {
                            Some(Callback::from(move |_| hatch_cb.emit(id)))
                        } else {
                            props.on_action.clone()
                        }
                    }
                    action_label={props.action_label.clone()}
                    loading={props.loading}
                    error={props.error.clone()}
                    fetch_data={props.fetch_data.clone()}
                    on_select_egg={props.on_select_egg.clone()}
                    on_energy_update={props.on_energy_update.clone()}
                />
            }
        }
    }
}
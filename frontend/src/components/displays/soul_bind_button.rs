use yew::prelude::*;
use web_sys::MouseEvent;
use crate::models::Creature;

#[derive(Properties, PartialEq)]
pub struct SoulBindButtonProps {
    pub loading: bool,
    pub energy_full: bool,
    pub can_bind: bool,
    pub on_click: Callback<MouseEvent>,
    pub target_creature: Creature,
    pub available_creatures: Vec<Creature>,
    #[prop_or_default]
    pub is_energy_transitioning: bool,
    #[prop_or_default]
    pub in_chaos_realm: bool,
}

#[function_component(SoulBindButton)]
pub fn soul_bind_button(props: &SoulBindButtonProps) -> Html {
    let has_valid_candidates = props.available_creatures.iter().any(|c| 
        c.id != props.target_creature.id && 
        c.rarity == props.target_creature.rarity && 
        c.essence == props.target_creature.essence
    );
    
    let disabled = props.is_energy_transitioning || !props.can_bind || !props.energy_full || 
                  !has_valid_candidates || props.in_chaos_realm;
    
    // Show the button even if conditions fail, but with a different style to indicate issues
    html! {
        <button 
            onclick={props.on_click.clone()}
            disabled={props.loading || disabled}
            class={if disabled {
                "px-3 py-1 rounded-lg text-xs font-semibold text-white bg-gradient-to-r from-gray-500 to-gray-600 hover:from-gray-600 hover:to-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-300"
            } else {
                "px-3 py-1 rounded-lg text-xs font-semibold text-white bg-gradient-to-r from-purple-500 to-pink-600 hover:from-purple-600 hover:to-pink-700 disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-300"
            }}
            title={if props.in_chaos_realm {
                "Cannot bind while in Chaos Realm"
            } else if !props.energy_full {
                "Requires full energy"
            } else if !props.can_bind {
                "Mythical creatures cannot bind"
            } else if !has_valid_candidates {
                "No matching creatures found with same essence and rarity"
            } else if props.is_energy_transitioning {
                "Energy is recharging"
            } else {
                "Soul Bind"
            }}
        >
            {if props.loading { 
                {"Upgrading..."} 
            } else if !disabled { 
                {"Soul Bind Available"} 
            } else {
                {if props.in_chaos_realm {
                    "In Chaos Realm"
                } else if !props.energy_full {
                    "Needs Energy"
                } else if !props.can_bind {
                    "Cannot Bind" 
                } else if !has_valid_candidates {
                    "No Matches"
                } else {
                    "Transitioning"
                }}
            }}
        </button>
    }
} 
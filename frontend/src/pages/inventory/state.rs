use crate::models::{Egg, Creature, Scroll};
use crate::Route;
use uuid::Uuid;
use yew_router::prelude::Navigator;
use crate::components::displays::DisplayItem;
use super::filters::CollectionType;
use yew::prelude::*;

#[derive(Debug, Clone)]
pub struct HatchState {
    pub egg_id: Option<Uuid>,
    pub error: String,
    pub last_attempt: Option<f64>,
}

impl Default for HatchState {
    fn default() -> Self {
        Self {
            egg_id: None,
            error: String::new(),
            last_attempt: None,
        }
    }
}

pub fn handle_session_expired(navigator: &Navigator) {
    navigator.push(&Route::Login);
}

pub fn get_filtered_items(
    collection_type: &CollectionType,
    eggs: &UseStateHandle<Vec<Egg>>,
    creatures: &UseStateHandle<Vec<Creature>>,
    scrolls: &UseStateHandle<Vec<Scroll>>
) -> Vec<DisplayItem> {
    match collection_type {
        CollectionType::All => {
            let mut items = Vec::new();
            items.extend(eggs.iter().map(|e| DisplayItem::Egg(e.clone())));
            items.extend(creatures.iter().filter(|c| c.status == "available").map(|c| DisplayItem::Creature(c.clone())));
            items.extend(scrolls.iter().map(|s| DisplayItem::Scroll(s.clone())));
            items
        }
        CollectionType::Eggs => eggs.iter().map(|e| DisplayItem::Egg(e.clone())).collect(),
        CollectionType::Creatures => creatures.iter().filter(|c| c.status == "available").map(|c| DisplayItem::Creature(c.clone())).collect(),
        CollectionType::Scrolls => scrolls.iter().map(|s| DisplayItem::Scroll(s.clone())).collect(),
    }
}
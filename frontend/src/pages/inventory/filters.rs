use yew::prelude::*;
use crate::components::displays::DisplayItem;
#[allow(unused_imports)]
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq)]
pub enum CollectionType {
    All,
    Eggs,
    Creatures,
    Scrolls,
}

#[derive(Clone, PartialEq)]
pub enum SortCriteria {
    Default,
    Rarity,
    Energy,
    Essence,
}

impl SortCriteria {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Rarity => "Rarity",
            Self::Energy => "Energy",
            Self::Essence => "Essence",
        }
    }

    pub fn all_options() -> Vec<Self> {
        vec![
            Self::Default,
            Self::Rarity,
            Self::Energy,
            Self::Essence,
        ]
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct FilterBarProps {
    pub collection_type: CollectionType,
    pub sort_criteria: SortCriteria,
    pub sort_ascending: bool,
    pub on_collection_change: Callback<CollectionType>,
    pub on_sort_change: Callback<SortCriteria>,
    pub on_direction_change: Callback<bool>,
}

#[function_component(FilterBar)]
pub fn filter_bar(props: &FilterBarProps) -> Html {
    let collection_onchange = {
        let on_collection_change = props.on_collection_change.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let value = select.value();
                let collection_type = match value.as_str() {
                    "all" => CollectionType::All,
                    "eggs" => CollectionType::Eggs,
                    "creatures" => CollectionType::Creatures,
                    "scrolls" => CollectionType::Scrolls,
                    _ => CollectionType::All,
                };
                on_collection_change.emit(collection_type);
            }
        })
    };

    let sort_onchange = {
        let on_sort_change = props.on_sort_change.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let value = select.value();
                let sort_criteria = match value.as_str() {
                    "default" => SortCriteria::Default,
                    "rarity" => SortCriteria::Rarity,
                    "energy" => SortCriteria::Energy,
                    "essence" => SortCriteria::Essence,
                    _ => SortCriteria::Default,
                };
                on_sort_change.emit(sort_criteria);
            }
        })
    };

    let direction_onclick = {
        let on_direction_change = props.on_direction_change.clone();
        let current_direction = props.sort_ascending;
        Callback::from(move |_| {
            on_direction_change.emit(!current_direction);
        })
    };

    html! {
        <div class="flex space-x-4 items-center">
            <select
                class="bg-gray-100 dark:bg-gray-700 rounded-xl p-2 text-sm"
                onchange={collection_onchange}
            >
                <option value="all" selected={props.collection_type == CollectionType::All}>
                    {"All"}
                </option>
                <option value="eggs" selected={props.collection_type == CollectionType::Eggs}>
                    {"Eggs"}
                </option>
                <option value="creatures" selected={props.collection_type == CollectionType::Creatures}>
                    {"Creatures"}
                </option>
                <option value="scrolls" selected={props.collection_type == CollectionType::Scrolls}>
                    {"Scrolls"}
                </option>
            </select>

            <select
                class="bg-gray-100 dark:bg-gray-700 rounded-xl p-2 text-sm"
                onchange={sort_onchange}
            >
                {for SortCriteria::all_options().into_iter().map(|criteria| {
                    let value = match criteria {
                        SortCriteria::Default => "default",
                        SortCriteria::Rarity => "rarity",
                        SortCriteria::Energy => "energy",
                        SortCriteria::Essence => "essence",
                    };
                    html! {
                        <option value={value} selected={props.sort_criteria == criteria}>
                            {criteria.label()}
                        </option>
                    }
                })}
            </select>

            <button
                onclick={direction_onclick}
                class="bg-gray-100 dark:bg-gray-700 rounded-xl p-2 text-sm hover:bg-gray-200 dark:hover:bg-gray-600"
            >
                {if props.sort_ascending {
                    "↑ Ascending"
                } else {
                    "↓ Descending"
                }}
            </button>
        </div>
    }
}

pub fn sort_items(items: &mut Vec<DisplayItem>, criteria: &SortCriteria, ascending: bool) {
    match criteria {
        SortCriteria::Default => (),
        _ => {
            items.sort_by(|a, b| {
                let cmp = match criteria {
                    SortCriteria::Rarity => compare_rarity(a, b),
                    SortCriteria::Energy => compare_energy(a, b),
                    SortCriteria::Essence => compare_essence(a, b),
                    SortCriteria::Default => std::cmp::Ordering::Equal,
                };
                if ascending { cmp } else { cmp.reverse() }
            });
        }
    }
}

fn get_rarity_value(rarity: &str) -> i32 {
    match rarity.to_lowercase().as_str() {
        "mythical" => 6,
        "legendary" => 5,
        "epic" => 4,
        "rare" => 3,
        "uncommon" => 2,
        "common" => 1,
        _ => 0,
    }
}

fn compare_rarity(a: &DisplayItem, b: &DisplayItem) -> std::cmp::Ordering {
    match (a, b) {
        (DisplayItem::Creature(a), DisplayItem::Creature(b)) => {
            match (a.rarity.as_ref(), b.rarity.as_ref()) {
                (Some(a_rarity), Some(b_rarity)) => {
                    let a_value = get_rarity_value(a_rarity);
                    let b_value = get_rarity_value(b_rarity);
                    a_value.cmp(&b_value)
                },
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        },
        (DisplayItem::Creature(_), _) => std::cmp::Ordering::Greater,
        (_, DisplayItem::Creature(_)) => std::cmp::Ordering::Less,
        (DisplayItem::Egg(_), DisplayItem::Scroll(_)) => std::cmp::Ordering::Less,
        (DisplayItem::Scroll(_), DisplayItem::Egg(_)) => std::cmp::Ordering::Greater,
        (DisplayItem::Egg(_), DisplayItem::Egg(_)) => std::cmp::Ordering::Equal,
        (DisplayItem::Scroll(_), DisplayItem::Scroll(_)) => std::cmp::Ordering::Equal,
    }
}

fn compare_energy(a: &DisplayItem, b: &DisplayItem) -> std::cmp::Ordering {
    match (a, b) {
        (DisplayItem::Creature(a), DisplayItem::Creature(b)) => {
            match (a.energy_full, b.energy_full) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Equal,
            }
        },
        (DisplayItem::Creature(_), _) => std::cmp::Ordering::Less,
        (_, DisplayItem::Creature(_)) => std::cmp::Ordering::Greater,
        (DisplayItem::Egg(_), DisplayItem::Scroll(_)) => std::cmp::Ordering::Less,
        (DisplayItem::Scroll(_), DisplayItem::Egg(_)) => std::cmp::Ordering::Greater,
        (DisplayItem::Egg(_), DisplayItem::Egg(_)) => std::cmp::Ordering::Equal,
        (DisplayItem::Scroll(_), DisplayItem::Scroll(_)) => std::cmp::Ordering::Equal,
    }
}

fn compare_essence(a: &DisplayItem, b: &DisplayItem) -> std::cmp::Ordering {
    let get_essence = |item: &DisplayItem| -> Option<String> {
        match item {
            DisplayItem::Creature(c) => c.essence.clone(),
            DisplayItem::Egg(e) => e.essence.clone(),
            DisplayItem::Scroll(_) => Some("None".to_string()),
        }
    };

    match (get_essence(a), get_essence(b)) {
        (Some(a_essence), Some(b_essence)) => a_essence.cmp(&b_essence),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}
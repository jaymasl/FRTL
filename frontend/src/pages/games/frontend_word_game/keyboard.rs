use yew::prelude::*;
use shared::shared_word_game::LetterTile;
use std::collections::HashMap;

#[derive(Properties, PartialEq)]
pub struct KeyboardProps {
    pub guess_history: Vec<Vec<LetterTile>>,
    #[prop_or_default]
    pub on_key_press: Option<Callback<char>>,
    #[prop_or_default]
    pub on_backspace: Option<Callback<()>>,
    #[prop_or_default]
    pub on_enter: Option<Callback<()>>,
}

#[function_component(Keyboard)]
pub fn keyboard(props: &KeyboardProps) -> Html {
    // Create a map to track the status of each letter
    // Priority: green > yellow > gray
    let letter_status = {
        let mut status_map = HashMap::new();
        
        for tiles in &props.guess_history {
            for tile in tiles {
                let letter = tile.letter.to_ascii_lowercase();
                let current_status = status_map.get(&letter).cloned().unwrap_or_else(|| "".to_string());
                
                // Only update if the new status has higher priority
                if current_status != "green" {
                    if tile.status == "green" {
                        status_map.insert(letter, "green".to_string());
                    } else if tile.status == "yellow" && current_status != "yellow" {
                        status_map.insert(letter, "yellow".to_string());
                    } else if current_status.is_empty() {
                        status_map.insert(letter, tile.status.clone());
                    }
                }
            }
        }
        
        status_map
    };
    
    // Define keyboard rows
    let rows = vec![
        "qwertyuiop".chars().collect::<Vec<_>>(),
        "asdfghjkl".chars().collect::<Vec<_>>(),
        "zxcvbnm".chars().collect::<Vec<_>>(),
    ];
    
    html! {
        <div class="keyboard mt-6 w-full">
            {
                for rows.iter().map(|row| {
                    html! {
                        <div class="flex justify-center mb-2 w-full">
                            {
                                for row.iter().map(|&letter| {
                                    let status = letter_status.get(&letter).cloned().unwrap_or_else(|| "unused".to_string());
                                    let status_class = match status.as_str() {
                                        "green" => "bg-green-500 text-black",
                                        "yellow" => "bg-yellow-400 text-black",
                                        "gray" => "bg-red-300 dark:bg-red-400 text-black dark:text-black",
                                        _ => "bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                                    };
                                    
                                    let on_key_press = props.on_key_press.clone();
                                    let letter_clone = letter;
                                    
                                    html! {
                                        <button 
                                            type="button"
                                            class={classes!(
                                                "w-5", "h-7", "sm:w-6", "sm:h-8", "md:w-7", "md:h-9", "lg:w-8", "lg:h-10", 
                                                "flex", "items-center", "justify-center", 
                                                "text-xs", "sm:text-sm", "font-bold", 
                                                "rounded", "mx-0.5", "cursor-pointer",
                                                status_class
                                            )}
                                            onclick={
                                                if let Some(callback) = on_key_press {
                                                    let callback = callback.clone();
                                                    Some(Callback::from(move |_| {
                                                        callback.emit(letter_clone);
                                                    }))
                                                } else {
                                                    None
                                                }
                                            }
                                        >
                                            <div class="relative flex flex-col items-center justify-center h-full w-full">
                                                <div class="absolute top-1/2 transform -translate-y-1/2">
                                                    { letter.to_ascii_uppercase() }
                                                </div>
                                                <div class="absolute bottom-0.5">
                                                    {
                                                        match status.as_str() {
                                                            "green" => html! {
                                                                <div class="flex gap-0.5">
                                                                    <div class="w-1 h-1 bg-black rounded-full"></div>
                                                                    <div class="w-1 h-1 bg-black rounded-full"></div>
                                                                </div>
                                                            },
                                                            "yellow" => html! {
                                                                <div class="w-1 h-1 bg-black rounded-full"></div>
                                                            },
                                                            _ => html! {}
                                                        }
                                                    }
                                                </div>
                                            </div>
                                        </button>
                                    }
                                })
                            }
                        </div>
                    }
                })
            }
            
            // Add Enter and Backspace buttons
            <div class="flex justify-center mb-2 w-full">
                <button 
                    type="button"
                    class="px-1 py-0.5 sm:px-2 sm:py-1 md:px-3 md:py-2 bg-gray-300 dark:bg-gray-600 text-gray-900 dark:text-white rounded mx-0.5 sm:mx-1 text-xs sm:text-sm font-bold cursor-pointer"
                    onclick={
                        if let Some(callback) = &props.on_enter {
                            let callback = callback.clone();
                            Some(Callback::from(move |_| {
                                callback.emit(());
                            }))
                        } else {
                            None
                        }
                    }
                >
                    {"Enter"}
                </button>
                
                <button 
                    type="button"
                    class="px-1 py-0.5 sm:px-2 sm:py-1 md:px-3 md:py-2 bg-gray-300 dark:bg-gray-600 text-gray-900 dark:text-white rounded mx-0.5 sm:mx-1 text-xs sm:text-sm font-bold cursor-pointer"
                    onclick={
                        if let Some(callback) = &props.on_backspace {
                            let callback = callback.clone();
                            Some(Callback::from(move |_| {
                                callback.emit(());
                            }))
                        } else {
                            None
                        }
                    }
                >
                    {"⌫"}
                </button>
            </div>
        </div>
    }
} 
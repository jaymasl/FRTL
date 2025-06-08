use yew::prelude::*;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use shared::shared_word_game::DICTIONARY;

// We only need the DICTIONARY_INDEX for the lookup component
static DICTIONARY_INDEX: Lazy<HashMap<usize, Vec<&'static str>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    // Use the shared DICTIONARY instead of directly including the file
    for word in DICTIONARY.iter() {
        map.entry(word.len()).or_insert_with(Vec::new).push(word.as_str());
    }
    map
});

#[derive(Properties, PartialEq, Clone)]
pub struct DictionaryLookupProps {
    pub word_length: usize,
}

// Wrapper component that will prevent re-renders
#[function_component(DictionaryLookup)]
pub fn dictionary_lookup(props: &DictionaryLookupProps) -> Html {
    // Use a key to force the component to only re-render when word_length changes
    html! {
        <div class="mt-4" key={format!("dict-{}", props.word_length)}>
            <h3 class="text-lg font-semibold mb-4 text-center text-gray-900 dark:text-white">{"Dictionary Lookup"}</h3>
            <DictionaryContent word_length={props.word_length} />
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct DictionaryContentProps {
    word_length: usize,
}

#[function_component(DictionaryContent)]
fn dictionary_content(props: &DictionaryContentProps) -> Html {
    let dict_filter = use_state(String::new);
    
    // Store the filter value in session storage to persist across re-renders
    {
        let dict_filter = dict_filter.clone();
        use_effect_with(props.word_length, move |word_length| {
            // Try to restore the filter value from session storage
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.session_storage() {
                    let storage_key = format!("dict_filter_{}", word_length);
                    if let Ok(Some(saved_filter)) = storage.get_item(&storage_key) {
                        dict_filter.set(saved_filter);
                    }
                }
            }
            || ()
        });
    }
    
    // Save filter value to session storage when it changes
    {
        let filter_value = (*dict_filter).clone();
        let word_length = props.word_length;  // Clone the word_length before the closure
        use_effect_with(filter_value, move |filter_value| {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.session_storage() {
                    let storage_key = format!("dict_filter_{}", word_length);
                    let _ = storage.set_item(&storage_key, filter_value);
                }
            }
            || ()
        });
    }
    
    // Filter words based on user input
    let filtered_words = {
        let mut words = Vec::new();
        let dict_filter_val = (*dict_filter).clone();
        
        // Always use the game's word length
        let source_words = DICTIONARY_INDEX.get(&props.word_length).cloned().unwrap_or_default();
        
        // Only show words if there's a filter text
        if !dict_filter_val.is_empty() {
            // Filter words containing the filter text
            for &word in source_words.iter() {
                if word.contains(&dict_filter_val.to_lowercase()) {
                    words.push(word);
                }
                if words.len() >= 500 {
                    break;
                }
            }
        }
        
        words
    };
    
    let on_dict_filter_input = {
        let dict_filter = dict_filter.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            dict_filter.set(input.value());
        })
    };
    
    html! {
        <div onclick={Callback::from(|e: MouseEvent| {
            e.stop_propagation();
        })}
        onmousedown={Callback::from(|e: MouseEvent| {
            e.stop_propagation();
        })}
        >
            <div class="flex flex-col gap-2">
                <input
                    type="text"
                    placeholder="Search dictionary..."
                    value={(*dict_filter).clone()}
                    oninput={on_dict_filter_input}
                    onclick={Callback::from(|e: MouseEvent| {
                        e.stop_propagation();
                    })}
                    onmousedown={Callback::from(|e: MouseEvent| {
                        e.stop_propagation();
                    })}
                    onkeydown={Callback::from(|e: KeyboardEvent| {
                        if e.key() == "Enter" {
                            e.prevent_default();
                            e.stop_propagation();
                        }
                    })}
                    class="w-[70%] mx-auto block p-1.5 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
                <div class="w-full bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg">
                    <div class="max-h-48 overflow-y-auto p-2">
                        <div class="flex flex-col gap-1">
                            {
                                filtered_words.iter().map(|&word| {
                                    html! {
                                        <div 
                                            key={word}
                                            class="p-1.5 text-sm text-center bg-gray-100 dark:bg-gray-700 rounded text-gray-900 dark:text-white hover:bg-gray-200 dark:hover:bg-gray-600 cursor-pointer transition-colors duration-200"
                                        >
                                            {word}
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                    </div>
                </div>
            </div>
            <div class="mt-2 text-xs text-gray-500 dark:text-gray-400 text-center">
                { 
                    if filtered_words.len() >= 500 { 
                        "Showing first 500 matches".to_string()
                    } else { 
                        format!("Showing {} matches", filtered_words.len()) 
                    } 
                }
            </div>
        </div>
    }
} 
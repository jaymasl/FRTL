use yew::prelude::*;
use web_sys::{window, KeyboardEvent, HtmlInputElement, HtmlElement, FocusEvent};
use wasm_bindgen::JsCast;
use shared::shared_word_game::{PublicWordGame, LetterTile};

#[derive(Properties, PartialEq)]
pub struct GameBoardProps {
    pub game: PublicWordGame,
    pub guess_history: Vec<Vec<LetterTile>>,
    pub is_loading: bool,
    pub on_submit: Callback<()>,
    pub time_left: f64,
    pub current_guess: String,
    #[prop_or_default]
    pub on_guess_change: Option<Callback<String>>,
}

#[function_component(GameBoard)]
pub fn game_board(props: &GameBoardProps) -> Html {
    let game = props.game.clone();
    
    // Focus the first input when the component mounts
    use_effect_with(
        (),
        |_| {
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(input) = document.get_element_by_id("input-0") {
                        let _ = input.dyn_ref::<HtmlInputElement>().unwrap().focus();
                    }
                }
            }
            || ()
        },
    );
    
    // Handle form submission
    let on_submit = {
        let on_submit = props.on_submit.clone();
        
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            on_submit.emit(());
        })
    };
    
    html! {
        <div class="game-board w-full max-w-lg mx-auto">
            // Display previous guesses
            <div class="mb-6 w-full">
                {
                    for props.guess_history.iter().map(|tiles| {
                        html! {
                            <div class="flex justify-center mb-2">
                                {
                                    for tiles.iter().map(|tile| {
                                        let status_class = match tile.status.as_str() {
                                            "green" => "bg-green-500 text-black",
                                            "yellow" => "bg-yellow-400 text-black",
                                            _ => "bg-red-300 dark:bg-red-400 text-black dark:text-black"
                                        };
                                        
                                        html! {
                                            <div class="mx-0.5 sm:mx-1">
                                                <div class={classes!(
                                                    "w-12", "h-12", "sm:w-12", "sm:h-12", "md:w-14", "md:h-14", "lg:w-16", "lg:h-16", "xl:w-16", "xl:h-16",
                                                    "flex", "items-center", "justify-center", 
                                                    "text-xl", "sm:text-2xl", "md:text-2xl", "lg:text-3xl", "font-bold", 
                                                    "rounded",
                                                    status_class
                                                )}>
                                                    <div class="relative flex flex-col items-center justify-center h-full">
                                                        <div class="absolute top-1/2 transform -translate-y-1/2">
                                                            { tile.letter.to_ascii_uppercase() }
                                                        </div>
                                                        <div class="absolute bottom-1">
                                                            {
                                                                match tile.status.as_str() {
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
                                                </div>
                                            </div>
                                        }
                                    })
                                }
                            </div>
                        }
                    })
                }
            </div>
            
            // Input form for current guess
            {
                if !game.solved && (game.guesses.len() as u32) < game.allowed_guesses && props.time_left > 0.0 && !props.is_loading {
                    html! {
                        <form id="word-form" onsubmit={on_submit} class="mb-6">
                            // Add a hint about the expected word length
                            <div class="text-center mb-2 text-sm text-gray-600 dark:text-gray-400">
                                {format!("Enter a {}-letter word", game.word_length)}
                            </div>
                            <div class="flex justify-center mb-4">
                                {
                                    for (0..game.word_length).map(|i| {
                                        let guess_chars = props.current_guess.chars().collect::<Vec<_>>();
                                        let i = i;
                                        let word_length = game.word_length;
                                        
                                        let onkeydown = {
                                            let guess_chars = guess_chars.clone();
                                            let current_guess = props.current_guess.clone();
                                            let on_guess_change = props.on_guess_change.clone();
                                            
                                            Callback::from(move |e: KeyboardEvent| {
                                                if e.key() == "Backspace" {
                                                    // Always prevent default for backspace to avoid browser navigation
                                                    e.prevent_default();
                                                    
                                                    // Only handle backspace if we're not in the first box
                                                    if i > 0 {
                                                        // Clear current input
                                                        let mut chars: Vec<char> = current_guess.chars().collect();
                                                        // Ensure we have enough characters
                                                        while chars.len() <= i {
                                                            chars.push(' ');
                                                        }
                                                        chars[i] = ' ';
                                                        
                                                        // Emit the change
                                                        if let Some(callback) = &on_guess_change {
                                                            callback.emit(chars.iter().collect());
                                                        }
                                                        
                                                        // Move focus to previous input
                                                        if let Some(window) = window() {
                                                            if let Some(document) = window.document() {
                                                                if let Some(prev_input) = document.get_element_by_id(&format!("input-{}", i - 1)) {
                                                                    let _ = prev_input.dyn_ref::<HtmlInputElement>().unwrap().focus();
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else if e.key() == "Enter" {
                                                    // Handle form submission
                                                    e.prevent_default();
                                                    
                                                    // Submit form if all characters are filled
                                                    let chars = guess_chars.clone();
                                                    let guess: String = chars.iter().collect::<String>().trim().to_string();
                                                    if guess.len() == word_length {
                                                        if let Some(window) = window() {
                                                            if let Some(document) = window.document() {
                                                                if let Some(_form) = document.get_element_by_id("word-form") {
                                                                    // Manually trigger a click on the submit button
                                                                    if let Some(submit_btn) = document.query_selector("form#word-form button[type='submit']").ok().flatten() {
                                                                        let _ = submit_btn.dyn_ref::<HtmlElement>().unwrap().click();
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else if e.key() == "ArrowLeft" {
                                                    // Move to previous input
                                                    if i > 0 {
                                                        if let Some(window) = window() {
                                                            if let Some(document) = window.document() {
                                                                if let Some(prev_input) = document.get_element_by_id(&format!("input-{}", i - 1)) {
                                                                    let _ = prev_input.dyn_ref::<HtmlInputElement>().unwrap().focus();
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else if e.key() == "ArrowRight" {
                                                    // Move to next input
                                                    if i < word_length - 1 {
                                                        if let Some(window) = window() {
                                                            if let Some(document) = window.document() {
                                                                if let Some(next_input) = document.get_element_by_id(&format!("input-{}", i + 1)) {
                                                                    let _ = next_input.dyn_ref::<HtmlInputElement>().unwrap().focus();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            })
                                        };
                                        
                                        let onfocus = {
                                            Callback::from(move |e: FocusEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                // Clone window and input_id before moving into closure
                                                let window = web_sys::window().unwrap();
                                                let input_id = input.id();
                                                let window_clone = window.clone();
                                                let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                                                    if let Some(document) = window_clone.document() {
                                                        if let Some(input) = document.get_element_by_id(&input_id) {
                                                            if let Some(input) = input.dyn_ref::<HtmlInputElement>() {
                                                                let _ = input.select();
                                                            }
                                                        }
                                                    }
                                                }) as Box<dyn FnMut()>);
                                                
                                                window.set_timeout_with_callback(closure.as_ref().unchecked_ref()).unwrap();
                                                closure.forget(); // Prevent closure from being dropped
                                            })
                                        };
                                        
                                        let oninput = {
                                            // Clone all the values we need from props to avoid borrowing props directly
                                            let current_guess = props.current_guess.clone();
                                            let on_guess_change = props.on_guess_change.clone();
                                            let word_length = game.word_length;
                                            let i = i;
                                            
                                            Callback::from(move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let value = input.value().to_lowercase();
                                                
                                                // Get the current character (only take the first character if multiple are entered)
                                                if let Some(new_char) = value.chars().next().filter(|c| c.is_alphabetic()) {
                                                    // Update the current guess by replacing the character at position i
                                                    let mut chars: Vec<char> = current_guess.chars().collect();
                                                    
                                                    // Extend chars vector if needed
                                                    while chars.len() <= i {
                                                        chars.push(' ');
                                                    }
                                                    
                                                    chars[i] = new_char;
                                                    let new_guess = chars.iter().collect::<String>();
                                                    
                                                    // Update the parent component's current_guess through a callback
                                                    if let Some(callback) = &on_guess_change {
                                                        callback.emit(new_guess);
                                                    }
                                                    
                                                    // Set the input value to the uppercase version of the character
                                                    input.set_value(&new_char.to_uppercase().to_string());
                                                    
                                                    // Move focus to next input if available
                                                    if i < word_length - 1 {
                                                        if let Some(window) = window() {
                                                            if let Some(document) = window.document() {
                                                                if let Some(next_input) = document.get_element_by_id(&format!("input-{}", i + 1)) {
                                                                    let _ = next_input.dyn_ref::<HtmlInputElement>().unwrap().focus();
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    // If the input is not a letter, clear it
                                                    input.set_value("");
                                                }
                                            })
                                        };
                                        
                                        html! {
                                            <div class="mx-0.5 sm:mx-1">
                                                <input
                                                    id={format!("input-{}", i)}
                                                    type="text"
                                                    maxlength="1"
                                                    value={guess_chars.get(i).cloned().unwrap_or(' ').to_string()}
                                                    onkeydown={onkeydown}
                                                    onfocus={onfocus}
                                                    oninput={oninput}
                                                    class="w-12 h-12 sm:w-12 sm:h-12 md:w-14 md:h-14 lg:w-16 lg:h-16 xl:w-16 xl:h-16 text-center text-xl sm:text-2xl md:text-2xl lg:text-3xl font-bold rounded border-2 border-gray-300 dark:border-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
                                                />
                                            </div>
                                        }
                                    })
                                }
                            </div>
                            <div class="flex justify-center">
                                <button
                                    type="submit"
                                    class="px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white font-medium rounded-lg transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-opacity-50"
                                >
                                    {"Submit"}
                                </button>
                            </div>
                        </form>
                    }
                } else if props.is_loading {
                    html! {
                        <div class="flex justify-center items-center h-12 mb-6">
                            <div class="animate-spin rounded-full h-6 w-6 border-t-2 border-b-2 border-blue-500"></div>
                        </div>
                    }
                } else {
                    html!{}
                }
            }
        </div>
    }
} 
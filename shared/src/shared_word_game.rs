use serde::{Serialize, Deserialize};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use once_cell::sync::Lazy;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PublicWordGame {
    // The maximum number of guesses allowed
    pub allowed_guesses: u32,
    // Remaining guesses left
    pub remaining_guesses: u32,
    // List of guesses made (each guess is a String)
    pub guesses: Vec<String>,
    // Whether the puzzle has been solved
    pub solved: bool,
    // The length of the secret word (letters count), to display blank tiles without revealing the word
    pub word_length: usize,
    pub solution: Option<String>,
    // When the game was created (seconds since epoch)
    pub created_at: Option<u64>,
    // History of tile evaluations for each guess
    pub tiles_history: Vec<Vec<LetterTile>>,
}

// Optionally, define shared API types for creating a new game and for processing guesses.

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct NewWordGameResponse {
    pub session_id: String,
    pub session_signature: String,
    pub game: PublicWordGame,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LetterTile {
    pub letter: char,
    pub status: String, // "green", "yellow", or "gray"
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GuessResponse {
    // Whether the guess was correct (if the secret word was guessed)
    pub correct: bool,
    // The updated public state of the game
    pub game: PublicWordGame,
    // A message string to provide feedback
    pub message: String,
    // The letter tiles evaluation for the guess
    pub tiles: Vec<LetterTile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RefreshResponse {
    pub game: PublicWordGame,
}

// Expose the dictionary for both frontend and backend
pub static DICTIONARY: Lazy<Vec<String>> = Lazy::new(|| {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Server-side loading
        let dict_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("cleaned_valid_words.txt");
        
        let content = fs::read_to_string(&dict_path)
            .expect("Cannot read dictionary file");
        
        content
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|w| !w.is_empty())
            .collect()
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        // Client-side loading - for WASM, we'll need to include the dictionary differently
        include_str!("cleaned_valid_words.txt")
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|w| !w.is_empty())
            .collect()
    }
});

// Add a helper function to get a random word
#[cfg(not(target_arch = "wasm32"))]
pub fn get_random_word() -> String {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    DICTIONARY.choose(&mut rng)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "puzzle".to_string()) // Fallback word if dictionary is empty
} 
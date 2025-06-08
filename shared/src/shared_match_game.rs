use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ColorVariant {
    Normal,
    Shiny,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Color {
    Red,
    Blue,
    Green,
    Lime,
    Purple,
    Orange,
    Pink,
    Teal,
    Gold,  // New special color for shiny variant
    // Add more colors if needed
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Card {
    pub id: usize,
    pub color: Color,
    pub variant: ColorVariant,
    pub revealed: bool,
    pub matched: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublicCard {
    pub id: usize,
    pub color: Option<Color>,  // None if not revealed
    pub variant: Option<ColorVariant>,  // None if not revealed
    pub revealed: bool,
    pub matched: bool,
}

impl Card {
    pub fn new(id: usize, color: Color, variant: ColorVariant) -> Self {
        Self { 
            id, 
            color, 
            variant,
            revealed: false, 
            matched: false 
        }
    }

    pub fn to_public(&self) -> PublicCard {
        PublicCard {
            id: self.id,
            color: if self.revealed || self.matched { Some(self.color.clone()) } else { None },
            variant: if self.revealed || self.matched { Some(self.variant.clone()) } else { None },
            revealed: self.revealed,
            matched: self.matched,
        }
    }
}

/// Returns true if the two cards have the same color
pub fn is_match(card1: &Card, card2: &Card) -> bool {
    card1.color == card2.color
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchGame {
    pub cards: Vec<Card>,
    pub score: u32,
    // Track the last revealed non-matching pair
    pub last_reveal: Option<(usize, usize)>,
    pub last_reveal_time: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublicMatchGame {
    pub cards: Vec<PublicCard>,
    pub score: u32,
}

impl MatchGame {
    /// Initializes a new MatchGame with a given vector of cards
    pub fn new(cards: Vec<Card>) -> Self {
        Self { 
            cards, 
            score: 0,
            last_reveal: None,
            last_reveal_time: None,
        }
    }

    pub fn to_public(&self) -> PublicMatchGame {
        PublicMatchGame {
            cards: self.cards.iter().map(|card| card.to_public()).collect(),
            score: self.score,
        }
    }

    /// Reveals two cards at the provided indices and checks if they match.
    /// If they match, both cards are marked as matched and score is incremented.
    /// Returns true if it's a match, false otherwise.
    pub fn reveal_and_check(&mut self, first_index: usize, second_index: usize, current_time: u64) -> bool {
        // First, check if we need to hide any previously revealed non-matching cards
        if let (Some(last_pair), Some(last_time)) = (self.last_reveal, self.last_reveal_time) {
            if current_time.saturating_sub(last_time) >= 1 {  // 1 second has passed
                // Order the indices to safely get two mutable references
                let (i, j) = if last_pair.0 <= last_pair.1 { (last_pair.0, last_pair.1) } else { (last_pair.1, last_pair.0) };
                let (first_part, second_part) = self.cards.split_at_mut(j);
                let card1 = &mut first_part[i];
                let card2 = &mut second_part[0];
                if !card1.matched {
                    card1.revealed = false;
                }
                if !card2.matched {
                    card2.revealed = false;
                }
                self.last_reveal = None;
                self.last_reveal_time = None;
            }
        }

        if first_index >= self.cards.len() || second_index >= self.cards.len() {
            return false; // Invalid indices
        }
        
        // If either card is already matched, then this reveal is a duplicate
        if self.cards[first_index].matched || self.cards[second_index].matched {
            log::info!("Reveal called on card(s) already matched, ignoring duplicate reveal.");
            return true;
        }
        
        // Use split_at_mut to get two mutable references
        let (left, right) = if first_index < second_index {
            let (left, right) = self.cards.split_at_mut(second_index);
            (&mut left[first_index], &mut right[0])
        } else {
            let (left, right) = self.cards.split_at_mut(first_index);
            (&mut right[0], &mut left[second_index])
        };

        // Log the card states before revealing
        log::info!("Revealing cards: left={:?}, right={:?}", left, right);

        // Always reveal the cards
        left.revealed = true;
        right.revealed = true;

        let is_matching = is_match(left, right);
        if is_matching {
            left.matched = true;
            right.matched = true;
            self.score += 1;
            log::info!("Cards matched. New score: {}", self.score);
            // Clear any previous reveal state since we have a match
            self.last_reveal = None;
            self.last_reveal_time = None;
        } else {
            // Store this reveal to hide it later
            self.last_reveal = Some((first_index, second_index));
            self.last_reveal_time = Some(current_time);
            log::info!("Cards did not match. Will hide after delay.");
        }
        
        is_matching
    }

    pub fn hide_unmatched(&mut self, current_time: u64) {
        if let (Some(last_pair), Some(last_time)) = (self.last_reveal, self.last_reveal_time) {
            if current_time.saturating_sub(last_time) >= 1 {
                let (i, j) = if last_pair.0 <= last_pair.1 {
                    (last_pair.0, last_pair.1)
                } else {
                    (last_pair.1, last_pair.0)
                };
                let (first_part, second_part) = self.cards.split_at_mut(j);
                let card1 = &mut first_part[i];
                let card2 = &mut second_part[0];
                if !card1.matched {
                    card1.revealed = false;
                }
                if !card2.matched {
                    card2.revealed = false;
                }
                self.last_reveal = None;
                self.last_reveal_time = None;
            }
        }
    }
}

// === Common API types for matching game used by both Backend and Frontend ===

#[derive(Debug, Serialize, Deserialize)]
pub struct NewGameResponse {
    pub session_id: String,
    pub session_signature: String,
    pub game: PublicMatchGame,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevealRequest {
    pub session_id: String,
    pub first_index: usize,
    pub second_index: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevealResponse {
    pub match_found: bool,
    pub score: u32,
    pub game: PublicMatchGame,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevealOneResponse {
    pub match_found: bool,
    pub score: u32,
    pub game: PublicMatchGame,
} 
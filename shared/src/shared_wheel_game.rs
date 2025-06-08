use serde::{Serialize, Deserialize};

/// Represents the current state of the wheel game
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WheelGame {
    pub is_spinning: bool,
    pub last_result: Option<WheelResult>,
    pub cost_to_spin: i32,  // Cost in pax to spin the wheel (5)
}

/// Represents the result of a wheel spin
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WheelResult {
    pub is_win: bool,
    pub reward_type: Option<RewardType>,
    pub new_balance: i32,
}

/// Types of rewards that can be won
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RewardType {
    Scroll,
    BigPax,   // 100 pax (was 10 pax)
    SmallPax, // 50 pax (was 5 pax)
    TinyPax,  // 10 pax (was 1 pax, replaces LOSE condition)
    // Can be extended later with more reward types
}

// === API Types ===

#[derive(Debug, Serialize, Deserialize)]
pub struct NewWheelGameResponse {
    pub session_id: String,
    pub session_signature: String,
    pub game: WheelGame,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WheelSpinRequest {
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WheelSpinResponse {
    pub success: bool,
    pub is_win: bool,
    pub new_balance: i32,
    pub message: Option<String>,
    pub result_number: Option<f64>,
}

impl WheelGame {
    pub fn new() -> Self {
        Self {
            is_spinning: false,
            last_result: None,
            cost_to_spin: 5,
        }
    }

    pub fn start_spin(&mut self) {
        self.is_spinning = true;
        self.last_result = None;
    }

    pub fn complete_spin(&mut self, result: WheelResult) {
        self.is_spinning = false;
        self.last_result = Some(result);
    }
}

// Constants for frontend animation
pub const WHEEL_SEGMENTS: u32 = 8;  // Total number of segments in the wheel
pub const WIN_SEGMENTS: u32 = 4;    // Number of winning segments (50% chance)
pub const SPIN_DURATION_MS: u32 = 3000;  // Duration of spin animation in milliseconds
pub const MIN_SPINS: f64 = 5.0;     // Minimum number of full rotations
pub const MAX_SPINS: f64 = 8.0;     // Maximum number of full rotations 
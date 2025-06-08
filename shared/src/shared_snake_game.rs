use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum FoodType {
    Regular,    // Red square - gives 1 pax
    Scroll      // Yellow square - gives 1 scroll
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Food {
    pub position: Position,
    pub food_type: FoodType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnakeGame {
    pub snake: Vec<Position>,
    pub food: Food,
    pub direction: Direction,
    pub score: u32,
    pub grid_size: (u32, u32),
    pub game_over: bool,
    pub started: bool,
    pub new_balance: Option<f64>,
    pub scroll_collected: bool,  // Indicates if a scroll was just collected
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SnakeMessage {
    Start,
    ChangeDirection(Direction),
    Tick,
    GameOver,
    BalanceUpdate(f64),
    ScrollCollected,  // New message type for scroll collection
}

impl Direction {
    pub fn is_opposite(&self, other: &Direction) -> bool {
        matches!(
            (self, other),
            (Direction::Up, Direction::Down)
                | (Direction::Down, Direction::Up)
                | (Direction::Left, Direction::Right)
                | (Direction::Right, Direction::Left)
        )
    }
}

impl SnakeGame {
    pub fn new(grid_size: (u32, u32)) -> Self {
        let center_x = (grid_size.0 / 2) as i32;
        let center_y = (grid_size.1 / 2) as i32;
        
        Self {
            snake: vec![Position { x: center_x, y: center_y }],
            food: Self::generate_food(grid_size, &vec![Position { x: center_x, y: center_y }]),
            direction: Direction::Right,
            score: 0,
            grid_size,
            game_over: false,
            started: false,
            new_balance: None,
            scroll_collected: false,
        }
    }

    pub fn update(&mut self) -> bool {
        if self.game_over || !self.started {
            return false;
        }

        let head = self.snake[0];
        let new_head = match self.direction {
            Direction::Up => Position { x: head.x, y: head.y - 1 },
            Direction::Down => Position { x: head.x, y: head.y + 1 },
            Direction::Left => Position { x: head.x - 1, y: head.y },
            Direction::Right => Position { x: head.x + 1, y: head.y },
        };

        // Check wall collision
        if new_head.x < 0 || new_head.x >= self.grid_size.0 as i32 ||
           new_head.y < 0 || new_head.y >= self.grid_size.1 as i32 {
            self.game_over = true;
            return false;
        }

        // Check self collision
        if self.snake.contains(&new_head) {
            self.game_over = true;
            return false;
        }

        // Move snake
        self.snake.insert(0, new_head);
        
        // Check food collision
        if new_head == self.food.position {
            self.score += 1;
            self.scroll_collected = self.food.food_type == FoodType::Scroll;
            self.food = Self::generate_food(self.grid_size, &self.snake);
            true
        } else {
            self.snake.pop();
            false
        }
    }

    fn generate_food(grid_size: (u32, u32), snake: &[Position]) -> Food {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // Generate position
        let pos = loop {
            let pos = Position {
                x: rng.gen_range(0..grid_size.0 as i32),
                y: rng.gen_range(0..grid_size.1 as i32),
            };
            if !snake.contains(&pos) {
                break pos;
            }
        };

        // Get the current score (snake length - 1)
        let current_score = snake.len() as u32 - 1;

        // Calculate progressive scroll chance based on score
        let scroll_chance = if current_score < 10 {
            0.0  // No scrolls for beginners
        } else if current_score < 25 {
            0.005  // 0.5% chance for medium scores
        } else if current_score < 35 {
            0.01  // 1% chance for higher scores
        } else if current_score < 45 {
            0.015  // 1.5% chance for expert players
        } else {
            0.02  // Cap at 2% for scores 40 and above
        };

        // Determine food type based on calculated scroll chance
        let food_type = if scroll_chance > 0.0 && rng.gen_bool(scroll_chance) {
            FoodType::Scroll
        } else {
            FoodType::Regular
        };

        Food {
            position: pos,
            food_type,
        }
    }

    pub fn can_change_direction_from(&self, from_direction: Direction, new_direction: Direction) -> bool {
        !from_direction.is_opposite(&new_direction)
    }
} 
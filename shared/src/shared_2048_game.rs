use serde::{Serialize, Deserialize};
use rand::seq::SliceRandom;
use rand::Rng;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Game2048 {
    // Board is represented as a 2D vector where each cell is an Option<u32> (None means empty).
    pub board: Vec<Vec<Option<u32>>>,
    pub score: u32,
    // grid_size is given as (rows, cols)
    pub grid_size: (usize, usize),
    pub game_over: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublicGame2048 {
    pub board: Vec<Vec<Option<u32>>>,
    pub score: u32,
    pub game_over: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Game2048 {
    /// Creates a new 2048 game with a given grid size (rows, cols) and spawns two initial tiles.
    pub fn new(grid_size: (usize, usize)) -> Self {
        let mut game = Self {
            board: vec![vec![None; grid_size.1]; grid_size.0],
            score: 0,
            grid_size,
            game_over: false,
        };
        game.spawn_tile();
        game.spawn_tile();
        game
    }

    /// Returns a public representation of the game state for frontend consumption.
    pub fn to_public(&self) -> PublicGame2048 {
        PublicGame2048 {
            board: self.board.clone(),
            score: self.score,
            game_over: self.game_over,
        }
    }

    /// Returns a vector of coordinates (row, col) that are currently empty.
    /// Sorts positions to prefer non-corner positions when possible.
    fn empty_cells(&self) -> Vec<(usize, usize)> {
        let mut corner_cells = Vec::new();
        let mut non_corner_cells = Vec::new();
        
        for i in 0..self.board.len() {
            for j in 0..self.board[i].len() {
                if self.board[i][j].is_none() {
                    let pos = (i, j);
                    // Check if position is a corner
                    if (i == 0 || i == self.board.len() - 1) && 
                       (j == 0 || j == self.board[i].len() - 1) {
                        corner_cells.push(pos);
                    } else {
                        non_corner_cells.push(pos);
                    }
                }
            }
        }
        
        // Prefer non-corner cells, but include corners if necessary
        if !non_corner_cells.is_empty() {
            non_corner_cells
        } else {
            corner_cells
        }
    }

    /// Spawns a new tile with value based on the current highest tile.
    /// Higher values become more likely as the game progresses.
    pub fn spawn_tile(&mut self) {
        let empties = self.empty_cells();
        if empties.is_empty() { return; }
        
        let mut rng = rand::thread_rng();
        let &(i, j) = empties.choose(&mut rng).unwrap();
        
        // Simplified spawn logic: 90% chance for 2, 10% chance for 4
        // This matches the original 2048 game probabilities
        let value = if rng.gen_bool(0.9) { 2 } else { 4 };
        
        self.board[i][j] = Some(value);
    }

    /// Processes a move in the given direction. Returns true if the board changed.
    /// If the move is valid, a new tile is spawned and the game over state is checked.
    pub fn make_move(&mut self, direction: Direction) -> bool {
        let changed = match direction {
            Direction::Left => self.move_left(),
            Direction::Right => self.move_right(),
            Direction::Up => self.move_up(),
            Direction::Down => self.move_down(),
        };
        if changed {
            self.spawn_tile();
            self.check_game_over();
        }
        changed
    }

    fn move_left(&mut self) -> bool {
        let mut board_changed = false;
        for row in &mut self.board {
            let original = row.clone();
            let (new_row, score_inc) = Self::compress_and_merge_with_score(row);
            *row = new_row;
            self.score += score_inc;
            if *row != original {
                board_changed = true;
            }
        }
        board_changed
    }

    fn move_right(&mut self) -> bool {
        let mut board_changed = false;
        for row in &mut self.board {
            let original = row.clone();
            row.reverse();
            let (mut new_row, score_inc) = Self::compress_and_merge_with_score(row);
            new_row.reverse();
            *row = new_row;
            self.score += score_inc;
            if *row != original {
                board_changed = true;
            }
        }
        board_changed
    }

    fn move_up(&mut self) -> bool {
        let mut transposed = Self::transpose(&self.board);
        let mut board_changed = false;
        for row in &mut transposed {
            let original = row.clone();
            let (new_row, score_inc) = Self::compress_and_merge_with_score(row);
            *row = new_row;
            self.score += score_inc;
            if *row != original {
                board_changed = true;
            }
        }
        self.board = Self::transpose(&transposed);
        board_changed
    }

    fn move_down(&mut self) -> bool {
        let mut transposed = Self::transpose(&self.board);
        let mut board_changed = false;
        for row in &mut transposed {
            let original = row.clone();
            row.reverse();
            let (mut new_row, score_inc) = Self::compress_and_merge_with_score(row);
            new_row.reverse();
            *row = new_row;
            self.score += score_inc;
            if *row != original {
                board_changed = true;
            }
        }
        self.board = Self::transpose(&transposed);
        board_changed
    }

    /// Compresses a row by sliding non-empty cells, merging adjacent equal values, and returns the new row along with
    /// the score increment from any merges.
    fn compress_and_merge_with_score(row: &Vec<Option<u32>>) -> (Vec<Option<u32>>, u32) {
        let new_row: Vec<u32> = row.iter().filter_map(|&x| x).collect();
        let mut merged_row = Vec::new();
        let mut score_inc = 0;
        let mut i = 0;
        while i < new_row.len() {
            if i + 1 < new_row.len() && new_row[i] == new_row[i + 1] {
                let merged_value = new_row[i] * 2;
                merged_row.push(merged_value);
                score_inc += merged_value / 4;
                i += 2;
            } else {
                merged_row.push(new_row[i]);
                i += 1;
            }
        }
        while merged_row.len() < row.len() {
            merged_row.push(0);
        }
        let result: Vec<Option<u32>> = merged_row.into_iter()
            .map(|x| if x == 0 { None } else { Some(x) })
            .collect();
        (result, score_inc)
    }

    /// Transposes the board (rows become columns and vice versa).
    fn transpose(board: &Vec<Vec<Option<u32>>>) -> Vec<Vec<Option<u32>>> {
        if board.is_empty() { return vec![]; }
        let rows = board.len();
        let cols = board[0].len();
        let mut transposed = vec![vec![None; rows]; cols];
        for i in 0..rows {
            for j in 0..cols {
                transposed[j][i] = board[i][j];
            }
        }
        transposed
    }

    /// Checks if any moves are possible. If no moves can change the board, sets game_over to true.
    pub fn check_game_over(&mut self) {
        if !self.empty_cells().is_empty() {
            return;
        }
        for &dir in &[Direction::Left, Direction::Right, Direction::Up, Direction::Down] {
            let mut clone = self.clone();
            if clone.make_move(dir) {
                return;
            }
        }
        self.game_over = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_tile() {
        let game = Game2048::new((4, 4));
        let count = game.board.iter().flatten().filter(|&&tile| tile.is_some()).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_move_left_merge() {
        let mut game = Game2048::new((4, 4));
        game.board[0] = vec![Some(2), Some(2), None, None];
        let changed = game.move_left();
        assert!(changed);
        assert_eq!(game.board[0], vec![Some(4), None, None, None]);
        assert_eq!(game.score, 4);
    }
}
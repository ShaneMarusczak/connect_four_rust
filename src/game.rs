//! Game state and opening book

use std::collections::HashMap;

use crate::bitboard::Bitboard;
use crate::solver::TranspositionTable;

/// Opening book with pre-computed best moves for common positions
pub struct OpeningBook {
    positions: HashMap<u64, usize>,
}

impl OpeningBook {
    pub fn new() -> Self {
        let mut positions = HashMap::new();

        // Empty board - play center
        positions.insert(0, 3);

        // After opponent plays any column, we play center
        for col in 0..7 {
            let board = Bitboard::new().play(col);
            positions.insert(board.key(), 3);
        }

        // Common strong continuations
        let board = Bitboard::new().play(3).play(3);
        positions.insert(board.key(), 2);
        let board = Bitboard::new().play(3).play(3).play(2);
        positions.insert(board.key(), 4);
        let board = Bitboard::new().play(3).play(3).play(4);
        positions.insert(board.key(), 2);

        OpeningBook { positions }
    }

    pub fn lookup(&self, board: &Bitboard) -> Option<usize> {
        self.positions.get(&board.key()).copied()
    }
}

impl Default for OpeningBook {
    fn default() -> Self {
        Self::new()
    }
}

/// Game state
pub struct Game {
    pub board: Bitboard,
    pub red_turn: bool,
    pub depth: usize,
    pub tt: TranspositionTable,
    pub opening_book: OpeningBook,
}

impl Game {
    pub fn new(depth: usize) -> Self {
        Game {
            board: Bitboard::new(),
            red_turn: true,
            depth,
            tt: TranspositionTable::new(),
            opening_book: OpeningBook::new(),
        }
    }

    pub fn play(&mut self, col: usize) {
        self.board = self.board.play(col);
        self.red_turn = !self.red_turn;
    }

    pub fn can_play(&self, col: usize) -> bool {
        self.board.can_play(col)
    }

    pub fn is_game_over(&self) -> bool {
        self.board.opponent_wins() || self.board.is_full()
    }

    pub fn get_winner(&self) -> Option<&str> {
        if self.board.opponent_wins() {
            Some(if self.red_turn { "YELLOW" } else { "RED" })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opening_book() {
        let book = OpeningBook::new();
        let board = Bitboard::new();
        assert_eq!(book.lookup(&board), Some(3));
    }
}

//! Bitboard representation for Connect Four
//!
//! Uses a 64-bit integer with 7 bits per column (6 cells + 1 sentinel)
//! for extremely fast win detection via bit operations.
//!
//! Column layout (bit positions):
//!  5 12 19 26 33 40 47
//!  4 11 18 25 32 39 46
//!  3 10 17 24 31 38 45
//!  2  9 16 23 30 37 44
//!  1  8 15 22 29 36 43
//!  0  7 14 21 28 35 42

use crate::{COLS, ROWS, TOTAL_CELLS};

/// Bitboard representation of the game state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bitboard {
    /// Current player's pieces (the player about to move)
    pub current: u64,
    /// Mask of all pieces (both players)
    pub mask: u64,
    /// Number of moves played
    pub moves: u32,
}

impl Bitboard {
    pub const HEIGHT: u32 = ROWS as u32;

    pub const BOTTOM: u64 =
        (1 << 0) | (1 << 7) | (1 << 14) | (1 << 21) | (1 << 28) | (1 << 35) | (1 << 42);

    pub const BOARD_MASK: u64 = Self::BOTTOM * ((1 << Self::HEIGHT) - 1);

    pub fn new() -> Self {
        Bitboard {
            current: 0,
            mask: 0,
            moves: 0,
        }
    }

    #[inline]
    pub fn opponent(&self) -> u64 {
        self.current ^ self.mask
    }

    #[inline]
    pub fn can_play(&self, col: usize) -> bool {
        (self.mask & Self::top_mask(col)) == 0
    }

    #[inline]
    pub fn play(&self, col: usize) -> Bitboard {
        let mut new_board = *self;
        new_board.current ^= new_board.mask;
        new_board.mask |= new_board.mask + Self::bottom_mask(col);
        new_board.moves += 1;
        new_board
    }

    #[inline]
    pub fn play_move(&self, move_mask: u64) -> Bitboard {
        let mut new_board = *self;
        new_board.current ^= new_board.mask;
        new_board.mask |= move_mask;
        new_board.moves += 1;
        new_board
    }

    #[inline]
    pub fn is_winning(position: u64) -> bool {
        // Horizontal
        let mut m = position & (position >> 7);
        if m & (m >> 14) != 0 {
            return true;
        }
        // Diagonal \
        m = position & (position >> 6);
        if m & (m >> 12) != 0 {
            return true;
        }
        // Diagonal /
        m = position & (position >> 8);
        if m & (m >> 16) != 0 {
            return true;
        }
        // Vertical
        m = position & (position >> 1);
        m & (m >> 2) != 0
    }

    #[inline]
    pub fn opponent_wins(&self) -> bool {
        Self::is_winning(self.opponent())
    }

    #[inline]
    pub fn winning_positions(&self) -> u64 {
        Self::compute_winning_positions(self.current, self.mask)
    }

    #[inline]
    pub fn opponent_winning_positions(&self) -> u64 {
        Self::compute_winning_positions(self.opponent(), self.mask)
    }

    pub fn compute_winning_positions(position: u64, mask: u64) -> u64 {
        // Vertical
        let mut r = (position << 1) & (position << 2) & (position << 3);

        // Horizontal
        let mut p = (position << 7) & (position << 14);
        r |= p & (position << 21);
        r |= p & (position >> 7);
        p = (position >> 7) & (position >> 14);
        r |= p & (position >> 21);
        r |= p & (position << 7);

        // Diagonal 1
        p = (position << 6) & (position << 12);
        r |= p & (position << 18);
        r |= p & (position >> 6);
        p = (position >> 6) & (position >> 12);
        r |= p & (position >> 18);
        r |= p & (position << 6);

        // Diagonal 2
        p = (position << 8) & (position << 16);
        r |= p & (position << 24);
        r |= p & (position >> 8);
        p = (position >> 8) & (position >> 16);
        r |= p & (position >> 24);
        r |= p & (position << 8);

        r & (Self::BOARD_MASK ^ mask)
    }

    pub fn possible_non_losing_moves(&self) -> u64 {
        let possible = self.possible_moves();
        let opponent_win = self.opponent_winning_positions();
        let forced_moves = possible & opponent_win;

        if forced_moves != 0 {
            if forced_moves & (forced_moves - 1) != 0 {
                return 0;
            }
            return forced_moves;
        }

        possible & !(opponent_win >> 1)
    }

    #[inline]
    pub fn possible_moves(&self) -> u64 {
        (self.mask + Self::BOTTOM) & Self::BOARD_MASK
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.moves >= TOTAL_CELLS as u32
    }

    #[inline]
    pub fn moves_remaining(&self) -> i32 {
        TOTAL_CELLS as i32 - self.moves as i32
    }

    #[inline]
    pub fn top_mask(col: usize) -> u64 {
        1u64 << ((Self::HEIGHT as u64 - 1) + col as u64 * (Self::HEIGHT as u64 + 1))
    }

    #[inline]
    pub fn bottom_mask(col: usize) -> u64 {
        1u64 << (col as u64 * (Self::HEIGHT as u64 + 1))
    }

    #[inline]
    pub fn column_mask(col: usize) -> u64 {
        ((1u64 << Self::HEIGHT) - 1) << (col as u64 * (Self::HEIGHT as u64 + 1))
    }

    #[inline]
    pub fn key(&self) -> u64 {
        self.current + self.mask
    }

    pub fn to_array(self) -> [[u8; COLS]; ROWS] {
        let mut array = [[0u8; COLS]; ROWS];
        let opponent = self.opponent();
        let is_yellow_turn = self.moves % 2 == 1;

        for (col, col_array) in array.iter_mut().enumerate().take(COLS) {
            for row in 0..ROWS {
                let bit = 1u64 << (col * 7 + row);
                if self.current & bit != 0 {
                    col_array[ROWS - 1 - row] = if is_yellow_turn { 2 } else { 1 };
                } else if opponent & bit != 0 {
                    col_array[ROWS - 1 - row] = if is_yellow_turn { 1 } else { 2 };
                }
            }
        }
        array
    }

    pub fn move_score(&self, move_mask: u64) -> i32 {
        let new_position = self.current | move_mask;
        Self::compute_winning_positions(new_position, self.mask | move_mask).count_ones() as i32
    }
}

impl Default for Bitboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitboard_new() {
        let board = Bitboard::new();
        assert_eq!(board.current, 0);
        assert_eq!(board.mask, 0);
        assert_eq!(board.moves, 0);
    }

    #[test]
    fn test_can_play_empty_board() {
        let board = Bitboard::new();
        for col in 0..COLS {
            assert!(board.can_play(col));
        }
    }

    #[test]
    fn test_play_move() {
        let board = Bitboard::new();
        let board = board.play(3);
        assert_eq!(board.moves, 1);
        assert!(board.mask != 0);
    }

    #[test]
    fn test_column_full() {
        let mut board = Bitboard::new();
        for _ in 0..ROWS {
            assert!(board.can_play(0));
            board = board.play(0);
        }
        assert!(!board.can_play(0));
    }

    #[test]
    fn test_horizontal_win() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(6);
        board = board.play(1);
        board = board.play(6);
        board = board.play(2);
        board = board.play(6);
        board = board.play(3);

        assert!(board.opponent_wins());
    }

    #[test]
    fn test_vertical_win() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(1);
        board = board.play(0);
        board = board.play(1);
        board = board.play(0);
        board = board.play(1);
        board = board.play(0);

        assert!(board.opponent_wins());
    }

    #[test]
    fn test_diagonal_win() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(1);
        board = board.play(1);
        board = board.play(2);
        board = board.play(2);
        board = board.play(3);
        board = board.play(2);
        board = board.play(3);
        board = board.play(3);
        board = board.play(0);
        board = board.play(3);

        assert!(board.opponent_wins());
    }

    #[test]
    fn test_no_win() {
        let board = Bitboard::new();
        assert!(!board.opponent_wins());
    }
}

use std::io;

use colored::Colorize;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use to_int_and_back::to;

// Board dimensions
const ROWS: usize = 6;
const COLS: usize = 7;
const TOTAL_CELLS: usize = ROWS * COLS;

// Scores for win/loss detection
const WIN_SCORE: i32 = 1_000_000;
const DRAW_SCORE: i32 = 0;

// Transposition table size (must be power of 2 for fast modulo)
const TT_SIZE: usize = 1 << 24; // 16 million entries (~256 MB)
const TT_MASK: u64 = (TT_SIZE - 1) as u64;

// Killer move table depth
const MAX_KILLER_DEPTH: usize = 64;
const KILLERS_PER_PLY: usize = 2;

// Endgame tablebase threshold - solve perfectly when this many moves remain
const ENDGAME_THRESHOLD: i32 = 14;

// Bitboard layout for 7x6 board (using 7 bits per column for easy vertical operations)
// Column layout (bit positions):
//  5 12 19 26 33 40 47
//  4 11 18 25 32 39 46
//  3 10 17 24 31 38 45
//  2  9 16 23 30 37 44
//  1  8 15 22 29 36 43
//  0  7 14 21 28 35 42

/// Bitboard representation of the game state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Bitboard {
    current: u64,
    mask: u64,
    moves: u32,
}

impl Bitboard {
    const HEIGHT: u32 = ROWS as u32;

    const BOTTOM: u64 =
        (1 << 0) | (1 << 7) | (1 << 14) | (1 << 21) | (1 << 28) | (1 << 35) | (1 << 42);

    const BOARD_MASK: u64 = Self::BOTTOM * ((1 << Self::HEIGHT) - 1);

    fn new() -> Self {
        Bitboard {
            current: 0,
            mask: 0,
            moves: 0,
        }
    }

    #[inline]
    fn opponent(&self) -> u64 {
        self.current ^ self.mask
    }

    #[inline]
    fn can_play(&self, col: usize) -> bool {
        (self.mask & Self::top_mask(col)) == 0
    }

    #[inline]
    fn play(&self, col: usize) -> Bitboard {
        let mut new_board = *self;
        new_board.current ^= new_board.mask;
        new_board.mask |= new_board.mask + Self::bottom_mask(col);
        new_board.moves += 1;
        new_board
    }

    #[inline]
    fn play_move(&self, move_mask: u64) -> Bitboard {
        let mut new_board = *self;
        new_board.current ^= new_board.mask;
        new_board.mask |= move_mask;
        new_board.moves += 1;
        new_board
    }

    #[inline]
    fn is_winning(position: u64) -> bool {
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
    fn opponent_wins(&self) -> bool {
        Self::is_winning(self.opponent())
    }

    #[inline]
    fn winning_positions(&self) -> u64 {
        Self::compute_winning_positions(self.current, self.mask)
    }

    #[inline]
    fn opponent_winning_positions(&self) -> u64 {
        Self::compute_winning_positions(self.opponent(), self.mask)
    }

    fn compute_winning_positions(position: u64, mask: u64) -> u64 {
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

    fn possible_non_losing_moves(&self) -> u64 {
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
    fn possible_moves(&self) -> u64 {
        (self.mask + Self::BOTTOM) & Self::BOARD_MASK
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.moves >= TOTAL_CELLS as u32
    }

    #[inline]
    fn moves_remaining(&self) -> i32 {
        TOTAL_CELLS as i32 - self.moves as i32
    }

    #[inline]
    fn top_mask(col: usize) -> u64 {
        1u64 << ((Self::HEIGHT as u64 - 1) + col as u64 * (Self::HEIGHT as u64 + 1))
    }

    #[inline]
    fn bottom_mask(col: usize) -> u64 {
        1u64 << (col as u64 * (Self::HEIGHT as u64 + 1))
    }

    #[inline]
    fn column_mask(col: usize) -> u64 {
        ((1u64 << Self::HEIGHT) - 1) << (col as u64 * (Self::HEIGHT as u64 + 1))
    }

    #[inline]
    fn key(&self) -> u64 {
        self.current + self.mask
    }

    fn to_array(self) -> [[u8; COLS]; ROWS] {
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

    fn move_score(&self, move_mask: u64) -> i32 {
        let new_position = self.current | move_mask;
        Self::compute_winning_positions(new_position, self.mask | move_mask).count_ones() as i32
    }

}

// ============================================================================
// Zobrist Hashing
// ============================================================================

/// Pre-computed random numbers for Zobrist hashing
struct ZobristKeys {
    /// Random numbers for each cell and player combination
    /// keys[player][cell] where player 0/1 and cell 0-48
    keys: [[u64; 49]; 2],
}

impl ZobristKeys {
    fn new() -> Self {
        // Use a fixed seed for reproducible hashing
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF_CAFEBABE);
        let mut keys = [[0u64; 49]; 2];

        for player_keys in &mut keys {
            for cell_key in player_keys.iter_mut() {
                *cell_key = rng.gen();
            }
        }

        ZobristKeys { keys }
    }

    /// Compute hash for a board position
    fn hash(&self, board: &Bitboard) -> u64 {
        let mut hash = 0u64;
        let current = board.current;
        let opponent = board.opponent();

        for cell in 0..49 {
            let bit = 1u64 << cell;
            if current & bit != 0 {
                hash ^= self.keys[0][cell];
            } else if opponent & bit != 0 {
                hash ^= self.keys[1][cell];
            }
        }

        hash
    }
}

// ============================================================================
// Transposition Table with Fixed-Size Array
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TTFlag {
    Exact,
    LowerBound,
    UpperBound,
}

/// Packed transposition table entry (16 bytes total)
#[derive(Clone, Copy)]
struct TTEntry {
    key: u64,        // Full key for verification
    score: i16,      // Score (i16 is enough for Connect Four)
    depth: u8,       // Search depth
    flag: u8,        // TTFlag as u8
    best_move: u8,   // Best move column (0-6, or 255 for none)
    _padding: [u8; 3],
}

impl TTEntry {
    fn empty() -> Self {
        TTEntry {
            key: 0,
            score: 0,
            depth: 0,
            flag: 0,
            best_move: 255,
            _padding: [0; 3],
        }
    }

    fn flag(&self) -> TTFlag {
        match self.flag {
            0 => TTFlag::Exact,
            1 => TTFlag::LowerBound,
            _ => TTFlag::UpperBound,
        }
    }

    fn set_flag(&mut self, flag: TTFlag) {
        self.flag = match flag {
            TTFlag::Exact => 0,
            TTFlag::LowerBound => 1,
            TTFlag::UpperBound => 2,
        };
    }
}

/// Lock-free transposition table using atomic operations
struct TranspositionTable {
    /// Fixed-size array of entries
    entries: Vec<TTEntry>,
    zobrist: ZobristKeys,
}

impl TranspositionTable {
    fn new() -> Self {
        TranspositionTable {
            entries: vec![TTEntry::empty(); TT_SIZE],
            zobrist: ZobristKeys::new(),
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key & TT_MASK) as usize
    }

    fn get(&self, board: &Bitboard) -> Option<TTEntry> {
        let key = self.zobrist.hash(board);
        let idx = self.index(key);
        let entry = self.entries[idx];

        if entry.key == key {
            Some(entry)
        } else {
            None
        }
    }

    fn put(&mut self, board: &Bitboard, score: i32, flag: TTFlag, depth: u8, best_move: Option<usize>) {
        let key = self.zobrist.hash(board);
        let idx = self.index(key);

        // Always replace (simpler and often better than depth-based replacement)
        let mut entry = TTEntry::empty();
        entry.key = key;
        entry.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        entry.depth = depth;
        entry.set_flag(flag);
        entry.best_move = best_move.map(|m| m as u8).unwrap_or(255);

        self.entries[idx] = entry;
    }
}

// ============================================================================
// Killer Move Heuristic
// ============================================================================

/// Killer moves - moves that caused beta cutoffs at each ply
struct KillerTable {
    /// killers[ply][slot] = column that caused cutoff
    killers: [[u8; KILLERS_PER_PLY]; MAX_KILLER_DEPTH],
}

impl KillerTable {
    fn new() -> Self {
        KillerTable {
            killers: [[255; KILLERS_PER_PLY]; MAX_KILLER_DEPTH],
        }
    }

    /// Record a killer move
    fn record(&mut self, ply: usize, col: usize) {
        if ply >= MAX_KILLER_DEPTH {
            return;
        }

        // Don't add duplicate
        if self.killers[ply][0] == col as u8 {
            return;
        }

        // Shift and insert at front
        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = col as u8;
    }

    /// Get killer moves for a ply
    fn get_killers(&self, ply: usize) -> [Option<usize>; KILLERS_PER_PLY] {
        if ply >= MAX_KILLER_DEPTH {
            return [None; KILLERS_PER_PLY];
        }

        [
            if self.killers[ply][0] < 7 { Some(self.killers[ply][0] as usize) } else { None },
            if self.killers[ply][1] < 7 { Some(self.killers[ply][1] as usize) } else { None },
        ]
    }
}

// ============================================================================
// Endgame Tablebase (computed at runtime for late positions)
// ============================================================================

/// Perfect endgame solver for positions with few moves remaining
struct EndgameSolver;

impl EndgameSolver {
    /// Solve position perfectly with no depth limit
    fn solve(board: &Bitboard, mut alpha: i32, mut beta: i32) -> i32 {
        if board.is_full() {
            return DRAW_SCORE;
        }

        let winning = board.winning_positions();
        let possible = board.possible_moves();
        if winning & possible != 0 {
            return (board.moves_remaining() + 1) / 2;
        }

        let max_score = (board.moves_remaining() - 1) / 2;
        if beta > max_score {
            beta = max_score;
            if alpha >= beta {
                return beta;
            }
        }

        let moves = board.possible_non_losing_moves();
        if moves == 0 {
            return -board.moves_remaining() / 2;
        }

        // Try moves in center-first order
        for &col in &COLUMN_ORDER {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = moves & col_mask;
            if move_mask != 0 {
                let new_board = board.play_move(move_mask);
                let score = -Self::solve(&new_board, -beta, -alpha);

                if score > alpha {
                    alpha = score;
                }
                if alpha >= beta {
                    return alpha;
                }
            }
        }

        alpha
    }
}

// ============================================================================
// Opening Book
// ============================================================================

struct OpeningBook {
    positions: std::collections::HashMap<u64, usize>,
}

impl OpeningBook {
    fn new() -> Self {
        let mut positions = std::collections::HashMap::new();

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

    fn lookup(&self, board: &Bitboard) -> Option<usize> {
        self.positions.get(&board.key()).copied()
    }
}

// ============================================================================
// Column order for move exploration
// ============================================================================

const COLUMN_ORDER: [usize; 7] = [3, 2, 4, 1, 5, 0, 6];

// ============================================================================
// Parallel Solver using Lazy SMP
// ============================================================================

/// Single-threaded solver for use within parallel search
struct Solver<'a> {
    tt: &'a mut TranspositionTable,
    killers: KillerTable,
    nodes_explored: u64,
}

impl<'a> Solver<'a> {
    fn new(tt: &'a mut TranspositionTable) -> Self {
        Solver {
            tt,
            killers: KillerTable::new(),
            nodes_explored: 0,
        }
    }

    /// Negamax with alpha-beta, killer moves, and TT
    fn negamax(&mut self, board: &Bitboard, mut alpha: i32, mut beta: i32, depth: u8, ply: usize) -> i32 {
        self.nodes_explored += 1;

        // Use endgame solver for late positions
        if board.moves_remaining() <= ENDGAME_THRESHOLD && depth > 0 {
            return EndgameSolver::solve(board, alpha, beta);
        }

        if board.is_full() {
            return DRAW_SCORE;
        }

        let winning = board.winning_positions();
        let possible = board.possible_moves();
        if winning & possible != 0 {
            return (board.moves_remaining() + 1) / 2;
        }

        let max_score = (board.moves_remaining() - 1) / 2;
        if beta > max_score {
            beta = max_score;
            if alpha >= beta {
                return beta;
            }
        }

        // TT lookup
        let mut tt_move: Option<usize> = None;
        if let Some(entry) = self.tt.get(board) {
            if entry.depth >= depth {
                match entry.flag() {
                    TTFlag::Exact => return entry.score as i32,
                    TTFlag::LowerBound => {
                        if entry.score as i32 >= beta {
                            return entry.score as i32;
                        }
                        alpha = alpha.max(entry.score as i32);
                    }
                    TTFlag::UpperBound => {
                        if (entry.score as i32) <= alpha {
                            return entry.score as i32;
                        }
                        beta = beta.min(entry.score as i32);
                    }
                }
            }
            if entry.best_move < 7 {
                tt_move = Some(entry.best_move as usize);
            }
        }

        if depth == 0 {
            return self.evaluate(board);
        }

        let moves = board.possible_non_losing_moves();
        if moves == 0 {
            return -board.moves_remaining() / 2;
        }

        // Build move list with ordering: TT move, killer moves, then by score
        let mut move_list: Vec<(usize, i32)> = Vec::with_capacity(7);
        let killers = self.killers.get_killers(ply);

        for &col in &COLUMN_ORDER {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = moves & col_mask;
            if move_mask != 0 {
                let mut priority = board.move_score(move_mask);

                // Boost TT move
                if tt_move == Some(col) {
                    priority += 10000;
                }
                // Boost killer moves
                else if killers[0] == Some(col) {
                    priority += 5000;
                } else if killers[1] == Some(col) {
                    priority += 4000;
                }

                move_list.push((col, priority));
            }
        }

        move_list.sort_by(|a, b| b.1.cmp(&a.1));

        let mut best_score = i32::MIN;
        let mut best_move = None;
        let orig_alpha = alpha;
        let mut first_move = true;

        for (col, _) in move_list {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = moves & col_mask;
            let new_board = board.play_move(move_mask);

            let score = if first_move {
                -self.negamax(&new_board, -beta, -alpha, depth - 1, ply + 1)
            } else {
                // PVS: null window search first
                let mut score = -self.negamax(&new_board, -alpha - 1, -alpha, depth - 1, ply + 1);
                if score > alpha && score < beta {
                    score = -self.negamax(&new_board, -beta, -alpha, depth - 1, ply + 1);
                }
                score
            };
            first_move = false;

            if score > best_score {
                best_score = score;
                best_move = Some(col);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                // Record killer move on cutoff
                self.killers.record(ply, col);
                break;
            }
        }

        if best_score == i32::MIN {
            best_score = -board.moves_remaining() / 2;
        }

        // Store in TT
        let flag = if best_score <= orig_alpha {
            TTFlag::UpperBound
        } else if best_score >= beta {
            TTFlag::LowerBound
        } else {
            TTFlag::Exact
        };
        self.tt.put(board, best_score, flag, depth, best_move);

        best_score
    }

    /// Enhanced static evaluation with multiple heuristics
    fn evaluate(&self, board: &Bitboard) -> i32 {
        let current = board.current;
        let opponent = board.opponent();
        let empty = Bitboard::BOARD_MASK ^ board.mask;

        // 1. Immediate winning positions (very high value)
        let current_wins = Bitboard::compute_winning_positions(current, board.mask);
        let opponent_wins = Bitboard::compute_winning_positions(opponent, board.mask);

        let current_win_count = current_wins.count_ones() as i32;
        let opponent_win_count = opponent_wins.count_ones() as i32;

        // 2. Three-in-a-row threats (high value)
        let current_threats = Self::count_threats(current, board.mask);
        let opponent_threats = Self::count_threats(opponent, board.mask);

        // 3. Two-in-a-row with potential (medium value)
        let current_pairs = Self::count_open_pairs(current, empty);
        let opponent_pairs = Self::count_open_pairs(opponent, empty);

        // 4. Center control bonus (center column is most valuable)
        let center_control = Self::center_control_score(current, opponent);

        // 5. Odd/Even threat analysis (critical in Connect Four)
        // Threats on odd rows are more valuable for first player
        let odd_even_score = Self::odd_even_analysis(current_wins, opponent_wins, board.moves);

        // 6. Connectivity bonus - pieces that connect to others are stronger
        let current_connectivity = Self::connectivity_score(current);
        let opponent_connectivity = Self::connectivity_score(opponent);

        // Combine all factors with appropriate weights
        let win_diff = (current_win_count - opponent_win_count) * 100;
        let threat_diff = (current_threats - opponent_threats) * 30;
        let pair_diff = (current_pairs - opponent_pairs) * 10;
        let connectivity_diff = (current_connectivity - opponent_connectivity) * 5;

        win_diff + threat_diff + pair_diff + center_control + odd_even_score + connectivity_diff
    }

    /// Count three-in-a-row patterns with one empty slot
    fn count_threats(position: u64, mask: u64) -> i32 {
        let empty = Bitboard::BOARD_MASK ^ mask;
        let mut count = 0i32;

        // Horizontal: XXX_ and _XXX
        let h1 = position & (position >> 7);
        let h2 = h1 & (position >> 14);
        count += (h2 & (empty >> 21)).count_ones() as i32;
        count += ((position >> 21) & (h1 >> 7) & empty).count_ones() as i32;

        // Horizontal: XX_X and X_XX (gaps in the middle)
        let h_gap1 = position & (position >> 7) & (position >> 21);
        count += (h_gap1 & (empty >> 14)).count_ones() as i32;
        let h_gap2 = position & (position >> 14) & (position >> 21);
        count += (h_gap2 & (empty >> 7)).count_ones() as i32;

        // Vertical
        let v1 = position & (position >> 1);
        let v2 = v1 & (position >> 2);
        count += (v2 & (empty >> 3)).count_ones() as i32;

        // Diagonal \
        let d1 = position & (position >> 6);
        let d2 = d1 & (position >> 12);
        count += (d2 & (empty >> 18)).count_ones() as i32;
        count += ((position >> 18) & (d1 >> 6) & empty).count_ones() as i32;

        // Diagonal /
        let d3 = position & (position >> 8);
        let d4 = d3 & (position >> 16);
        count += (d4 & (empty >> 24)).count_ones() as i32;
        count += ((position >> 24) & (d3 >> 8) & empty).count_ones() as i32;

        count
    }

    /// Count two-in-a-row patterns with open ends
    fn count_open_pairs(position: u64, empty: u64) -> i32 {
        let mut count = 0i32;

        // Horizontal pairs with space to grow
        let h_pair = position & (position >> 7);
        count += (h_pair & (empty >> 14) & (empty << 7)).count_ones() as i32;

        // Vertical pairs
        let v_pair = position & (position >> 1);
        count += (v_pair & (empty >> 2)).count_ones() as i32;

        // Diagonal pairs
        let d1_pair = position & (position >> 6);
        count += (d1_pair & (empty >> 12) & (empty << 6)).count_ones() as i32;

        let d2_pair = position & (position >> 8);
        count += (d2_pair & (empty >> 16) & (empty << 8)).count_ones() as i32;

        count
    }

    /// Calculate center control bonus
    fn center_control_score(current: u64, opponent: u64) -> i32 {
        // Column weights: edges are worst, center is best
        const COL_WEIGHTS: [i32; 7] = [1, 2, 3, 4, 3, 2, 1];

        let mut score = 0i32;

        for (col, &weight) in COL_WEIGHTS.iter().enumerate() {
            let col_mask = Bitboard::column_mask(col);
            let current_in_col = (current & col_mask).count_ones() as i32;
            let opponent_in_col = (opponent & col_mask).count_ones() as i32;
            score += (current_in_col - opponent_in_col) * weight;
        }

        score * 3 // Weight for center control
    }

    /// Analyze odd/even row threats
    /// In Connect Four, the first player wins threats on odd rows (1, 3, 5)
    /// Second player wins threats on even rows (0, 2, 4)
    fn odd_even_analysis(current_wins: u64, opponent_wins: u64, moves: u32) -> i32 {
        let is_first_player = moves.is_multiple_of(2);

        // Odd row mask (rows 1, 3, 5 = bits 1, 3, 5 in each column)
        let odd_mask: u64 = 0x2A | (0x2A << 7) | (0x2A << 14) | (0x2A << 21) | (0x2A << 28) | (0x2A << 35) | (0x2A << 42);
        let even_mask: u64 = Bitboard::BOARD_MASK & !odd_mask;

        let current_odd = (current_wins & odd_mask).count_ones() as i32;
        let current_even = (current_wins & even_mask).count_ones() as i32;
        let opponent_odd = (opponent_wins & odd_mask).count_ones() as i32;
        let opponent_even = (opponent_wins & even_mask).count_ones() as i32;

        if is_first_player {
            // First player benefits from odd-row threats
            (current_odd * 15 + current_even * 5) - (opponent_odd * 5 + opponent_even * 15)
        } else {
            // Second player benefits from even-row threats
            (current_even * 15 + current_odd * 5) - (opponent_even * 5 + opponent_odd * 15)
        }
    }

    /// Score based on piece connectivity (pieces adjacent to other pieces)
    fn connectivity_score(position: u64) -> i32 {
        let mut score = 0i32;

        // Count horizontal connections
        score += (position & (position >> 7)).count_ones() as i32;

        // Count vertical connections
        score += (position & (position >> 1)).count_ones() as i32;

        // Count diagonal connections
        score += (position & (position >> 6)).count_ones() as i32;
        score += (position & (position >> 8)).count_ones() as i32;

        score
    }

    /// Find best move with iterative deepening
    fn find_best_move(&mut self, board: &Bitboard, max_depth: usize) -> (usize, i32) {
        self.nodes_explored = 0;

        // Check for immediate wins
        let winning = board.winning_positions();
        let possible = board.possible_moves();

        if winning & possible != 0 {
            for col in COLUMN_ORDER {
                let col_mask = Bitboard::column_mask(col);
                if winning & possible & col_mask != 0 {
                    return (col, (board.moves_remaining() + 1) / 2);
                }
            }
        }

        let safe_moves = board.possible_non_losing_moves();
        if safe_moves == 0 {
            for col in COLUMN_ORDER {
                if board.can_play(col) {
                    return (col, -WIN_SCORE);
                }
            }
        }

        let mut best_col = 3;
        let mut best_score = i32::MIN;

        // Iterative deepening
        for depth in 1..=max_depth {
            let mut alpha = -WIN_SCORE;
            let beta = WIN_SCORE;

            let mut move_scores: Vec<(usize, i32)> = COLUMN_ORDER
                .iter()
                .filter_map(|&col| {
                    let col_mask = Bitboard::column_mask(col);
                    let move_mask = safe_moves & col_mask;
                    if move_mask != 0 {
                        Some((col, board.move_score(move_mask)))
                    } else {
                        None
                    }
                })
                .collect();

            move_scores.sort_by(|a, b| b.1.cmp(&a.1));

            for (col, _) in &move_scores {
                let col_mask = Bitboard::column_mask(*col);
                let move_mask = safe_moves & col_mask;
                let new_board = board.play_move(move_mask);

                let score = -self.negamax(&new_board, -beta, -alpha, depth as u8, 1);

                if score > best_score || (score == best_score && *col == 3) {
                    best_score = score;
                    best_col = *col;
                }
                if score > alpha {
                    alpha = score;
                }
            }

            // Early exit on forced win
            if best_score >= (board.moves_remaining() - depth as i32) / 2 {
                break;
            }
        }

        (best_col, best_score)
    }
}

/// Parallel search using Lazy SMP (multiple threads searching same tree)
fn parallel_find_best_move(
    board: &Bitboard,
    tt: &mut TranspositionTable,
    max_depth: usize,
) -> (usize, i32) {
    // For low depths or few moves remaining, use single-threaded search
    if max_depth <= 8 || board.moves_remaining() <= ENDGAME_THRESHOLD {
        let mut solver = Solver::new(tt);
        return solver.find_best_move(board, max_depth);
    }

    // Check for immediate wins first
    let winning = board.winning_positions();
    let possible = board.possible_moves();

    if winning & possible != 0 {
        for col in COLUMN_ORDER {
            let col_mask = Bitboard::column_mask(col);
            if winning & possible & col_mask != 0 {
                return (col, (board.moves_remaining() + 1) / 2);
            }
        }
    }

    let safe_moves = board.possible_non_losing_moves();
    if safe_moves == 0 {
        for col in COLUMN_ORDER {
            if board.can_play(col) {
                return (col, -WIN_SCORE);
            }
        }
    }

    // Collect available moves
    let moves: Vec<usize> = COLUMN_ORDER
        .iter()
        .filter(|&&col| {
            let col_mask = Bitboard::column_mask(col);
            safe_moves & col_mask != 0
        })
        .copied()
        .collect();

    if moves.len() == 1 {
        return (moves[0], 0);
    }

    // Parallel search over root moves using Lazy SMP
    let board_copy = *board;
    let results: Vec<(usize, i32)> = moves
        .par_iter()
        .map(|&col| {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = safe_moves & col_mask;
            let new_board = board_copy.play_move(move_mask);

            // Each thread gets its own TT (Lazy SMP style)
            let mut local_tt = TranspositionTable::new();
            let mut solver = Solver::new(&mut local_tt);

            let score = -solver.negamax(&new_board, -WIN_SCORE, WIN_SCORE, max_depth as u8, 1);
            (col, score)
        })
        .collect();

    // Find best result
    let mut final_best_col = 3;
    let mut final_best_score = i32::MIN;

    for (col, score) in results {
        if score > final_best_score || (score == final_best_score && col == 3) {
            final_best_score = score;
            final_best_col = col;
        }
    }

    // Update TT with best move info
    tt.put(board, final_best_score, TTFlag::Exact, max_depth as u8, Some(final_best_col));

    (final_best_col, final_best_score)
}

// ============================================================================
// Game State
// ============================================================================

struct Game {
    board: Bitboard,
    red_turn: bool,
    depth: usize,
    tt: TranspositionTable,
    opening_book: OpeningBook,
}

impl Game {
    fn new(depth: usize) -> Self {
        Game {
            board: Bitboard::new(),
            red_turn: true,
            depth,
            tt: TranspositionTable::new(),
            opening_book: OpeningBook::new(),
        }
    }

    fn play(&mut self, col: usize) {
        self.board = self.board.play(col);
        self.red_turn = !self.red_turn;
    }

    fn can_play(&self, col: usize) -> bool {
        self.board.can_play(col)
    }

    fn is_game_over(&self) -> bool {
        self.board.opponent_wins() || self.board.is_full()
    }

    fn get_winner(&self) -> Option<&str> {
        if self.board.opponent_wins() {
            Some(if self.red_turn { "YELLOW" } else { "RED" })
        } else {
            None
        }
    }
}

// ============================================================================
// Main and UI
// ============================================================================

fn main() {
    intro();

    let depth = get_difficulty();
    let mut game = Game::new(depth);

    print_board(game.board);

    loop {
        let col = get_human_move(&game);
        game.play(col);
        print_board(game.board);

        if game.is_game_over() {
            print_result(&game);
            break;
        }

        println!("AI is thinking...");

        let (ai_col, score) = if let Some(book_move) = game.opening_book.lookup(&game.board) {
            (book_move, 0)
        } else {
            parallel_find_best_move(&game.board, &mut game.tt, game.depth)
        };

        game.play(ai_col);
        println!(
            "AI plays column {} (eval: {})",
            ai_col + 1,
            if score > 0 { format!("+{score}") } else { score.to_string() }
        );
        print_board(game.board);

        if game.is_game_over() {
            print_result(&game);
            break;
        }
    }
}

fn intro() {
    print!("{}", "\n\nO".red());
    print!("{}", "O".red());
    print!("{}", "O".red());
    print!("{}", "O".red());
    print!("   ");
    print!("Connect Four AI");
    print!("   ");
    print!("{}", "O".yellow());
    print!("{}", "O".yellow());
    print!("{}", "O".yellow());
    print!("{}", "O\n\n".yellow());
    println!("Features: Bitboard | Transposition Table | Killer Moves | Parallel Search | Endgame Solver\n");
}

fn get_difficulty() -> usize {
    loop {
        println!("Difficulty?");
        println!("Valid values: easy, medium, hard, vhard, expert, impossible");

        let mut dif = String::new();
        io::stdin()
            .read_line(&mut dif)
            .expect("Failed to read line");

        match dif.trim().to_lowercase().as_str() {
            "easy" => return 8,
            "medium" => return 12,
            "hard" => return 16,
            "vhard" => return 20,
            "expert" => return 28,
            "impossible" => return 42,
            _ => continue,
        }
    }
}

fn get_human_move(game: &Game) -> usize {
    loop {
        println!("Which column to play token? (1-7)");
        let mut col_input = String::new();

        io::stdin()
            .read_line(&mut col_input)
            .expect("Failed to read line");

        let col: usize = match col_input.trim().parse() {
            Ok(num) => num,
            Err(_) => match to::int(col_input.trim()) {
                Ok(num) => num as usize,
                Err(e) => {
                    println!("{e}");
                    continue;
                }
            },
        };

        if (1..=COLS).contains(&col) && game.can_play(col - 1) {
            return col - 1;
        }
        println!("Invalid column. Please choose 1-7.");
        print_board(game.board);
    }
}

fn print_board(board: Bitboard) {
    let array = board.to_array();
    println!("___________________________________");
    for row in array {
        for cell in row {
            match cell {
                0 => print!("|   |"),
                1 => {
                    print!("{}", "| ".white());
                    print!("{}", "◯".red());
                    print!("{}", " |".white());
                }
                2 => {
                    print!("{}", "| ".white());
                    print!("{}", "◯".yellow());
                    print!("{}", " |".white());
                }
                _ => {}
            }
        }
        println!();
    }
    println!("¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯");
    println!("  1    2    3    4    5    6    7 ");
}

fn print_result(game: &Game) {
    match game.get_winner() {
        Some(winner) => println!("{winner} WINS!"),
        None => println!("DRAW!"),
    }
}

// ============================================================================
// Tests
// ============================================================================

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

    #[test]
    fn test_solver_finds_winning_move() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(6);
        board = board.play(1);
        board = board.play(6);
        board = board.play(2);
        board = board.play(6);

        let mut tt = TranspositionTable::new();
        let mut solver = Solver::new(&mut tt);
        let (best_col, _) = solver.find_best_move(&board, 10);
        assert_eq!(best_col, 3);
    }

    #[test]
    fn test_solver_blocks_opponent_win() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(6);
        board = board.play(1);
        board = board.play(6);
        board = board.play(2);

        let mut tt = TranspositionTable::new();
        let mut solver = Solver::new(&mut tt);
        let (best_col, _) = solver.find_best_move(&board, 10);
        assert_eq!(best_col, 3);
    }

    #[test]
    fn test_transposition_table() {
        let mut tt = TranspositionTable::new();
        let board = Bitboard::new().play(3);
        tt.put(&board, 5, TTFlag::Exact, 10, Some(3));

        let entry = tt.get(&board);
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.score, 5);
        assert_eq!(e.flag(), TTFlag::Exact);
        assert_eq!(e.best_move, 3);
    }

    #[test]
    fn test_killer_moves() {
        let mut killers = KillerTable::new();

        killers.record(5, 2);
        let k = killers.get_killers(5);
        assert_eq!(k[0], Some(2));
        assert_eq!(k[1], None);

        killers.record(5, 4);
        let k = killers.get_killers(5);
        assert_eq!(k[0], Some(4));
        assert_eq!(k[1], Some(2)); // Still a killer (in slot 1 now)
    }

    #[test]
    fn test_endgame_solver() {
        // Create a position close to the end
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(6);
        board = board.play(1);
        board = board.play(6);
        board = board.play(2);
        board = board.play(6);

        // Should find winning move
        let score = EndgameSolver::solve(&board, -WIN_SCORE, WIN_SCORE);
        assert!(score > 0); // P1 can win
    }

    #[test]
    fn test_zobrist_hashing() {
        let zobrist = ZobristKeys::new();
        let board1 = Bitboard::new().play(3);
        let board2 = Bitboard::new().play(3);
        let board3 = Bitboard::new().play(2);

        assert_eq!(zobrist.hash(&board1), zobrist.hash(&board2));
        assert_ne!(zobrist.hash(&board1), zobrist.hash(&board3));
    }

    #[test]
    fn test_opening_book() {
        let book = OpeningBook::new();
        let board = Bitboard::new();
        assert_eq!(book.lookup(&board), Some(3));
    }

    #[test]
    fn test_possible_non_losing_moves() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(3);
        board = board.play(1);
        board = board.play(3);
        board = board.play(6);
        board = board.play(3);

        let non_losing = board.possible_non_losing_moves();
        let col3_mask = Bitboard::column_mask(3) & board.possible_moves();
        assert_eq!(non_losing, col3_mask);
    }

}

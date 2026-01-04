//! AI solver for Connect Four
//!
//! Implements negamax with alpha-beta pruning, transposition tables,
//! killer move heuristic, and parallel search using Lazy SMP.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use crate::bitboard::Bitboard;
use crate::{
    DRAW_SCORE, ENDGAME_THRESHOLD, KILLERS_PER_PLY, MAX_KILLER_DEPTH, TT_MASK, TT_SIZE, WIN_SCORE,
};

/// Column order for move exploration (center-first for better pruning)
pub const COLUMN_ORDER: [usize; 7] = [3, 2, 4, 1, 5, 0, 6];

// ============================================================================
// Zobrist Hashing
// ============================================================================

/// Pre-computed random numbers for Zobrist hashing
struct ZobristKeys {
    keys: [[u64; 49]; 2],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF_CAFEBABE);
        let mut keys = [[0u64; 49]; 2];

        for player_keys in &mut keys {
            for cell_key in player_keys.iter_mut() {
                *cell_key = rng.gen();
            }
        }

        ZobristKeys { keys }
    }

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
// Transposition Table
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TTFlag {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy)]
pub struct TTEntry {
    key: u64,
    pub score: i16,
    pub depth: u8,
    flag: u8,
    pub best_move: u8,
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

    pub fn flag(&self) -> TTFlag {
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

/// Fixed-size transposition table
pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    zobrist: ZobristKeys,
}

impl TranspositionTable {
    pub fn new() -> Self {
        TranspositionTable {
            entries: vec![TTEntry::empty(); TT_SIZE],
            zobrist: ZobristKeys::new(),
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key & TT_MASK) as usize
    }

    pub fn get(&self, board: &Bitboard) -> Option<TTEntry> {
        let key = self.zobrist.hash(board);
        let idx = self.index(key);
        let entry = self.entries[idx];

        if entry.key == key {
            Some(entry)
        } else {
            None
        }
    }

    pub fn put(
        &mut self,
        board: &Bitboard,
        score: i32,
        flag: TTFlag,
        depth: u8,
        best_move: Option<usize>,
    ) {
        let key = self.zobrist.hash(board);
        let idx = self.index(key);

        let mut entry = TTEntry::empty();
        entry.key = key;
        entry.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        entry.depth = depth;
        entry.set_flag(flag);
        entry.best_move = best_move.map(|m| m as u8).unwrap_or(255);

        self.entries[idx] = entry;
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Killer Move Heuristic
// ============================================================================

struct KillerTable {
    killers: [[u8; KILLERS_PER_PLY]; MAX_KILLER_DEPTH],
}

impl KillerTable {
    fn new() -> Self {
        KillerTable {
            killers: [[255; KILLERS_PER_PLY]; MAX_KILLER_DEPTH],
        }
    }

    fn record(&mut self, ply: usize, col: usize) {
        if ply >= MAX_KILLER_DEPTH {
            return;
        }

        if self.killers[ply][0] == col as u8 {
            return;
        }

        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = col as u8;
    }

    fn get_killers(&self, ply: usize) -> [Option<usize>; KILLERS_PER_PLY] {
        if ply >= MAX_KILLER_DEPTH {
            return [None; KILLERS_PER_PLY];
        }

        [
            if self.killers[ply][0] < 7 {
                Some(self.killers[ply][0] as usize)
            } else {
                None
            },
            if self.killers[ply][1] < 7 {
                Some(self.killers[ply][1] as usize)
            } else {
                None
            },
        ]
    }
}

// ============================================================================
// Endgame Solver
// ============================================================================

/// Perfect endgame solver for positions with few moves remaining
pub struct EndgameSolver;

impl EndgameSolver {
    pub fn solve(board: &Bitboard, mut alpha: i32, mut beta: i32) -> i32 {
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
// Solver
// ============================================================================

/// Single-threaded negamax solver
pub struct Solver<'a> {
    tt: &'a mut TranspositionTable,
    killers: KillerTable,
    nodes_explored: u64,
}

impl<'a> Solver<'a> {
    pub fn new(tt: &'a mut TranspositionTable) -> Self {
        Solver {
            tt,
            killers: KillerTable::new(),
            nodes_explored: 0,
        }
    }

    pub fn negamax(
        &mut self,
        board: &Bitboard,
        mut alpha: i32,
        mut beta: i32,
        depth: u8,
        ply: usize,
    ) -> i32 {
        self.nodes_explored += 1;

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

        let mut move_list: Vec<(usize, i32)> = Vec::with_capacity(7);
        let killers = self.killers.get_killers(ply);

        for &col in &COLUMN_ORDER {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = moves & col_mask;
            if move_mask != 0 {
                let mut priority = board.move_score(move_mask);

                if tt_move == Some(col) {
                    priority += 10000;
                } else if killers[0] == Some(col) {
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
                self.killers.record(ply, col);
                break;
            }
        }

        if best_score == i32::MIN {
            best_score = -board.moves_remaining() / 2;
        }

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

    fn evaluate(&self, board: &Bitboard) -> i32 {
        let current = board.current;
        let opponent = board.opponent();
        let empty = Bitboard::BOARD_MASK ^ board.mask;

        let current_wins = Bitboard::compute_winning_positions(current, board.mask);
        let opponent_wins = Bitboard::compute_winning_positions(opponent, board.mask);

        let current_win_count = current_wins.count_ones() as i32;
        let opponent_win_count = opponent_wins.count_ones() as i32;

        let current_threats = Self::count_threats(current, board.mask);
        let opponent_threats = Self::count_threats(opponent, board.mask);

        let current_pairs = Self::count_open_pairs(current, empty);
        let opponent_pairs = Self::count_open_pairs(opponent, empty);

        let center_control = Self::center_control_score(current, opponent);
        let odd_even_score = Self::odd_even_analysis(current_wins, opponent_wins, board.moves);

        let current_connectivity = Self::connectivity_score(current);
        let opponent_connectivity = Self::connectivity_score(opponent);

        let win_diff = (current_win_count - opponent_win_count) * 100;
        let threat_diff = (current_threats - opponent_threats) * 30;
        let pair_diff = (current_pairs - opponent_pairs) * 10;
        let connectivity_diff = (current_connectivity - opponent_connectivity) * 5;

        win_diff + threat_diff + pair_diff + center_control + odd_even_score + connectivity_diff
    }

    fn count_threats(position: u64, mask: u64) -> i32 {
        let empty = Bitboard::BOARD_MASK ^ mask;
        let mut count = 0i32;

        let h1 = position & (position >> 7);
        let h2 = h1 & (position >> 14);
        count += (h2 & (empty >> 21)).count_ones() as i32;
        count += ((position >> 21) & (h1 >> 7) & empty).count_ones() as i32;

        let h_gap1 = position & (position >> 7) & (position >> 21);
        count += (h_gap1 & (empty >> 14)).count_ones() as i32;
        let h_gap2 = position & (position >> 14) & (position >> 21);
        count += (h_gap2 & (empty >> 7)).count_ones() as i32;

        let v1 = position & (position >> 1);
        let v2 = v1 & (position >> 2);
        count += (v2 & (empty >> 3)).count_ones() as i32;

        let d1 = position & (position >> 6);
        let d2 = d1 & (position >> 12);
        count += (d2 & (empty >> 18)).count_ones() as i32;
        count += ((position >> 18) & (d1 >> 6) & empty).count_ones() as i32;

        let d3 = position & (position >> 8);
        let d4 = d3 & (position >> 16);
        count += (d4 & (empty >> 24)).count_ones() as i32;
        count += ((position >> 24) & (d3 >> 8) & empty).count_ones() as i32;

        count
    }

    fn count_open_pairs(position: u64, empty: u64) -> i32 {
        let mut count = 0i32;

        let h_pair = position & (position >> 7);
        count += (h_pair & (empty >> 14) & (empty << 7)).count_ones() as i32;

        let v_pair = position & (position >> 1);
        count += (v_pair & (empty >> 2)).count_ones() as i32;

        let d1_pair = position & (position >> 6);
        count += (d1_pair & (empty >> 12) & (empty << 6)).count_ones() as i32;

        let d2_pair = position & (position >> 8);
        count += (d2_pair & (empty >> 16) & (empty << 8)).count_ones() as i32;

        count
    }

    fn center_control_score(current: u64, opponent: u64) -> i32 {
        const COL_WEIGHTS: [i32; 7] = [1, 2, 3, 4, 3, 2, 1];

        let mut score = 0i32;

        for (col, &weight) in COL_WEIGHTS.iter().enumerate() {
            let col_mask = Bitboard::column_mask(col);
            let current_in_col = (current & col_mask).count_ones() as i32;
            let opponent_in_col = (opponent & col_mask).count_ones() as i32;
            score += (current_in_col - opponent_in_col) * weight;
        }

        score * 3
    }

    fn odd_even_analysis(current_wins: u64, opponent_wins: u64, moves: u32) -> i32 {
        let is_first_player = moves.is_multiple_of(2);

        let odd_mask: u64 = 0x2A
            | (0x2A << 7)
            | (0x2A << 14)
            | (0x2A << 21)
            | (0x2A << 28)
            | (0x2A << 35)
            | (0x2A << 42);
        let even_mask: u64 = Bitboard::BOARD_MASK & !odd_mask;

        let current_odd = (current_wins & odd_mask).count_ones() as i32;
        let current_even = (current_wins & even_mask).count_ones() as i32;
        let opponent_odd = (opponent_wins & odd_mask).count_ones() as i32;
        let opponent_even = (opponent_wins & even_mask).count_ones() as i32;

        if is_first_player {
            (current_odd * 15 + current_even * 5) - (opponent_odd * 5 + opponent_even * 15)
        } else {
            (current_even * 15 + current_odd * 5) - (opponent_even * 5 + opponent_odd * 15)
        }
    }

    fn connectivity_score(position: u64) -> i32 {
        let mut score = 0i32;
        score += (position & (position >> 7)).count_ones() as i32;
        score += (position & (position >> 1)).count_ones() as i32;
        score += (position & (position >> 6)).count_ones() as i32;
        score += (position & (position >> 8)).count_ones() as i32;
        score
    }

    pub fn find_best_move(&mut self, board: &Bitboard, max_depth: usize) -> (usize, i32) {
        self.nodes_explored = 0;

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

            if best_score >= (board.moves_remaining() - depth as i32) / 2 {
                break;
            }
        }

        (best_col, best_score)
    }
}

// ============================================================================
// Parallel Search
// ============================================================================

/// Parallel search using Lazy SMP
pub fn parallel_find_best_move(
    board: &Bitboard,
    tt: &mut TranspositionTable,
    max_depth: usize,
) -> (usize, i32) {
    if max_depth <= 8 || board.moves_remaining() <= ENDGAME_THRESHOLD {
        let mut solver = Solver::new(tt);
        return solver.find_best_move(board, max_depth);
    }

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

    let board_copy = *board;
    let results: Vec<(usize, i32)> = moves
        .par_iter()
        .map(|&col| {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = safe_moves & col_mask;
            let new_board = board_copy.play_move(move_mask);

            let mut local_tt = TranspositionTable::new();
            let mut solver = Solver::new(&mut local_tt);

            let score = -solver.negamax(&new_board, -WIN_SCORE, WIN_SCORE, max_depth as u8, 1);
            (col, score)
        })
        .collect();

    let mut final_best_col = 3;
    let mut final_best_score = i32::MIN;

    for (col, score) in results {
        if score > final_best_score || (score == final_best_score && col == 3) {
            final_best_score = score;
            final_best_col = col;
        }
    }

    tt.put(
        board,
        final_best_score,
        TTFlag::Exact,
        max_depth as u8,
        Some(final_best_col),
    );

    (final_best_col, final_best_score)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_endgame_solver() {
        let mut board = Bitboard::new();
        board = board.play(0);
        board = board.play(6);
        board = board.play(1);
        board = board.play(6);
        board = board.play(2);
        board = board.play(6);

        let score = EndgameSolver::solve(&board, -WIN_SCORE, WIN_SCORE);
        assert!(score > 0);
    }
}

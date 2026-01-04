//! Connect Four AI with advanced techniques
//!
//! Features:
//! - Bitboard representation for fast win detection
//! - Transposition table with Zobrist hashing
//! - Negamax with alpha-beta pruning and PVS
//! - Killer move heuristic
//! - Parallel search using Lazy SMP
//! - Perfect endgame solver

pub mod bitboard;
pub mod game;
pub mod solver;
pub mod ui;

// Board dimensions
pub const ROWS: usize = 6;
pub const COLS: usize = 7;
pub const TOTAL_CELLS: usize = ROWS * COLS;

// Scores for win/loss detection
pub const WIN_SCORE: i32 = 1_000_000;
pub const DRAW_SCORE: i32 = 0;

// Transposition table size (must be power of 2 for fast modulo)
pub const TT_SIZE: usize = 1 << 24; // 16 million entries (~256 MB)
pub const TT_MASK: u64 = (TT_SIZE - 1) as u64;

// Killer move table depth
pub const MAX_KILLER_DEPTH: usize = 64;
pub const KILLERS_PER_PLY: usize = 2;

// Endgame tablebase threshold - solve perfectly when this many moves remain
pub const ENDGAME_THRESHOLD: i32 = 14;

use std::collections::HashMap;
use std::io;

use colored::Colorize;
use to_int_and_back::to;

// Board dimensions
const ROWS: usize = 6;
const COLS: usize = 7;

// Scores for win/loss detection
const WIN_SCORE: i32 = 1_000_000;
const DRAW_SCORE: i32 = 0;

// Bitboard layout for 7x6 board (using 7 bits per column for easy vertical operations)
// The board is stored with each column using 7 bits (6 for cells + 1 sentinel bit)
// This allows for very fast win detection using bit operations
//
// Column layout (bit positions):
//  5 12 19 26 33 40 47
//  4 11 18 25 32 39 46
//  3 10 17 24 31 38 45
//  2  9 16 23 30 37 44
//  1  8 15 22 29 36 43
//  0  7 14 21 28 35 42
//
// The top bit (6, 13, 20, etc.) serves as a sentinel for column-full detection

/// Bitboard representation of the game state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Bitboard {
    /// Current player's pieces (the player about to move)
    current: u64,
    /// Mask of all pieces (both players)
    mask: u64,
    /// Number of moves played
    moves: u32,
}

impl Bitboard {
    const HEIGHT: u32 = ROWS as u32;

    // Bottom row mask: one bit at the bottom of each column
    // For a 7-column board with 7 bits per column (6 cells + 1 sentinel):
    // Bits at positions: 0, 7, 14, 21, 28, 35, 42
    const BOTTOM: u64 =
        (1 << 0) | (1 << 7) | (1 << 14) | (1 << 21) | (1 << 28) | (1 << 35) | (1 << 42);

    // Full board mask (all playable cells) - 6 rows × 7 columns
    // Each column has bits 0-5 set (6 playable cells), bit 6 is sentinel
    const BOARD_MASK: u64 = Self::BOTTOM * ((1 << Self::HEIGHT) - 1);

    fn new() -> Self {
        Bitboard {
            current: 0,
            mask: 0,
            moves: 0,
        }
    }

    /// Returns the opponent's pieces
    #[inline]
    fn opponent(&self) -> u64 {
        self.current ^ self.mask
    }

    /// Check if a column can accept another piece
    #[inline]
    fn can_play(&self, col: usize) -> bool {
        (self.mask & Self::top_mask(col)) == 0
    }

    /// Play a piece in the given column
    /// Returns the new bitboard state
    #[inline]
    fn play(&self, col: usize) -> Bitboard {
        let mut new_board = *self;
        new_board.current ^= new_board.mask;
        new_board.mask |= new_board.mask + Self::bottom_mask(col);
        new_board.moves += 1;
        new_board
    }

    /// Play a move given the move bitmask (faster than column-based play)
    #[inline]
    fn play_move(&self, move_mask: u64) -> Bitboard {
        let mut new_board = *self;
        new_board.current ^= new_board.mask;
        new_board.mask |= move_mask;
        new_board.moves += 1;
        new_board
    }

    /// Check if a position is a winning position
    /// Uses the efficient 4-direction bit shift method
    #[inline]
    fn is_winning(position: u64) -> bool {
        // Horizontal check
        let mut m = position & (position >> 7);
        if m & (m >> 14) != 0 {
            return true;
        }

        // Diagonal \ check
        m = position & (position >> 6);
        if m & (m >> 12) != 0 {
            return true;
        }

        // Diagonal / check
        m = position & (position >> 8);
        if m & (m >> 16) != 0 {
            return true;
        }

        // Vertical check
        m = position & (position >> 1);
        m & (m >> 2) != 0
    }

    /// Check if the opponent just made a winning move
    #[inline]
    fn opponent_wins(&self) -> bool {
        Self::is_winning(self.opponent())
    }

    /// Get positions where the current player can win immediately
    #[inline]
    fn winning_positions(&self) -> u64 {
        Self::compute_winning_positions(self.current, self.mask)
    }

    /// Get positions where the opponent could win
    #[inline]
    fn opponent_winning_positions(&self) -> u64 {
        Self::compute_winning_positions(self.opponent(), self.mask)
    }

    /// Compute winning positions for a given position
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

        // Return only empty, playable positions
        r & (Self::BOARD_MASK ^ mask)
    }

    /// Get possible non-losing moves
    /// These are moves that don't immediately let the opponent win
    fn possible_non_losing_moves(&self) -> u64 {
        let possible = self.possible_moves();
        let opponent_win = self.opponent_winning_positions();
        let forced_moves = possible & opponent_win;

        if forced_moves != 0 {
            // Must block opponent's winning move
            // If there are multiple winning threats, we lose
            if forced_moves & (forced_moves - 1) != 0 {
                return 0; // Multiple threats, no good moves
            }
            return forced_moves; // Only one forced move
        }

        // Avoid moves that give opponent a winning position directly above
        possible & !(opponent_win >> 1)
    }

    /// Get all possible moves as a bitmask
    #[inline]
    fn possible_moves(&self) -> u64 {
        (self.mask + Self::BOTTOM) & Self::BOARD_MASK
    }

    /// Check if board is full
    #[inline]
    fn is_full(&self) -> bool {
        self.moves >= (ROWS * COLS) as u32
    }

    /// Count number of moves remaining
    #[inline]
    fn moves_remaining(&self) -> i32 {
        (ROWS * COLS) as i32 - self.moves as i32
    }

    /// Mask for the top cell of a column
    #[inline]
    fn top_mask(col: usize) -> u64 {
        1u64 << ((Self::HEIGHT as u64 - 1) + col as u64 * (Self::HEIGHT as u64 + 1))
    }

    /// Mask for the bottom cell of a column
    #[inline]
    fn bottom_mask(col: usize) -> u64 {
        1u64 << (col as u64 * (Self::HEIGHT as u64 + 1))
    }

    /// Mask for an entire column
    #[inline]
    fn column_mask(col: usize) -> u64 {
        ((1u64 << Self::HEIGHT) - 1) << (col as u64 * (Self::HEIGHT as u64 + 1))
    }

    /// Generate a unique key for transposition table
    /// Uses position + mask encoding that's unique for each game state
    #[inline]
    fn key(&self) -> u64 {
        self.current + self.mask
    }

    /// Convert to display array for printing
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

    /// Compute a score for move ordering - moves that create more threats are better
    fn move_score(&self, move_mask: u64) -> i32 {
        let new_position = self.current | move_mask;
        Self::compute_winning_positions(new_position, self.mask | move_mask).count_ones() as i32
    }
}

/// Transposition table entry types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TTFlag {
    Exact,
    LowerBound,
    UpperBound,
}

/// Transposition table entry
#[derive(Clone, Copy)]
struct TTEntry {
    key: u64,
    score: i32,
    flag: TTFlag,
    depth: u8,
}

/// Transposition table for caching evaluated positions
struct TranspositionTable {
    table: HashMap<u64, TTEntry>,
}

impl TranspositionTable {
    fn new() -> Self {
        TranspositionTable {
            table: HashMap::with_capacity(8_000_000),
        }
    }

    fn get(&self, key: u64) -> Option<&TTEntry> {
        self.table.get(&key).filter(|e| e.key == key)
    }

    fn put(&mut self, key: u64, score: i32, flag: TTFlag, depth: u8) {
        // Always replace strategy with depth preference
        if let Some(existing) = self.table.get(&key) {
            if existing.depth > depth && existing.flag == TTFlag::Exact {
                return;
            }
        }
        self.table.insert(
            key,
            TTEntry {
                key,
                score,
                flag,
                depth,
            },
        );
    }
}

/// Opening book - pre-computed best moves for early game positions
struct OpeningBook {
    positions: HashMap<u64, usize>,
}

impl OpeningBook {
    fn new() -> Self {
        let mut positions = HashMap::new();

        // Empty board - play center
        positions.insert(0, 3);

        // After opponent plays center, we play center too (on top)
        let board = Bitboard::new().play(3);
        positions.insert(board.key(), 3);

        // After opponent plays edge columns, we play center
        for col in [0, 1, 2, 4, 5, 6] {
            let board = Bitboard::new().play(col);
            positions.insert(board.key(), 3);
        }

        // Common responses in strong play
        // If opponent plays 3, we play 3, then if they play 2, we play 4
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

/// Game state
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
            // The opponent of current player won (the one who just played)
            Some(if self.red_turn { "YELLOW" } else { "RED" })
        } else {
            None // Draw or game not over
        }
    }
}

/// Column order for move exploration (center-first for better pruning)
const COLUMN_ORDER: [usize; 7] = [3, 2, 4, 1, 5, 0, 6];

/// Negamax solver with alpha-beta pruning, transposition table, and advanced techniques
struct Solver<'a> {
    tt: &'a mut TranspositionTable,
    nodes_explored: u64,
}

impl<'a> Solver<'a> {
    fn new(tt: &'a mut TranspositionTable) -> Self {
        Solver {
            tt,
            nodes_explored: 0,
        }
    }

    /// Principal Variation Search (PVS) - an enhancement of alpha-beta
    /// First move is searched with full window, others with null window
    fn negamax(&mut self, board: &Bitboard, mut alpha: i32, mut beta: i32, depth: u8) -> i32 {
        self.nodes_explored += 1;

        // Check for draw
        if board.is_full() {
            return DRAW_SCORE;
        }

        // Check if current player can win immediately
        let winning = board.winning_positions();
        let possible = board.possible_moves();
        if winning & possible != 0 {
            return (board.moves_remaining() + 1) / 2;
        }

        // Upper bound on score
        let max_score = (board.moves_remaining() - 1) / 2;
        if beta > max_score {
            beta = max_score;
            if alpha >= beta {
                return beta;
            }
        }

        // Transposition table lookup
        let key = board.key();
        if let Some(entry) = self.tt.get(key) {
            if entry.depth >= depth {
                match entry.flag {
                    TTFlag::Exact => return entry.score,
                    TTFlag::LowerBound => {
                        if entry.score >= beta {
                            return entry.score;
                        }
                        alpha = alpha.max(entry.score);
                    }
                    TTFlag::UpperBound => {
                        if entry.score <= alpha {
                            return entry.score;
                        }
                        beta = beta.min(entry.score);
                    }
                }
            }
        }

        // Depth limit - use static evaluation
        if depth == 0 {
            return self.evaluate(board);
        }

        // Get non-losing moves
        let moves = board.possible_non_losing_moves();
        if moves == 0 {
            return -board.moves_remaining() / 2;
        }

        // Sort moves by their potential (move ordering optimization)
        let mut move_list: Vec<(usize, i32)> = COLUMN_ORDER
            .iter()
            .filter_map(|&col| {
                let col_mask = Bitboard::column_mask(col);
                let move_mask = moves & col_mask;
                if move_mask != 0 {
                    Some((col, board.move_score(move_mask)))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending (best moves first)
        move_list.sort_by(|a, b| b.1.cmp(&a.1));

        let mut best_score = i32::MIN;
        let orig_alpha = alpha;
        let mut first_move = true;

        for (col, _) in move_list {
            let col_mask = Bitboard::column_mask(col);
            let move_mask = moves & col_mask;
            let new_board = board.play_move(move_mask);

            let score = if first_move {
                // Search first move with full window
                -self.negamax(&new_board, -beta, -alpha, depth - 1)
            } else {
                // Null window search for other moves
                let mut score = -self.negamax(&new_board, -alpha - 1, -alpha, depth - 1);
                if score > alpha && score < beta {
                    // Re-search with full window if it might be better
                    score = -self.negamax(&new_board, -beta, -alpha, depth - 1);
                }
                score
            };
            first_move = false;

            if score > best_score {
                best_score = score;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                break;
            }
        }

        // Handle case where no moves were tried
        if best_score == i32::MIN {
            best_score = -board.moves_remaining() / 2;
        }

        // Store in transposition table
        let flag = if best_score <= orig_alpha {
            TTFlag::UpperBound
        } else if best_score >= beta {
            TTFlag::LowerBound
        } else {
            TTFlag::Exact
        };
        self.tt.put(key, best_score, flag, depth);

        best_score
    }

    /// Find the best move for the current player using iterative deepening
    fn find_best_move(&mut self, board: &Bitboard, max_depth: usize) -> (usize, i32) {
        self.nodes_explored = 0;

        // Check for immediate wins first
        let winning = board.winning_positions();
        let possible = board.possible_moves();

        if winning & possible != 0 {
            // Find which column wins
            for col in COLUMN_ORDER {
                let col_mask = Bitboard::column_mask(col);
                if winning & possible & col_mask != 0 {
                    return (col, (board.moves_remaining() + 1) / 2);
                }
            }
        }

        // Get non-losing moves
        let safe_moves = board.possible_non_losing_moves();
        if safe_moves == 0 {
            // All moves lose - just play first available
            for col in COLUMN_ORDER {
                if board.can_play(col) {
                    return (col, -WIN_SCORE);
                }
            }
        }

        let mut best_col = 3;
        let mut best_score = i32::MIN;

        // Iterative deepening with aspiration windows
        for depth in 1..=max_depth {
            let mut alpha = -WIN_SCORE;
            let beta = WIN_SCORE;

            // Sort moves for this iteration
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

                let score = -self.negamax(&new_board, -beta, -alpha, depth as u8);

                if score > best_score || (score == best_score && *col == 3) {
                    best_score = score;
                    best_col = *col;
                }
                if score > alpha {
                    alpha = score;
                }
            }

            // Early exit if we found a winning move
            if best_score >= (board.moves_remaining() - depth as i32) / 2 {
                break;
            }
        }

        (best_col, best_score)
    }

    /// Static evaluation for positions at depth limit
    fn evaluate(&self, board: &Bitboard) -> i32 {
        let current = board.current;
        let opponent = board.opponent();

        // Count threats for each player
        let current_threats = Self::count_threats(current, board.mask);
        let opponent_threats = Self::count_threats(opponent, board.mask);

        // Also consider potential winning positions
        let current_wins = Bitboard::compute_winning_positions(current, board.mask).count_ones() as i32;
        let opponent_wins = Bitboard::compute_winning_positions(opponent, board.mask).count_ones() as i32;

        (current_threats * 2 + current_wins * 3) - (opponent_threats * 2 + opponent_wins * 3)
    }

    /// Count potential threats (3-in-a-row with empty slot)
    fn count_threats(position: u64, mask: u64) -> i32 {
        let empty = Bitboard::BOARD_MASK ^ mask;
        let mut count = 0i32;

        // Horizontal
        let h1 = position & (position >> 7);
        let h2 = h1 & (position >> 14);
        count += (h2 & (empty >> 21)).count_ones() as i32;
        count += (h2 & (empty << 7)).count_ones() as i32;

        // Also count 2-in-a-row patterns with 2 empty slots
        let h_open = h1 & (empty >> 14) & (empty >> 21);
        count += h_open.count_ones() as i32 / 2;

        // Vertical
        let v1 = position & (position >> 1);
        let v2 = v1 & (position >> 2);
        count += (v2 & (empty >> 3)).count_ones() as i32;

        // Diagonals
        let d1 = position & (position >> 6);
        let d2 = d1 & (position >> 12);
        count += (d2 & (empty >> 18)).count_ones() as i32;
        count += (d2 & (empty << 6)).count_ones() as i32;

        let d3 = position & (position >> 8);
        let d4 = d3 & (position >> 16);
        count += (d4 & (empty >> 24)).count_ones() as i32;
        count += (d4 & (empty << 8)).count_ones() as i32;

        count
    }
}

fn main() {
    intro();

    let depth = get_difficulty();
    let mut game = Game::new(depth);

    print_board(game.board);

    loop {
        // Human player's turn
        let col = get_human_move(&game);
        game.play(col);
        print_board(game.board);

        if game.is_game_over() {
            print_result(&game);
            break;
        }

        // AI's turn
        println!("AI is thinking...");

        // Check opening book first
        let (ai_col, score) = if let Some(book_move) = game.opening_book.lookup(&game.board) {
            (book_move, 0)
        } else {
            let mut solver = Solver::new(&mut game.tt);
            solver.find_best_move(&game.board, game.depth)
        };

        game.play(ai_col);
        println!(
            "AI plays column {} (eval: {})",
            ai_col + 1,
            if score > 0 {
                format!("+{score}")
            } else {
                score.to_string()
            }
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
    print!("Connect Four!");
    print!("   ");
    print!("{}", "O".yellow());
    print!("{}", "O".yellow());
    print!("{}", "O".yellow());
    print!("{}", "O\n\n".yellow());
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
            "easy" => return 6,
            "medium" => return 10,
            "hard" => return 14,
            "vhard" => return 18,
            "expert" => return 24,
            "impossible" => return 42, // Full tree search (perfect play)
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
        // Player 1 plays: 0, 1, 2, 3 (with player 2 playing elsewhere)
        board = board.play(0); // P1
        board = board.play(6); // P2
        board = board.play(1); // P1
        board = board.play(6); // P2
        board = board.play(2); // P1
        board = board.play(6); // P2
        board = board.play(3); // P1 wins

        assert!(board.opponent_wins());
    }

    #[test]
    fn test_vertical_win() {
        let mut board = Bitboard::new();
        // Player 1 stacks in column 0
        board = board.play(0); // P1
        board = board.play(1); // P2
        board = board.play(0); // P1
        board = board.play(1); // P2
        board = board.play(0); // P1
        board = board.play(1); // P2
        board = board.play(0); // P1 wins

        assert!(board.opponent_wins());
    }

    #[test]
    fn test_diagonal_win() {
        let mut board = Bitboard::new();
        // Set up diagonal win for P1
        // P1: (5,0), (4,1), (3,2), (2,3)
        board = board.play(0); // P1 at (5,0)
        board = board.play(1); // P2 at (5,1)
        board = board.play(1); // P1 at (4,1)
        board = board.play(2); // P2 at (5,2)
        board = board.play(2); // P1 at (4,2)
        board = board.play(3); // P2 at (5,3)
        board = board.play(2); // P1 at (3,2)
        board = board.play(3); // P2 at (4,3)
        board = board.play(3); // P1 at (3,3)
        board = board.play(0); // P2 at (4,0)
        board = board.play(3); // P1 at (2,3) - wins diagonal!

        assert!(board.opponent_wins());
    }

    #[test]
    fn test_no_win() {
        let board = Bitboard::new();
        assert!(!board.opponent_wins());
    }

    #[test]
    fn test_winning_positions() {
        let mut board = Bitboard::new();
        // Set up 3 in a row
        board = board.play(0); // P1
        board = board.play(6); // P2
        board = board.play(1); // P1
        board = board.play(6); // P2
        board = board.play(2); // P1

        // Check P1's winning positions
        let winning = Bitboard::compute_winning_positions(board.opponent(), board.mask);
        // P1 should be able to win at column 3
        let col3_bottom = Bitboard::bottom_mask(3);
        assert!(winning & col3_bottom != 0);
    }

    #[test]
    fn test_solver_finds_winning_move() {
        let mut board = Bitboard::new();
        // Set up 3 in a row for P1
        board = board.play(0); // P1
        board = board.play(6); // P2
        board = board.play(1); // P1
        board = board.play(6); // P2
        board = board.play(2); // P1
        board = board.play(6); // P2
        // Now it's P1's turn, they should win by playing column 3

        let mut tt = TranspositionTable::new();
        let mut solver = Solver::new(&mut tt);
        let (best_col, _) = solver.find_best_move(&board, 10);
        assert_eq!(best_col, 3);
    }

    #[test]
    fn test_solver_blocks_opponent_win() {
        let mut board = Bitboard::new();
        // Set up 3 in a row for P1, but it's P2's turn
        board = board.play(0); // P1
        board = board.play(6); // P2
        board = board.play(1); // P1
        board = board.play(6); // P2
        board = board.play(2); // P1
        // Now P2 must block at column 3

        let mut tt = TranspositionTable::new();
        let mut solver = Solver::new(&mut tt);
        let (best_col, _) = solver.find_best_move(&board, 10);
        assert_eq!(best_col, 3);
    }

    #[test]
    fn test_transposition_table() {
        let mut tt = TranspositionTable::new();
        tt.put(12345, 5, TTFlag::Exact, 10);

        let entry = tt.get(12345);
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.score, 5);
        assert_eq!(e.flag, TTFlag::Exact);
    }

    #[test]
    fn test_opening_book() {
        let book = OpeningBook::new();
        let board = Bitboard::new();
        // Empty board should suggest center
        assert_eq!(book.lookup(&board), Some(3));
    }

    #[test]
    fn test_possible_non_losing_moves() {
        let mut board = Bitboard::new();
        // Create a position where P2 has 3 in a row
        board = board.play(0); // P1
        board = board.play(3); // P2
        board = board.play(1); // P1
        board = board.play(3); // P2
        board = board.play(6); // P1
        board = board.play(3); // P2 has 3 in column 3
        // P1 must block at column 3

        let non_losing = board.possible_non_losing_moves();
        let col3_mask = Bitboard::column_mask(3) & board.possible_moves();
        // Only column 3 should be a valid non-losing move
        assert_eq!(non_losing, col3_mask);
    }
}

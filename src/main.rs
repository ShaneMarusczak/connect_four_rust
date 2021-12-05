use std::io;

use colored::Colorize;

const ROWS: usize = 6;
const COLS: usize = 7;
const RED_WIN: isize = -100000;
const YELLOW_WIN: isize = 100000;

const NOCOL: usize = 99;

#[derive(Clone)]
struct Board {
    array: [[usize; COLS]; ROWS],
}

struct Game {
    board: Board,
    red_turn: bool,
    turns_taken: usize,
    depth: usize,
}

fn main() {
    intro();
    println!("Depth?");

    let mut depth = String::new();

    io::stdin()
        .read_line(&mut depth)
        .expect("Failed to read line");

    let depth: usize = match depth.trim().parse() {
        Ok(num) => num,
        Err(e) => panic!("{}", e),
    };

    let mut game = game_init(depth);

    loop {
        println!("Which column to play token?");
        let mut col_input = String::new();

        io::stdin()
            .read_line(&mut col_input)
            .expect("Failed to read line");

        let col_input: usize = match col_input.trim().parse() {
            Ok(num) => num,
            Err(_) => continue,
        };

        if can_place_in_col(&game.board, col_input) {
            play_move(&mut game, col_input);
            print_board(&game.board);
        }
        let comp_move = get_comp_move(&game);
        play_move(&mut game, comp_move);
        print_board(&game.board);

        if check_game_over(&game) {
            break;
        }
    }
}

fn intro() {
    print!("{}", "\n\nO".red());
    print!("{}", "O".red());
    print!("{}", "O".red());
    print!("{}", "O".red());
    print!("{}", "   ");
    print!("Connect Four!");
    print!("{}", "   ");
    print!("{}", "O".yellow());
    print!("{}", "O".yellow());
    print!("{}", "O".yellow());
    print!("{}", "O\n\n".yellow());

    println!("Columns are numbered 1-7")
}

fn game_init(depth: usize) -> Game {
    Game {
        board: Board {
            array: [[0; COLS]; ROWS],
        },
        red_turn: true,
        turns_taken: 0,
        depth,
    }
}

fn can_place_in_col(board: &Board, col: usize) -> bool {
    board.array[0][col - 1] == 0
}

fn play_move(game: &mut Game, col: usize) {
    game.turns_taken += 1;
    game.red_turn = !game.red_turn;
    let mut row_to_check = game.board.array.len() - 1;
    while game.board.array[row_to_check][col - 1] != 0 {
        row_to_check -= 1;
    }
    game.board.array[row_to_check][col - 1] = if game.red_turn { 1 } else { 2 };
}

fn print_board(board: &Board) {
    print!("{}", "___________________________________\n");
    for row in board.array {
        for cell in row {
            if cell == 0 {
                print!("{}", "|   |");
            } else if cell == 1 {
                print!("{}", "| ".white());
                print!("{}", "◯".red());
                print!("{}", " |".white());
            } else if cell == 2 {
                print!("{}", "| ".white());
                print!("{}", "◯".yellow());
                print!("{}", " |".white());
            }
        }
        print!("{}", "\n")
    }
    print!("{}", "¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯\n");
}

fn score_board(board: &Board, row: usize, col: usize, d_y: isize, d_x: isize) -> isize {
    let mut red_points: isize = 0;
    let mut yellow_points: isize = 0;
    let mut inner_row = row;
    let mut inner_col = col;

    for _ in 0..4 {
        if board.array[inner_row][inner_col] == 1 {
            red_points += 1;
        } else if board.array[inner_row][inner_col] == 2 {
            yellow_points += 1;
        }
        inner_row = (inner_row as isize + d_y) as usize;
        inner_col = (inner_col as isize + d_x) as usize;
    }
    if red_points == 4 {
        return RED_WIN;
    }
    if yellow_points == 4 {
        return YELLOW_WIN;
    }
    yellow_points
}

fn evaluate_score(board: &Board) -> isize {
    let mut ver_points = 0;
    let mut hor_points = 0;
    let mut diag_1_points = 0;
    let mut diag_2_points = 0;

    for row in 0..ROWS - 3 {
        for col in 0..COLS {
            let score = score_board(&board, row, col, 1, 0);
            if score == RED_WIN {
                return RED_WIN;
            }
            if score == YELLOW_WIN {
                return YELLOW_WIN;
            }
            ver_points += score;
        }
    }

    for row in 0..ROWS {
        for col in 0..COLS - 3 {
            let score = score_board(&board, row, col, 0, 1);
            if score == RED_WIN {
                return RED_WIN;
            }
            if score == YELLOW_WIN {
                return YELLOW_WIN;
            }
            hor_points += score;
        }
    }

    for row in 0..ROWS - 3 {
        for col in 0..COLS - 3 {
            let score = score_board(&board, row, col, 1, 1);
            if score == RED_WIN {
                return RED_WIN;
            }
            if score == YELLOW_WIN {
                return YELLOW_WIN;
            }
            diag_1_points += score;
        }
    }

    for row in 3..ROWS {
        for col in 0..COLS - 4 {
            let score = score_board(&board, row, col, -1, 1);
            if score == RED_WIN {
                return RED_WIN;
            }
            if score == YELLOW_WIN {
                return YELLOW_WIN;
            }
            diag_2_points += score;
        }
    }

    ver_points + hor_points + diag_1_points + diag_2_points
}

fn check_game_over(game: &Game) -> bool {
    let score = evaluate_score(&game.board);
    let mut rv = false;
    if score == RED_WIN {
        rv = true;
        println!("RED WINS");
    } else if score == YELLOW_WIN {
        rv = true;
        println!("YELLOW WINS");
    } else if is_draw(&game.board) {
        rv = true;
        println!("DRAW");
    }
    rv
}

fn is_draw(board: &Board) -> bool {
    for col in 0..COLS {
        if board.array[0][col] == 0 {
            return false;
        }
    }
    return true;
}

fn get_board_copy(board: &Board) -> Board {
    board.clone()
}

fn get_comp_move(game: &Game) -> usize {
    let result = maximize(&game.board, game.depth, -100000, 100000);
    result.col
}

struct AIMove {
    col: usize,
    score: isize,
}

fn maximize(board: &Board, depth: usize, mut alpha: isize, beta: isize) -> AIMove {
    let score = evaluate_score(board);
    if is_finished(board, depth, score) {
        return AIMove { col: NOCOL, score };
    }
    let mut max = AIMove {
        col: NOCOL,
        score: -99999,
    };
    for col in [4, 3, 5, 2, 6, 1, 0] {
        let new_board = get_board_copy(board);
        if can_place_in_col(board, col + 1) {
            let next_move = minimize(&new_board, depth - 1, alpha, beta);
            if max.col == NOCOL || next_move.score > max.score {
                max.col = col;
                max.score = next_move.score;
                alpha = next_move.score;
            }
            if alpha >= beta {
                return max;
            }
        }
    }
    max
}

fn minimize(board: &Board, depth: usize, alpha: isize, mut beta: isize) -> AIMove {
    let score = evaluate_score(board);
    if is_finished(board, depth, score) {
        return AIMove { col: NOCOL, score };
    }
    let mut max = AIMove {
        col: NOCOL,
        score: 99999,
    };
    for col in [4, 3, 5, 2, 6, 1, 0] {
        let new_board = get_board_copy(board);
        if can_place_in_col(board, col + 1) {
            let next_move = maximize(&new_board, depth - 1, alpha, beta);
            if max.col == NOCOL || next_move.score < max.score {
                max.col = col;
                max.score = next_move.score;
                beta = next_move.score;
            }
            if alpha >= beta {
                return max;
            }
        }
    }
    max
}

fn is_finished(board: &Board, depth: usize, score: isize) -> bool {
    depth == 0 || score == RED_WIN || score == YELLOW_WIN || is_draw(board)
}

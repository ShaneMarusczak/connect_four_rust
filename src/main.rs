use std::io;

use colored::Colorize;
use to_int_and_back::to;

const ROWS: usize = 6;
const COLS: usize = 7;
const RED_WIN: isize = -100000;
const YELLOW_WIN: isize = 100000;

const NOCOL: usize = 99;

#[derive(Clone, Debug)]
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
    //TODO: Break this into smaller functions and a lib.rs
    intro();
    let mut getting_dif = true;
    let mut dif = String::new();
    let mut depth = 0;

    while getting_dif {
        dif.clear();
        println!("Difficuly?");
        println!("Valid values are easy, medium, hard, vhard");

        io::stdin()
            .read_line(&mut dif)
            .expect("Failed to read line");

        depth = match dif.trim().to_lowercase().as_str() {
            "easy" => 4,
            "medium" => 6,
            "hard" => 8,
            "vhard" => 9,
            _ => continue,
        };
        getting_dif = false;
    }

    let mut game = game_init(depth);
    print_board(&game.board);

    loop {
        println!("Which column to play token?");
        let mut col_input = String::new();

        io::stdin()
            .read_line(&mut col_input)
            .expect("Failed to read line");

        let col: usize = match col_input.trim().parse() {
            Ok(num) => num,
            Err(_) => match to::int(&col_input.trim()) {
                Ok(num) => num as usize,
                Err(e) => {
                    print_board(&game.board);
                    println!("{}", e);
                    col_input.clear();
                    continue;
                }
            },
        };

        if can_place_in_col(&game.board, col - 1) {
            play_move(&mut game, col - 1);
            print_board(&game.board);
        } else {
            println!("Invalid Column");
            print_board(&game.board);
            continue;
        }
        if check_game_over(&game) {
            break;
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
    col < COLS && board.array[0][col] == 0
}

fn play_move(game: &mut Game, col: usize) {
    game.turns_taken += 1;
    play_move_inner(&mut game.board, col, game.red_turn);
    game.red_turn = !game.red_turn;
}

fn play_move_inner(board: &mut Board, col: usize, red_turn: bool) {
    //TODO: This function panics sometimes. Figure out why.
    let mut row_to_check = board.array.len() - 1;
    while board.array[row_to_check][col] != 0 {
        row_to_check -= 1;
    }
    board.array[row_to_check][col] = if red_turn { 1 } else { 2 };
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
    print!("{}", "  1    2    3    4    5    6    7 \n");
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

fn get_comp_move(game: &Game) -> usize {
    if game.turns_taken == 1 {
        return 3;
    }
    let mut rv = 4;
    let mut scan_order = [3, 2, 4, 1, 5, 6, 0];
    for d in 2..=game.depth {
        let best_move_at_depth = maximize(&game.board, d, RED_WIN, YELLOW_WIN, scan_order).col;
        rv = best_move_at_depth;
        if best_move_at_depth == 0 {
            scan_order = [0, 1, 2, 3, 4, 5, 6];
        } else if best_move_at_depth == 1 {
            scan_order = [1, 0, 2, 3, 4, 5, 6];
        } else if best_move_at_depth == 2 {
            scan_order = [2, 1, 3, 0, 4, 5, 6];
        } else if best_move_at_depth == 3 {
            scan_order = [3, 2, 4, 1, 5, 0, 6];
        } else if best_move_at_depth == 4 {
            scan_order = [4, 3, 5, 2, 6, 1, 0];
        } else if best_move_at_depth == 5 {
            scan_order = [5, 6, 4, 3, 2, 1, 0];
        } else if best_move_at_depth == 6 {
            scan_order = [6, 5, 4, 3, 2, 1, 0];
        }
    }
    rv
}

struct AIMove {
    col: usize,
    score: isize,
}

fn maximize(
    board: &Board,
    depth: usize,
    mut alpha: isize,
    beta: isize,
    scan_order: [usize; 7],
) -> AIMove {
    let score = evaluate_score(board);
    if is_finished(board, depth, score) {
        return AIMove { col: NOCOL, score };
    }
    let mut max = AIMove {
        col: NOCOL,
        score: -99999,
    };
    for col in scan_order {
        let mut new_board = board.clone();
        if can_place_in_col(&new_board, col) {
            play_move_inner(&mut new_board, col, false);
            let next_move = minimize(&new_board, depth - 1, alpha, beta, scan_order);
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

fn minimize(
    board: &Board,
    depth: usize,
    alpha: isize,
    mut beta: isize,
    scan_order: [usize; 7],
) -> AIMove {
    let score = evaluate_score(board);
    if is_finished(board, depth, score) {
        return AIMove { col: NOCOL, score };
    }
    let mut min = AIMove {
        col: NOCOL,
        score: 99999,
    };
    for col in scan_order {
        let mut new_board = board.clone();

        if can_place_in_col(&new_board, col) {
            play_move_inner(&mut new_board, col, true);

            let next_move = maximize(&new_board, depth - 1, alpha, beta, scan_order);
            if min.col == NOCOL || next_move.score < min.score {
                min.col = col;
                min.score = next_move.score;
                beta = next_move.score;
            }
            if alpha >= beta {
                return min;
            }
        }
    }
    min
}

fn is_finished(board: &Board, depth: usize, score: isize) -> bool {
    depth == 0 || score == RED_WIN || score == YELLOW_WIN || is_draw(board)
}

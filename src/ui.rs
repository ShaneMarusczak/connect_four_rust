//! Terminal UI for Connect Four

use std::io;

use colored::Colorize;
use to_int_and_back::to;

use crate::bitboard::Bitboard;
use crate::game::Game;
use crate::COLS;

/// Display the intro banner
pub fn intro() {
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

/// Get difficulty level from user
pub fn get_difficulty() -> usize {
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

/// Get move column from human player
pub fn get_human_move(game: &Game) -> usize {
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

/// Print the game board
pub fn print_board(board: Bitboard) {
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

/// Print the game result
pub fn print_result(game: &Game) {
    match game.get_winner() {
        Some(winner) => println!("{winner} WINS!"),
        None => println!("DRAW!"),
    }
}

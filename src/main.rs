//! Connect Four AI
//!
//! A powerful Connect Four AI with bitboard representation, transposition tables,
//! killer moves, parallel search, and perfect endgame solver.

use connect_four::game::Game;
use connect_four::solver::parallel_find_best_move;
use connect_four::ui::{get_difficulty, get_human_move, intro, print_board, print_result};

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

use colored::Colorize;

fn main() {
    let mut board: [[u8; 7]; 6] = [[0; 7]; 6];
    if can_place_in_col(&board, 2) {
        play_move(&mut board, 2, true);
    }
    if can_place_in_col(&board, 2) {
        play_move(&mut board, 2, false);
    }
    if can_place_in_col(&board, 2) {
        play_move(&mut board, 2, true);
    }
    if can_place_in_col(&board, 2) {
        play_move(&mut board, 2, false);
    }
    print_board(&board);
}

fn can_place_in_col(&board: &[[u8; 7]; 6], col: usize) -> bool {
    board[0][col - 1] == 0
}

fn play_move(board: &mut [[u8; 7]; 6], col: usize, is_red: bool) {
    let mut row_to_check = board.len() - 1;
    while board[row_to_check][col - 1] != 0 {
        row_to_check -= 1;
    }
    board[row_to_check][col - 1] = if is_red { 1 } else { 2 };
}

fn print_board(board: &[[u8; 7]; 6]) {
    print!("{}", "___________________________________\n");
    for row in board {
        for cell in row {
            if *cell == 0 {
                print!("{}", "|   |");
            } else if *cell == 1 {
                print!("{}", "| ".white());
                print!("{}", "O".red());
                print!("{}", " |".white());
            } else if *cell == 2 {
                print!("{}", "| ".white());
                print!("{}", "O".yellow());
                print!("{}", " |".white());
            }
        }
        print!("{}", "\n")
    }
    print!("{}", "¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯¯\n");
}

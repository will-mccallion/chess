mod board;
mod fen;
mod perft;
mod search;
mod types;
mod uci;
mod uci_io;
mod zobrist;

use crate::board::Board;
use crate::perft::{divide, perft};
use crate::search::best_move_depth;
use crate::types::START_FEN;
use crate::uci_io::{format_uci, parse_uci_move};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "chesser",
    version,
    about = "Chess engine with perft/uci/play modes"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run perft from FEN or startpos
    Perft {
        depth: usize,
        #[arg(long)]
        fen: Option<String>,
        #[arg(long)]
        divide: bool,
    },
    /// Play in the terminal (you vs engine). Moves in UCI like e2e4, g8f6.
    PlayCli {
        #[arg(long)]
        fen: Option<String>,
        /// Engine search depth (plies)
        #[arg(long, default_value_t = 3)]
        depth: usize,
    },
    /// Run UCI loop (for GUIs or engine-vs-engine)
    Uci,
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Uci) {
        Cmd::Perft {
            depth,
            fen,
            divide: div,
        } => {
            let fen_str = fen.unwrap_or_else(|| START_FEN.to_string());
            let mut b = Board::from_fen(&fen_str).unwrap_or_else(|e| {
                eprintln!("FEN parse error: {e}");
                std::process::exit(1);
            });
            if div {
                divide(&mut b, depth);
            } else {
                let n = perft(&mut b, depth);
                println!("perft({depth}) = {n}");
            }
        }
        Cmd::PlayCli { fen, depth } => {
            let fen_str = fen.unwrap_or_else(|| START_FEN.to_string());
            let mut b = Board::from_fen(&fen_str).unwrap_or_else(|e| {
                eprintln!("FEN parse error: {e}");
                std::process::exit(1);
            });
            play_cli(&mut b, depth);
        }
        Cmd::Uci => uci::run_uci(),
    }
}

fn play_cli(b: &mut Board, depth: usize) {
    use std::io::{self, Write};

    loop {
        println!("{}", b.to_fen());
        print_board_ascii(b);

        print!("your move (uci like e2e4, or 'quit'): ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let mv = line.trim().to_string();
        if mv == "quit" {
            break;
        }

        if let Some(m) = parse_uci_move(b, &mv) {
            // Already verified as legal by matching generated moves
            let _u = b.make_move(m);
        } else {
            println!("illegal or unrecognized move");
            continue;
        }

        // Engine move
        let bm = best_move_depth(b, depth);
        if let Some(m) = bm {
            let _u = b.make_move(m);
            println!("engine: {}", format_uci(m));
        } else {
            println!("(no moves) game over");
            break;
        }
    }
}

// very small ASCII rendering for CLI play
fn print_board_ascii(b: &Board) {
    use crate::types::Piece;

    const BLUE: &str = "\x1b[34m";
    const RESET: &str = "\x1b[0m";

    println!("\n   a b c d e f g h");
    println!(" +-----------------+");
    for r in (0..8).rev() {
        print!("{}| ", r + 1);
        for f in 0..8 {
            let p = b.piece_on[(r * 8 + f) as usize];

            // Map to a single-character string, wrapping black pieces in blue.
            let s = match p {
                Piece::Empty => ".".to_string(),
                Piece::WP => "P".to_string(),
                Piece::WN => "N".to_string(),
                Piece::WB => "B".to_string(),
                Piece::WR => "R".to_string(),
                Piece::WQ => "Q".to_string(),
                Piece::WK => "K".to_string(),
                Piece::BP => format!("{BLUE}p{RESET}"),
                Piece::BN => format!("{BLUE}n{RESET}"),
                Piece::BB => format!("{BLUE}b{RESET}"),
                Piece::BR => format!("{BLUE}r{RESET}"),
                Piece::BQ => format!("{BLUE}q{RESET}"),
                Piece::BK => format!("{BLUE}k{RESET}"),
            };

            print!("{s} ");
        }
        println!("|{}", r + 1);
    }
    println!(" +-----------------+");
    println!("   a b c d e f g h\n");
}

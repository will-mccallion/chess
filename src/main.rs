use chess::board::Board;
use chess::perft::{divide, perft};
use chess::search::{best_move_timed, extract_pv};
use chess::tt::SharedTransTable;
use chess::types::{Move, START_FEN};
use chess::uci;
use chess::uci_io::{format_uci, parse_uci_move};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

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
    Perft {
        depth: usize,
        #[arg(long)]
        fen: Option<String>,
        #[arg(long)]
        divide: bool,
    },
    PlayCli {
        #[arg(long)]
        fen: Option<String>,
        #[arg(long, default_value_t = 10000)]
        time: u64,
        #[arg(long, default_value_t = 64)]
        depth: usize,
        #[arg(long)]
        threads: Option<usize>,
    },
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
                eprint!("FEN parse error: {e}");
                std::process::exit(1);
            });
            if div {
                divide(&mut b, depth);
            } else {
                let n = perft(&mut b, depth);
                println!("perft({depth}) = {n}");
            }
        }
        Cmd::PlayCli {
            fen,
            time,
            depth,
            threads,
        } => {
            let threads_count = threads.unwrap_or_else(num_cpus::get).max(1);
            let fen_str = fen.unwrap_or_else(|| START_FEN.to_string());
            let mut b = Board::from_fen(&fen_str).unwrap_or_else(|e| {
                eprint!("FEN parse error: {e}");
                std::process::exit(1);
            });
            play_cli(&mut b, time, depth, threads_count);
        }
        Cmd::Uci => uci::run_uci(),
    }
}

fn play_cli(b: &mut Board, time_ms: u64, max_depth: usize, threads_count: usize) {
    {
        let mut _moves = Vec::new();
        b.generate_legal_moves(&mut _moves);
    }

    let tt_size_mb = 1024;
    let mut tt = SharedTransTable::new(tt_size_mb);

    struct PonderState {
        handle: Option<thread::JoinHandle<()>>,
        stop_signal: Arc<AtomicBool>,
    }
    let mut ponder_state = PonderState {
        handle: None,
        stop_signal: Arc::new(AtomicBool::new(false)),
    };

    let mut ponder_move_opt: Option<Move> = None;

    'gameloop: loop {
        print!("\x1B[2J\x1B[H"); // Clear screen
        println!("FEN: {}", b.to_fen());
        print_board_ascii(b);
        if let Some(pm) = ponder_move_opt {
            println!("(Engine is pondering your move: {})", format_uci(pm));
        }

        let mut legal_moves = Vec::new();
        b.generate_legal_moves(&mut legal_moves);

        if legal_moves.is_empty() {
            println!("You have no legal moves. Game Over.");
            break;
        }

        let mut user_move_made = false;
        while !user_move_made {
            print!("\nYour move (e.g., Nf3, e2e4, or 'quit'): ");
            io::stdout().flush().unwrap();
            let mut line = String::new();
            if io::stdin().read_line(&mut line).is_err() {
                break 'gameloop;
            }
            let input_str = line.trim();

            if input_str.eq_ignore_ascii_case("quit") {
                break 'gameloop;
            }

            if let Some(handle) = ponder_state.handle.take() {
                ponder_state.stop_signal.store(true, Ordering::Relaxed);
                handle.join().unwrap();
            }

            let mut user_move_opt = parse_uci_move(b, input_str);

            if user_move_opt.is_none() {
                for &legal_move in &legal_moves {
                    // --- THIS IS THE CORRECTED LINE ---
                    let san_str = b.to_san(legal_move, &legal_moves).replace(['+', '#'], "");
                    if san_str == input_str {
                        user_move_opt = Some(legal_move);
                        break;
                    }
                }
            }

            if let Some(user_move) = user_move_opt {
                if legal_moves.contains(&user_move) {
                    if Some(user_move) == ponder_move_opt {
                        println!("(Ponder hit!)");
                    }
                    let _u = b.make_move(user_move);
                    user_move_made = true;
                } else {
                    println!("Illegal move. Try again.");
                }
            } else {
                println!("Unrecognized or illegal move format. Try again.");
            }
        }

        print!("\x1B[2J\x1B[H"); // Clear screen
        println!("FEN: {}", b.to_fen());
        print_board_ascii(b);
        println!(
            "\nEngine is thinking for up to {} seconds using {} threads...",
            time_ms / 1000,
            threads_count
        );
        println!("(Search information will appear below)");
        println!("--------------------------------");
        io::stdout().flush().unwrap();

        let stop_signal = Arc::new(AtomicBool::new(false));
        let mut helpers = vec![];

        for _ in 0..(threads_count - 1) {
            let board_clone = b.clone();
            let tt_clone = tt.clone();
            let stop_clone = Arc::clone(&stop_signal);
            let handle = thread::spawn(move || {
                let mut tt_local = tt_clone;
                best_move_timed(
                    &board_clone,
                    &mut tt_local,
                    u64::MAX / 4,
                    max_depth,
                    stop_clone,
                    false,
                );
            });
            helpers.push(handle);
        }

        let (engine_move_opt, _, _) = best_move_timed(
            b,
            &mut tt,
            time_ms,
            max_depth,
            Arc::clone(&stop_signal),
            true,
        );

        stop_signal.store(true, Ordering::Relaxed);
        for h in helpers {
            let _ = h.join();
        }

        let engine_move = if let Some(m) = engine_move_opt {
            m
        } else {
            println!("Engine has no moves. Game Over.");
            break;
        };

        let pv = extract_pv(b.clone(), &tt, 2);
        ponder_move_opt = pv.get(1).copied();

        println!("\n--------------------------------");
        println!("Engine plays: {}", format_uci(engine_move));
        let _u = b.make_move(engine_move);
        thread::sleep(std::time::Duration::from_millis(500));

        if let Some(ponder_move) = ponder_move_opt {
            let mut legal_moves = Vec::new();
            b.generate_legal_moves(&mut legal_moves);
            if legal_moves.contains(&ponder_move) {
                let mut ponder_board = b.clone();
                let _ = ponder_board.make_move(ponder_move);
                let tt_clone = tt.clone();
                ponder_state.stop_signal.store(false, Ordering::Relaxed);
                let stop_clone = Arc::clone(&ponder_state.stop_signal);

                let handle = thread::spawn(move || {
                    let mut tt_local = tt_clone;
                    best_move_timed(
                        &ponder_board,
                        &mut tt_local,
                        u64::MAX / 4,
                        max_depth,
                        stop_clone,
                        false,
                    );
                });
                ponder_state.handle = Some(handle);
            }
        }
    }

    if let Some(handle) = ponder_state.handle.take() {
        ponder_state.stop_signal.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
    println!("Exiting game.");
}

fn print_board_ascii(b: &Board) {
    use chess::types::Piece;
    const BLUE: &str = "\x1b[34m";
    const RESET: &str = "\x1b[0m";
    println!("\n   a b c d e f g h");
    println!(" +-----------------+");
    for r in (0..8).rev() {
        print!("{}| ", r + 1);
        for f in 0..8 {
            let p = b.piece_on[(r * 8 + f) as usize];
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

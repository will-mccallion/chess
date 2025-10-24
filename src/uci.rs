use crate::board::Board;
use crate::opening_book::get_book_move;
use crate::search::{best_move_depth, best_move_timed};
use crate::time::TimeControl;
use crate::types::{Color, START_FEN};
use crate::uci_io::{format_uci, parse_uci_move};
use std::io::{self, Write};
use std::time::SystemTime;

fn extract_i64(cmd: &str, key: &str) -> Option<i64> {
    for tok in cmd.split_whitespace().collect::<Vec<_>>().windows(2) {
        if tok[0].eq_ignore_ascii_case(key) {
            if let Ok(v) = tok[1].parse::<i64>() {
                return Some(v);
            }
        }
    }
    None
}

pub fn run_uci() {
    let mut b = Board::from_fen(START_FEN).expect("valid startpos");
    let mut default_depth: usize = 8;
    let mut tc = TimeControl::default();

    loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let cmd = line.trim();

        if cmd.eq_ignore_ascii_case("uci") {
            println!("id name Rusty-Improved");
            println!("id author Your Name");
            println!("option name Hash type spin default 16 min 1 max 1024");
            println!(
                "option name Depth type spin default {} min 1 max 99",
                default_depth
            );
            println!("uciok");
            io::stdout().flush().ok();
            continue;
        }

        if cmd.eq_ignore_ascii_case("isready") {
            println!("readyok");
            io::stdout().flush().ok();
            continue;
        }

        if cmd.eq_ignore_ascii_case("ucinewgame") {
            b = Board::from_fen(START_FEN).unwrap();
            io::stdout().flush().ok();
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("setoption ") {
            let mut parts = rest.split_whitespace();
            if let Some(name_token) = parts.next() {
                if name_token.eq_ignore_ascii_case("name") {
                    let name = parts.next().unwrap_or("");
                    parts.next(); // skip "value"
                    let value = parts.next().unwrap_or("");
                    if name.eq_ignore_ascii_case("Depth") {
                        if let Ok(d) = value.parse::<usize>() {
                            if d > 0 {
                                default_depth = d;
                            }
                        }
                    }
                }
            }
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("position ") {
            let mut parts = rest.split_whitespace();
            let first = parts.next().unwrap_or("");
            if first.eq_ignore_ascii_case("startpos") {
                b = Board::from_fen(START_FEN).unwrap();
                parts.next(); // skip "moves"
            } else if first.eq_ignore_ascii_case("fen") {
                let fen_parts: Vec<&str> = parts.by_ref().take(6).collect();
                b = Board::from_fen(&fen_parts.join(" ")).unwrap();
            }
            for mv_str in parts {
                if let Some(mv) = parse_uci_move(&b, mv_str) {
                    let _u = b.make_move(mv);
                }
            }
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("go") {
            if let Some(book_uci) = get_book_move(&b) {
                println!("bestmove {}", book_uci);
                io::stdout().flush().ok();
                continue;
            }

            if let Some(depth) = extract_i64(rest, "depth") {
                if let Some(m) = best_move_depth(&b, depth as usize) {
                    println!("bestmove {}", format_uci(m));
                } else {
                    println!("bestmove 0000");
                }
                io::stdout().flush().ok();
                continue;
            }

            tc.wtime = extract_i64(rest, "wtime").unwrap_or(0);
            tc.btime = extract_i64(rest, "btime").unwrap_or(0);
            tc.winc = extract_i64(rest, "winc").unwrap_or(0);
            tc.binc = extract_i64(rest, "binc").unwrap_or(0);
            tc.movestogo = extract_i64(rest, "movestogo").unwrap_or(0) as i32;

            let (soft, _hard) = tc.allocation_ms(b.turn == Color::White);
            let time_to_use = if let Some(movetime) = extract_i64(rest, "movetime") {
                movetime
            } else {
                soft
            };

            let (best, _, _) = best_move_timed(&b, time_to_use as u64, 128);
            if let Some(m) = best {
                println!("bestmove {}", format_uci(m));
            } else {
                println!("bestmove 0000");
            }
            io::stdout().flush().ok();
            continue;
        }

        if cmd.eq_ignore_ascii_case("quit") {
            break;
        }
    }
}

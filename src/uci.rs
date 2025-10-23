// src/uci.rs
use crate::board::Board;
use crate::search::{best_move_depth, best_move_timed};
use crate::types::START_FEN;
use crate::uci_io::{format_uci, parse_uci_move};
use std::io::{self, Write};

fn extract_u64(cmd: &str, key: &str) -> Option<u64> {
    for tok in cmd.split_whitespace().collect::<Vec<_>>().windows(2) {
        if tok[0].eq_ignore_ascii_case(key) {
            if let Ok(v) = tok[1].parse::<u64>() {
                return Some(v);
            }
        }
    }
    None
}

pub fn run_uci() {
    let mut b = Board::from_fen(START_FEN).expect("valid startpos");

    // default caps (used when GUI sends neither movetime nor clocks)
    let mut default_depth: usize = 6;
    let mut default_time_ms: u64 = 1000;

    loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let cmd = line.trim();

        if cmd.eq_ignore_ascii_case("uci") {
            println!("id name Rusty");
            println!("id author you");
            println!(
                "option name Depth type spin default {} min 1 max 99",
                default_depth
            );
            println!(
                "option name DefaultMoveTimeMs type spin default {} min 1 max 60000",
                default_time_ms
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
            let mut name: Option<String> = None;
            let mut value: Option<String> = None;
            let toks: Vec<&str> = rest.split_whitespace().collect();
            let mut i = 0;
            while i < toks.len() {
                match toks[i].to_ascii_lowercase().as_str() {
                    "name" => {
                        i += 1;
                        let mut buf = Vec::new();
                        while i < toks.len() && !toks[i].eq_ignore_ascii_case("value") {
                            buf.push(toks[i]);
                            i += 1;
                        }
                        if !buf.is_empty() {
                            name = Some(buf.join(" "));
                        }
                    }
                    "value" => {
                        i += 1;
                        let mut buf = Vec::new();
                        while i < toks.len() {
                            buf.push(toks[i]);
                            i += 1;
                        }
                        if !buf.is_empty() {
                            value = Some(buf.join(" "));
                        }
                    }
                    _ => i += 1,
                }
            }
            if let (Some(n), Some(v)) = (name, value) {
                if n.eq_ignore_ascii_case("Depth") {
                    if let Ok(d) = v.parse::<usize>() {
                        if d >= 1 {
                            default_depth = d;
                        }
                    }
                } else if n.eq_ignore_ascii_case("DefaultMoveTimeMs") {
                    if let Ok(ms) = v.parse::<u64>() {
                        default_time_ms = ms.max(1);
                    }
                }
            }
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("position ") {
            // position [startpos | fen <fen...>] [moves <m1> <m2> ...]
            let mut parts = rest.split_whitespace().peekable();
            if let Some(tok) = parts.next() {
                if tok.eq_ignore_ascii_case("startpos") {
                    b = Board::from_fen(START_FEN).unwrap();
                } else if tok.eq_ignore_ascii_case("fen") {
                    let mut fen_tokens = Vec::with_capacity(6);
                    for _ in 0..6 {
                        if let Some(t) = parts.next() {
                            fen_tokens.push(t);
                        }
                    }
                    b = Board::from_fen(&fen_tokens.join(" ")).unwrap();
                }
            }
            let toks: Vec<&str> = rest.split_whitespace().collect();
            if let Some(idx) = toks.iter().position(|t| t.eq_ignore_ascii_case("moves")) {
                for mv_str in &toks[idx + 1..] {
                    if let Some(mv) = parse_uci_move(&b, mv_str) {
                        let _u = b.make_move(mv);
                    }
                }
            }
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("go") {
            // Prefer true time control if provided.
            let mut time_mode = None::<u64>; // movetime in ms
            if let Some(ms) = extract_u64(rest, "movetime") {
                time_mode = Some(ms);
            } else {
                // Very simple clock use: take 1/30th of side's remaining time, capped.
                let wtime = extract_u64(rest, "wtime");
                let btime = extract_u64(rest, "btime");
                let to_move_time = match b.turn {
                    crate::types::Color::White => wtime,
                    crate::types::Color::Black => btime,
                };
                if let Some(rem) = to_move_time {
                    let budget = (rem / 30).clamp(10, 3_000);
                    time_mode = Some(budget);
                }
            }

            // Depth override if given explicitly.
            if let Some(depth) = extract_u64(rest, "depth") {
                if let Some(m) = best_move_depth(&b, depth as usize) {
                    println!("bestmove {}", format_uci(m));
                } else {
                    println!("bestmove 0000");
                }
                io::stdout().flush().ok();
                continue;
            }

            // Time-managed search (default to a sane 1s if nothing provided)
            let ms = time_mode.unwrap_or(default_time_ms);
            let (best, _depth, _nodes) = best_move_timed(&b, ms, 64);
            if let Some(m) = best {
                println!("bestmove {}", format_uci(m));
            } else {
                println!("bestmove 0000");
            }
            io::stdout().flush().ok();
            continue;
        }

        if cmd.eq_ignore_ascii_case("isready") {
            println!("readyok");
            io::stdout().flush().ok();
            continue;
        }

        if cmd.eq_ignore_ascii_case("quit") {
            break;
        }

        // We don't run background pondering, so ignore "stop" politely.
        if cmd.eq_ignore_ascii_case("stop") {
            // no background search to stop
            continue;
        }
    }
}

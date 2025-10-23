// src/uci.rs
use crate::board::Board;
use crate::search::best_move_depth;
use crate::types::START_FEN;
use crate::uci_io::{format_uci, parse_uci_move};

use std::io::{self, Write};

/// Very small helper to extract an integer following a token, e.g. "movetime 300".
fn extract_u64(cmd: &str, key: &str) -> Option<u64> {
    // look for exact word boundary to avoid partial matches
    for tok in cmd.split_whitespace().collect::<Vec<_>>().windows(2) {
        if tok[0].eq_ignore_ascii_case(key) {
            if let Ok(v) = tok[1].parse::<u64>() {
                return Some(v);
            }
        }
    }
    None
}

/// Crude mapping from a movetime budget (ms) to a fixed search depth.
/// This is just to cooperate with cutechess `-each st=...` right away.
/// Tweak thresholds as your engine gets faster.
fn depth_from_movetime_ms(ms: u64, default_depth: usize) -> usize {
    match ms {
        0..=120 => 1,
        121..=350 => 2,
        351..=900 => 3,
        901..=2_500 => 4,
        2_501..=5_000 => 5,
        _ => default_depth.max(6),
    }
}

/// Optional: map remaining clock to depth if GUI sends wtime/btime.
/// Very rough and conservative.
fn depth_from_clock_ms(ms: u64, default_depth: usize) -> usize {
    match ms {
        0..=500 => 1,
        501..=1_500 => 2,
        1_501..=4_000 => 3,
        4_001..=12_000 => 4,
        12_001..=30_000 => 5,
        _ => default_depth.max(6),
    }
}

pub fn run_uci() {
    let mut b = Board::from_fen(START_FEN).expect("valid startpos");

    // Default fixed depth if no time hints are given.
    // You can change this at runtime with:  setoption name Depth value N
    let mut default_depth: usize = 4;

    loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let cmd = line.trim();

        // ---- Core UCI commands ----
        if cmd.eq_ignore_ascii_case("uci") {
            println!("id name Rusty");
            println!("id author you");
            // If you later add options, declare them here with "option name ...".
            // Example (already supported in code below): setoption name Depth value N
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

        if cmd.eq_ignore_ascii_case("quit") {
            break;
        }

        if cmd.eq_ignore_ascii_case("stop") {
            // We don't run a background search thread.
            // Nothing to stop; just ignore.
            continue;
        }

        // ---- SetOption (support a simple Depth override) ----
        // e.g. "setoption name Depth value 3"
        if let Some(rest) = cmd.strip_prefix("setoption ") {
            let mut name: Option<String> = None;
            let mut value: Option<String> = None;

            // Parse "name ... value ..."
            // We'll keep it simple and not handle quoted names.
            let toks: Vec<&str> = rest.split_whitespace().collect();
            let mut i = 0;
            while i < toks.len() {
                match toks[i].to_ascii_lowercase().as_str() {
                    "name" => {
                        i += 1;
                        // collect until we hit "value" or end
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
                }
            }
            continue;
        }

        // ---- Position command ----
        // position [startpos | fen <fen...>] [moves <m1> <m2> ...]
        if let Some(rest) = cmd.strip_prefix("position ") {
            let mut parts = rest.split_whitespace().peekable();
            // Set the base position
            if let Some(tok) = parts.next() {
                if tok.eq_ignore_ascii_case("startpos") {
                    b = Board::from_fen(START_FEN).unwrap();
                } else if tok.eq_ignore_ascii_case("fen") {
                    // collect 6 tokens
                    let mut fen_tokens = Vec::with_capacity(6);
                    for _ in 0..6 {
                        if let Some(t) = parts.next() {
                            fen_tokens.push(t);
                        }
                    }
                    b = Board::from_fen(&fen_tokens.join(" ")).unwrap();
                }
            }
            // Apply moves if present
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

        // ---- Go command ----
        if let Some(rest) = cmd.strip_prefix("go") {
            // Priority:
            //   1) go depth N
            //   2) go movetime M
            //   3) go wtime/btime ... (map to depth)
            //   4) fallback to default_depth
            let mut search_depth = default_depth;

            // depth N
            if let Some(d) = extract_u64(rest, "depth") {
                search_depth = d as usize;
            } else if let Some(ms) = extract_u64(rest, "movetime") {
                search_depth = depth_from_movetime_ms(ms, default_depth);
            } else {
                // consider clock-based if provided
                // (We don’t use winc/binc here; feel free to incorporate later.)
                if let Some(wtime) = extract_u64(rest, "wtime") {
                    // If we had side-to-move here, we could pick wtime/btime accordingly.
                    // For now, map whichever exists to a safe depth.
                    search_depth = depth_from_clock_ms(wtime, default_depth);
                }
                if let Some(btime) = extract_u64(rest, "btime") {
                    // pick the more conservative (min) of the two mappings
                    let alt = depth_from_clock_ms(btime, default_depth);
                    if alt < search_depth {
                        search_depth = alt;
                    }
                }
            }

            if let Some(m) = best_move_depth(&b, search_depth) {
                println!("bestmove {}", format_uci(m));
            } else {
                println!("bestmove 0000");
            }
            io::stdout().flush().ok();
            continue;
        }

        // Ignore any other commands quietly (like "ponderhit", "register", etc.)
    }
}

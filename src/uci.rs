use crate::board::Board;
use crate::opening_book::get_book_move;
use crate::search::{best_move_timed, extract_pv};
use crate::time::TimeControl;
use crate::tt::SharedTransTable;
use crate::types::{Color, START_FEN};
use crate::uci_io::{format_uci, parse_uci_move};
use num_cpus;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

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

struct PonderState {
    handle: Option<std::thread::JoinHandle<()>>,
    stop_signal: Option<Arc<AtomicBool>>,
}

impl PonderState {
    fn new() -> Self {
        Self {
            handle: None,
            stop_signal: None,
        }
    }

    fn stop_and_join(&mut self) {
        if let Some(sig) = &self.stop_signal {
            sig.store(true, Ordering::Relaxed);
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.stop_signal = None;
    }
}

pub fn run_uci() {
    let mut b = Board::from_fen(START_FEN).expect("valid startpos");
    let mut tc = TimeControl::default();

    let mut tt_size_mb: usize = 1024;
    let mut tt = SharedTransTable::new(tt_size_mb);
    let mut threads_count: usize = num_cpus::get().max(1);

    let mut ponder = PonderState::new();

    loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let cmd = line.trim();

        if cmd.eq_ignore_ascii_case("uci") {
            println!("id name Rusty-Improved");
            println!("id author Will");
            println!(
                "option name Hash type spin default {} min 1 max 4096",
                tt_size_mb
            );
            println!(
                "option name Threads type spin default {} min 1 max 128",
                threads_count
            );
            println!("option name Ponder type check default true");
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
            tt.clear();
            ponder.stop_and_join();
            io::stdout().flush().ok();
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("setoption ") {
            let mut parts = rest.split_whitespace();

            if let Some("name") = parts.next() {
                let name = parts.next().unwrap_or("");

                if let Some("value") = parts.next() {
                    let value = parts.next().unwrap_or("");

                    if name.eq_ignore_ascii_case("Hash") {
                        if let Ok(size) = value.parse::<usize>() {
                            if size > 0 && size <= 4096 && tt_size_mb != size {
                                tt_size_mb = size;
                                tt = SharedTransTable::new(tt_size_mb);
                            }
                        }
                    }

                    if name.eq_ignore_ascii_case("Threads") {
                        if let Ok(n) = value.parse::<usize>() {
                            if n > 0 && n <= 128 {
                                threads_count = n;
                            }
                        }
                    }
                }
            }
            continue;
        }

        if let Some(rest) = cmd.strip_prefix("position ") {
            ponder.stop_and_join();
            let mut parts = rest.split_whitespace();
            let first = parts.next().unwrap_or("");

            if first.eq_ignore_ascii_case("startpos") {
                b = Board::from_fen(START_FEN).unwrap();

                if let Some("moves") = parts.clone().next() {
                    parts.next();
                }
            } else if first.eq_ignore_ascii_case("fen") {
                let fen_parts: Vec<&str> = parts.by_ref().take(6).collect();
                b = Board::from_fen(&fen_parts.join(" ")).unwrap();

                if let Some("moves") = parts.clone().next() {
                    parts.next();
                }
            }

            for mv_str in parts {
                if let Some(mv) = parse_uci_move(&mut b, mv_str) {
                    let _ = b.make_move(mv);
                }
            }

            continue;
        }

        if cmd.eq_ignore_ascii_case("ponderhit") {
            continue;
        }

        if cmd.eq_ignore_ascii_case("stop") {
            ponder.stop_and_join();
            let pv = extract_pv(b.clone(), &tt, 1);

            if let Some(best) = pv.get(0).copied() {
                println!("bestmove {}", format_uci(best));
            }
            io::stdout().flush().ok();

            continue;
        }

        if let Some(rest) = cmd.strip_prefix("go") {
            ponder.stop_and_join();

            if let Some(book_uci) = get_book_move(&b) {
                let mut reply_pos = b.clone();
                if let Some(book_mv) = parse_uci_move(&mut reply_pos, book_uci) {
                    let _ = reply_pos.make_move(book_mv);

                    let ponder_uci = get_book_move(&reply_pos)
                        .and_then(|s| {
                            let mut tmp = reply_pos.clone();
                            parse_uci_move(&mut tmp, s).map(|_| s)
                        })
                        .map(|s| s.to_string());

                    let ponder_str = if let Some(pu) = ponder_uci {
                        pu
                    } else {
                        let stop = Arc::new(AtomicBool::new(false));
                        let (bm, _, _) =
                            best_move_timed(&reply_pos, &mut tt, 25, 6, Arc::clone(&stop), true);

                        if let Some(pmv) = bm {
                            format_uci(pmv)
                        } else {
                            println!("bestmove {}", book_uci);
                            io::stdout().flush().ok();
                            continue;
                        }
                    };

                    println!("bestmove {} ponder {}", book_uci, ponder_str);
                    io::stdout().flush().ok();
                    continue;
                } else {
                    println!("bestmove {}", book_uci);
                    io::stdout().flush().ok();
                    continue;
                }
            }

            let is_ponder = rest
                .split_whitespace()
                .any(|t| t.eq_ignore_ascii_case("ponder"));
            let depth = extract_i64(rest, "depth").map_or(128, |d| d as usize);

            tc.wtime = extract_i64(rest, "wtime").unwrap_or(0);
            tc.btime = extract_i64(rest, "btime").unwrap_or(0);
            tc.winc = extract_i64(rest, "winc").unwrap_or(0);
            tc.binc = extract_i64(rest, "binc").unwrap_or(0);
            tc.movestogo = extract_i64(rest, "movestogo").unwrap_or(0) as i32;

            let time_to_use = if is_ponder {
                u64::MAX / 4
            } else if let Some(movetime) = extract_i64(rest, "movetime") {
                movetime as u64
            } else {
                tc.allocation_ms(b.turn == Color::White).0 as u64
            };

            if is_ponder {
                let pos = b.clone();
                let mut tt_bg = tt.clone();
                let stop_signal = Arc::new(AtomicBool::new(false));
                let stop_clone = Arc::clone(&stop_signal);

                let handle = thread::spawn(move || {
                    let _ = best_move_timed(&pos, &mut tt_bg, time_to_use, depth, stop_clone, true);
                });

                ponder.handle = Some(handle);
                ponder.stop_signal = Some(stop_signal);
                continue;
            }

            let stop_signal = Arc::new(AtomicBool::new(false));
            let mut helpers = vec![];

            for _ in 0..(threads_count - 1) {
                let board_clone = b.clone();
                let tt_clone = tt.clone();
                let stop_clone = Arc::clone(&stop_signal);

                let handle = thread::spawn(move || {
                    let mut tt_local = tt_clone;
                    let _ = best_move_timed(
                        &board_clone,
                        &mut tt_local,
                        u64::MAX / 4,
                        depth,
                        stop_clone,
                        false,
                    );
                });
                helpers.push(handle);
            }

            let (best, reached_depth, _nodes) = best_move_timed(
                &b,
                &mut tt,
                time_to_use,
                depth,
                Arc::clone(&stop_signal),
                true,
            );

            stop_signal.store(true, Ordering::Relaxed);
            for h in helpers {
                let _ = h.join();
            }

            if let Some(m) = best {
                let pv = extract_pv(b.clone(), &tt, reached_depth.max(32));
                if let Some(pm) = pv.get(1).copied() {
                    println!("bestmove {} ponder {}", format_uci(m), format_uci(pm));
                } else {
                    println!("bestmove {}", format_uci(m));
                }
            } else {
                println!("bestmove 0000");
            }
            io::stdout().flush().ok();
            continue;
        }

        if cmd.eq_ignore_ascii_case("quit") {
            ponder.stop_and_join();
            break;
        }
    }
}

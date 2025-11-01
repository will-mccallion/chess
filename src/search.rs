use crate::board::Board;
use crate::eval::evaluate;
use crate::see::see;
use crate::tt::{Bound, SharedTransTable};
use crate::types::{Move, Piece, PieceKind};
use crate::uci_io::format_uci;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MATE_SCORE: i32 = 30_000;
const MATE_THRESHOLD: i32 = MATE_SCORE - 512;
const MAX_PLY: usize = 128;

const DRAW_SCORE: i32 = 0;

// Futility pruning margins
const FUTILITY_MARGIN: [i32; 4] = [0, 100, 250, 500];

// Move Ordering Scores
const TT_MOVE_SCORE: i32 = 1_000_000_000;
const GOOD_CAPTURE_SCORE: i32 = 900_000_000;
const KILLER_SCORE: i32 = 800_000_000;
const BAD_CAPTURE_SCORE: i32 = -1_000_000_000;

struct SearchController {
    start_time: Instant,
    time_budget: Duration,
    stop_signal: Arc<AtomicBool>,
    is_main_thread: bool,
    nodes: u64,
}

impl SearchController {
    fn time_is_up(&mut self) -> bool {
        if self.stop_signal.load(Ordering::Relaxed) {
            return true;
        }
        if self.is_main_thread
            && (self.nodes & 4095) == 0 // Check every 4096 nodes
            && self.start_time.elapsed() >= self.time_budget
        {
            self.stop_signal.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }
}

pub struct Search<'a> {
    board: Board,
    tt: &'a mut SharedTransTable,
    controller: SearchController,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 13], // [piece][to_square]
    ply: usize,
    seldepth: usize,
}

fn score_move(s: &Search, m: Move, tt_move: Option<Move>) -> i32 {
    if Some(m) == tt_move {
        return TT_MOVE_SCORE;
    }
    if m.capture {
        let see_val = see(&s.board, m);
        return if see_val >= 0 {
            GOOD_CAPTURE_SCORE + see_val
        } else {
            BAD_CAPTURE_SCORE + see_val
        };
    }
    if Some(m) == s.killers[s.ply][0] || Some(m) == s.killers[s.ply][1] {
        return KILLER_SCORE;
    }
    let piece_idx = s.board.piece_on[m.from as usize].index();
    s.history[piece_idx][m.to as usize]
}

fn quiesce(s: &mut Search, mut alpha: i32, beta: i32) -> i32 {
    s.seldepth = s.seldepth.max(s.ply);
    s.controller.nodes += 1;
    if s.controller.time_is_up() {
        return 0;
    }

    let king_sq_opt =
        s.board.piece_bb[Piece::from_kind(PieceKind::King, s.board.turn).index()].trailing_zeros();
    let king_sq = if king_sq_opt < 64 {
        king_sq_opt as i32
    } else {
        -1
    };
    let in_check = king_sq != -1 && s.board.is_square_attacked(king_sq, s.board.turn.other());

    if !in_check {
        let stand_pat = evaluate(&s.board);
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }
    }

    let mut moves = Vec::with_capacity(64);
    s.board.generate_pseudo_legal_moves(&mut moves);

    let mut scored_moves: Vec<(i32, Move)> = moves
        .into_iter()
        .filter(|m| in_check || m.capture)
        .map(|m| (score_move(s, m, None), m))
        .collect();

    scored_moves.sort_unstable_by_key(|(score, _)| -*score);

    for (_, m) in scored_moves {
        if !in_check && m.capture && see(&s.board, m) < 0 {
            continue;
        }

        let undo = s.board.make_move(m);

        let us = s.board.turn.other();
        let king_bb = s.board.piece_bb[Piece::from_kind(PieceKind::King, us).index()];
        if king_bb != 0 {
            let current_king_sq = king_bb.trailing_zeros() as i32;
            if s.board.is_square_attacked(current_king_sq, s.board.turn) {
                s.board.unmake_move(m, undo);
                continue;
            }
        } else {
            s.board.unmake_move(m, undo);
            continue;
        }

        s.ply += 1;
        let score = -quiesce(s, -beta, -alpha);
        s.ply -= 1;
        s.board.unmake_move(m, undo);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    if in_check && alpha == -MATE_SCORE {
        return -MATE_SCORE + s.ply as i32;
    }

    alpha
}

fn negamax(s: &mut Search, mut alpha: i32, beta: i32, mut depth: i32) -> i32 {
    s.seldepth = s.seldepth.max(s.ply);
    if s.controller.time_is_up() {
        return 0;
    }
    if s.ply > 0 && (s.board.is_draw_by_repetition() || s.board.halfmove_clock >= 100) {
        return DRAW_SCORE;
    }
    if s.ply >= MAX_PLY - 1 {
        return evaluate(&s.board);
    }

    let is_pv = beta - alpha > 1;
    let alpha_orig = alpha;
    let key = s.board.zobrist;

    if let Some(entry) = s.tt.probe(key)
        && entry.depth() >= depth as i16
        && s.ply > 0
    {
        let score = entry.score();
        match entry.bound() {
            Bound::Exact => return score,
            Bound::Lower if score >= beta => return score,
            Bound::Upper if score <= alpha => return score,
            _ => {}
        }
    }

    let king_sq_opt =
        s.board.piece_bb[Piece::from_kind(PieceKind::King, s.board.turn).index()].trailing_zeros();
    let king_sq = if king_sq_opt < 64 {
        king_sq_opt as i32
    } else {
        -1
    };
    let in_check = king_sq != -1 && s.board.is_square_attacked(king_sq, s.board.turn.other());

    if in_check {
        depth += 1;
    }

    if depth <= 0 {
        return quiesce(s, alpha, beta);
    }

    s.controller.nodes += 1;

    if !is_pv && !in_check && depth < 4 {
        let eval = evaluate(&s.board);
        if eval - FUTILITY_MARGIN[depth as usize] >= beta {
            return beta;
        }
    }

    if !is_pv && !in_check && depth >= 3 {
        let undo = s.board.make_null_move();
        s.ply += 1;
        let null_score = -negamax(s, -beta, -beta + 1, depth - 3);
        s.ply -= 1;
        s.board.unmake_null_move(undo);
        if null_score >= beta {
            return beta;
        }
    }

    let mut moves = Vec::with_capacity(128);
    s.board.generate_pseudo_legal_moves(&mut moves);

    let tt_move = s.tt.probe(key).and_then(|e| e.best_move());
    let mut scored_moves: Vec<(i32, Move)> = moves
        .iter()
        .map(|&m| (score_move(s, m, tt_move), m))
        .collect();
    scored_moves.sort_unstable_by_key(|(score, _)| -*score);

    let mut best_score = -MATE_SCORE;
    let mut best_move: Option<Move> = None;
    let mut moves_searched = 0;

    for (_, m) in scored_moves {
        let undo = s.board.make_move(m);

        let us = s.board.turn.other();
        let king_bb = s.board.piece_bb[Piece::from_kind(PieceKind::King, us).index()];
        if king_bb == 0 {
            s.board.unmake_move(m, undo);
            continue;
        }
        let current_king_sq = king_bb.trailing_zeros() as i32;
        if s.board.is_square_attacked(current_king_sq, s.board.turn) {
            s.board.unmake_move(m, undo);
            continue;
        }

        s.ply += 1;
        moves_searched += 1;

        let score = if moves_searched == 1 {
            -negamax(s, -beta, -alpha, depth - 1)
        } else {
            let reduction = if depth >= 3 && !m.capture && !in_check {
                1 + if moves_searched > 4 { 1 } else { 0 }
            } else {
                0
            };
            let mut search_score = -negamax(s, -alpha - 1, -alpha, depth - 1 - reduction);
            if search_score > alpha && reduction > 0 {
                search_score = -negamax(s, -alpha - 1, -alpha, depth - 1);
            }
            if search_score > alpha && search_score < beta {
                -negamax(s, -beta, -alpha, depth - 1)
            } else {
                search_score
            }
        };

        s.ply -= 1;
        s.board.unmake_move(m, undo);

        if s.controller.time_is_up() {
            return 0;
        }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    if !m.capture {
                        if Some(m) != s.killers[s.ply][0] {
                            s.killers[s.ply][1] = s.killers[s.ply][0];
                            s.killers[s.ply][0] = Some(m);
                        }
                        let piece_idx = s.board.piece_on[m.from as usize].index();
                        s.history[piece_idx][m.to as usize] += depth * depth;
                    }
                    break;
                }
            }
        }
    }

    if moves_searched == 0 {
        return if in_check {
            -MATE_SCORE + s.ply as i32
        } else {
            DRAW_SCORE
        };
    }

    let bound = if best_score <= alpha_orig {
        Bound::Upper
    } else if best_score >= beta {
        Bound::Lower
    } else {
        Bound::Exact
    };
    s.tt.store(key, depth as i16, best_score, bound, best_move);

    best_score
}

pub fn extract_pv(mut pos: Board, tt: &SharedTransTable, max_len: usize) -> Vec<Move> {
    let mut pv = Vec::with_capacity(max_len);
    for _ in 0..max_len {
        if let Some(m) = tt.probe(pos.zobrist).and_then(|e| e.best_move()) {
            let mut legal_moves = Vec::new();
            pos.generate_legal_moves(&mut legal_moves);
            if legal_moves.contains(&m) {
                pv.push(m);
                pos.make_move(m);
                continue;
            }
        }
        break;
    }
    pv
}

pub fn best_move_timed(
    b: &Board,
    tt: &mut SharedTransTable,
    time_ms: u64,
    max_depth: usize,
    stop_signal: Arc<AtomicBool>,
    is_main_thread: bool,
) -> (Option<Move>, usize, u64) {
    if is_main_thread {
        tt.tick_age();
    }

    let mut search = Search {
        board: b.clone(),
        tt,
        controller: SearchController {
            start_time: Instant::now(),
            time_budget: Duration::from_millis(time_ms),
            stop_signal,
            is_main_thread,
            nodes: 0,
        },
        killers: [[None; 2]; MAX_PLY],
        history: [[0; 64]; 13],
        ply: 0,
        seldepth: 0,
    };

    let mut best_move: Option<Move> = None;
    let mut score = 0;

    for d in 1..=max_depth {
        search.seldepth = 0;
        let mut alpha = -MATE_SCORE;
        let mut beta = MATE_SCORE;

        if d > 1 {
            alpha = score - 30;
            beta = score + 30;
        }

        loop {
            score = negamax(&mut search, alpha, beta, d as i32);
            if search.controller.time_is_up() {
                break;
            }
            if score <= alpha {
                alpha = -MATE_SCORE; // Search failed low, must widen search
                beta = score + 1; // Update beta to narrow the window from above
            } else if score >= beta {
                beta = MATE_SCORE; // Search failed high, must widen search
                alpha = score - 1; // Update alpha to narrow the window from below
            } else {
                break; // Success
            }
        }

        if search.controller.time_is_up() {
            break;
        }

        if let Some(entry) = search.tt.probe(search.board.zobrist) {
            best_move = entry.best_move();
        }

        if is_main_thread {
            let elapsed_ms = search.controller.start_time.elapsed().as_millis();
            let nps = if elapsed_ms > 0 {
                (search.controller.nodes * 1000) / elapsed_ms as u64
            } else {
                0
            };
            let hashfull = search.tt.hashfull_permill();

            let pv = extract_pv(search.board.clone(), search.tt, d);
            let pv_str = pv
                .iter()
                .map(|&m| format_uci(m))
                .collect::<Vec<_>>()
                .join(" ");
            let score_str = if score.abs() > MATE_THRESHOLD {
                let mate_in = (MATE_SCORE - score.abs() + 1) / 2;
                format!("mate {}", if score > 0 { mate_in } else { -mate_in })
            } else {
                format!("cp {}", score)
            };

            println!(
                "info depth {} seldepth {} score {} hashfull {} nodes {} nps {} time {} pv {}",
                d,
                search.seldepth,
                score_str,
                hashfull,
                search.controller.nodes,
                nps,
                elapsed_ms,
                pv_str
            );
        }

        if score.abs() > MATE_THRESHOLD {
            break;
        }
    }

    (best_move, max_depth, search.controller.nodes)
}

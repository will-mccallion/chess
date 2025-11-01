use crate::board::Board;
use crate::eval::evaluate;
use crate::see::see;
use crate::tt::{Bound, SharedTransTable};
use crate::types::{Color, Move, Piece};
use crate::uci_io::format_uci;
use std::cmp::{max, min};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MATE_SCORE: i32 = 30_000;
const MATE_THRESHOLD: i32 = MATE_SCORE - 512;
const MAX_PLY: usize = 256;

const DRAW_CP: i32 = 0;
const REPETITION_CONTEMPT: i32 = 20;

// Margins for futility pruning, indexed by depth
const FUTILITY_MARGINS: [i32; 4] = [0, 200, 500, 750];

// Quiescence hardening
const MAX_QPLY: usize = 64; // absolute QS recursion cap (belt/suspenders)
const QS_CHECKS: usize = 2; // allow up to 2 plies of quiet-check expansion

#[inline]
fn repetition_score(static_eval: i32) -> i32 {
    if static_eval > 50 {
        -REPETITION_CONTEMPT
    } else if static_eval < -50 {
        REPETITION_CONTEMPT
    } else {
        DRAW_CP
    }
}

struct SearchCtrl {
    start: Instant,
    time_budget: Duration,
    stop_signal: Arc<AtomicBool>,
    is_main_thread: bool,
}

impl SearchCtrl {
    fn new(time_ms: u64, stop_signal: Arc<AtomicBool>, is_main_thread: bool) -> Self {
        Self {
            start: Instant::now(),
            time_budget: Duration::from_millis(time_ms),
            stop_signal,
            is_main_thread,
        }
    }

    #[inline]
    fn time_up(&mut self, nodes: u64) -> bool {
        if self.stop_signal.load(Ordering::Relaxed) {
            return true;
        }
        if self.is_main_thread && (nodes & 4095) == 0 {
            if self.start.elapsed() >= self.time_budget {
                self.stop_signal.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    }
}

pub struct Search<'a> {
    ctrl: SearchCtrl,
    tt: &'a mut SharedTransTable,
    nodes: u64,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 13],
    ply: usize,
    moves: Vec<Move>,
}

impl<'a> Search<'a> {
    fn new(
        time_ms: u64,
        tt: &'a mut SharedTransTable,
        stop_signal: Arc<AtomicBool>,
        is_main_thread: bool,
    ) -> Self {
        Self {
            ctrl: SearchCtrl::new(time_ms, stop_signal, is_main_thread),
            tt,
            nodes: 0,
            killers: [[None; 2]; MAX_PLY],
            history: [[0; 64]; 13],
            ply: 0,
            moves: Vec::with_capacity(256),
        }
    }

    #[inline]
    fn store_killer(&mut self, m: Move) {
        if self.ply < MAX_PLY {
            if self.killers[self.ply][0] != Some(m) {
                self.killers[self.ply][1] = self.killers[self.ply][0];
                self.killers[self.ply][0] = Some(m);
            }
        }
    }
}

fn to_uci_score(s: i32, ply: usize) -> String {
    let adjusted = if s > MATE_THRESHOLD {
        s - ply as i32
    } else if s < -MATE_THRESHOLD {
        s + ply as i32
    } else {
        s
    };
    if adjusted.abs() > MATE_THRESHOLD {
        let plies_to_mate = MATE_SCORE - adjusted.abs();
        let moves_to_mate = (plies_to_mate + 1) / 2;
        if s > 0 {
            format!("mate {}", moves_to_mate)
        } else {
            format!("mate -{}", moves_to_mate)
        }
    } else {
        format!("cp {}", s)
    }
}

#[inline]
fn score_move(
    b: &Board,
    m: Move,
    tt_move: Option<Move>,
    killers: &[Option<Move>; 2],
    history: &[[i32; 64]; 13],
) -> i32 {
    if Some(m) == tt_move {
        return 2_000_000;
    }
    if m.capture {
        return 1_000_000 + see(b, m);
    }
    if Some(m) == killers[0] {
        return 900_000;
    }
    if Some(m) == killers[1] {
        return 850_000;
    }
    let piece = b.piece_on[m.from as usize];
    history[piece.index()][m.to as usize]
}

#[inline]
fn is_in_check(b: &Board) -> bool {
    let king = if b.turn == Color::White {
        Piece::WK
    } else {
        Piece::BK
    };
    let ksq = b.piece_bb[king.index()].trailing_zeros() as i32;
    ksq != 64 && b.is_square_attacked(ksq, b.turn.other())
}

fn quiesce(b: &mut Board, mut alpha: i32, beta: i32, s: &mut Search, qply: usize) -> i32 {
    if (s.nodes & 4095) == 0 && s.ctrl.time_up(s.nodes) {
        return 0;
    }
    s.nodes += 1;

    if s.ply >= MAX_PLY.saturating_sub(2) || qply >= MAX_QPLY {
        let sp = evaluate(b);
        if sp >= beta {
            return beta;
        }
        if sp > alpha {
            alpha = sp;
        }
        return alpha;
    }

    let in_check = is_in_check(b);

    if in_check {
        let mut legal = Vec::with_capacity(64);
        b.generate_legal_moves(&mut legal);
        if legal.is_empty() {
            return -MATE_SCORE + s.ply as i32;
        }

        let mut scored: Vec<(i32, Move)> = Vec::with_capacity(legal.len());
        for &m in &legal {
            let sc = if m.capture { 10_000 + see(b, m) } else { 0 };
            scored.push((sc, m));
        }
        scored.sort_by_key(|(sc, _)| -sc);

        for (_, m) in scored {
            let u = b.make_move(m);
            s.ply += 1;
            let score = -quiesce(b, -beta, -alpha, s, qply + 1);
            s.ply -= 1;
            b.unmake_move(m, u);

            if s.ctrl.stop_signal.load(Ordering::Relaxed) {
                return 0;
            }
            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }
        return alpha;
    }

    let stand_pat = evaluate(b);
    if stand_pat >= beta {
        return beta;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    const QUEEN_VAL: i32 = 900;
    if stand_pat + QUEEN_VAL < alpha {
        return alpha;
    }

    let mut moves = Vec::with_capacity(64);
    b.generate_legal_moves(&mut moves);

    {
        let mut scored: Vec<(i32, Move)> = Vec::new();
        scored.reserve(moves.len());

        for &m in &moves {
            if m.capture {
                let see_score = see(b, m);
                if see_score < 0 {
                    continue;
                }
                scored.push((100_000 + see_score, m));
            } else if m.promotion.is_some() {
                scored.push((50_000, m));
            }
        }

        scored.sort_by_key(|(sc, _)| -sc);

        for (_, m) in scored {
            let u = b.make_move(m);
            s.ply += 1;
            let score = -quiesce(b, -beta, -alpha, s, qply + 1);
            s.ply -= 1;
            b.unmake_move(m, u);

            if s.ctrl.stop_signal.load(Ordering::Relaxed) {
                return 0;
            }
            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }
    }

    if qply < QS_CHECKS {
        let mut checkers: Vec<Move> = Vec::new();
        for &m in &moves {
            if m.capture || m.promotion.is_some() || m.castle || m.en_passant {
                continue;
            }

            let u = b.make_move(m);
            let opp_king = if b.turn == Color::White {
                Piece::WK
            } else {
                Piece::BK
            };
            let opp_ksq = b.piece_bb[opp_king.index()].trailing_zeros() as i32;
            let gives_check = opp_ksq != 64 && b.is_square_attacked(opp_ksq, b.turn.other());
            b.unmake_move(m, u);

            if !gives_check {
                continue;
            }

            const QCHECK_MARGIN: i32 = 150;
            if stand_pat + QCHECK_MARGIN <= alpha {
                continue;
            }

            checkers.push(m);
        }

        if !checkers.is_empty() {
            let killers = if s.ply < MAX_PLY {
                s.killers[s.ply]
            } else {
                [None; 2]
            };
            let tt_move = s.tt.probe(b.zobrist).and_then(|e| e.best_move());
            let mut scored: Vec<(i32, Move)> = checkers
                .into_iter()
                .map(|m| (score_move(b, m, tt_move, &killers, &s.history), m))
                .collect();
            scored.sort_by_key(|(sc, _)| -*sc);

            for (_, m) in scored {
                let u = b.make_move(m);
                s.ply += 1;
                let score = -quiesce(b, -beta, -alpha, s, qply + 1);
                s.ply -= 1;
                b.unmake_move(m, u);

                if s.ctrl.stop_signal.load(Ordering::Relaxed) {
                    return 0;
                }
                if score >= beta {
                    return beta;
                }
                if score > alpha {
                    alpha = score;
                }
            }
        }
    }

    alpha
}

fn negamax(
    b: &mut Board,
    mut alpha: i32,
    mut beta: i32,
    mut depth: i32,
    s: &mut Search,
    is_pv: bool,
) -> i32 {
    if s.ctrl.time_up(s.nodes) {
        return 0;
    }

    if s.ply >= MAX_PLY.saturating_sub(2) {
        return quiesce(b, alpha, beta, s, 0);
    }

    if s.ply > 0 && (b.halfmove_clock >= 100 || b.is_draw_by_repetition()) {
        return repetition_score(evaluate(b));
    }

    let is_root = s.ply == 0;
    let alpha_orig = alpha;

    alpha = max(alpha, -MATE_SCORE + s.ply as i32);
    beta = min(beta, MATE_SCORE - s.ply as i32);
    if alpha >= beta {
        return alpha;
    }

    let king = if b.turn == Color::White {
        Piece::WK
    } else {
        Piece::BK
    };
    let ksq = b.piece_bb[king.index()].trailing_zeros() as i32;
    let in_check = ksq != 64 && b.is_square_attacked(ksq, b.turn.other());

    if in_check {
        depth += 1; // check extension
    }
    if depth <= 0 {
        return quiesce(b, alpha, beta, s, 0);
    }

    let zobrist_key = b.zobrist;
    if !is_root {
        if let Some(e) = s.tt.probe(zobrist_key) {
            if e.depth() >= depth as i16 {
                let score = e.score();
                match e.bound() {
                    Bound::Exact => return score,
                    Bound::Lower if score >= beta => return score,
                    Bound::Upper if score <= alpha => return score,
                    _ => {}
                }
            }
        }
    }

    if !is_pv && !in_check && depth < 4 {
        let eval = evaluate(b);
        if eval + FUTILITY_MARGINS[depth as usize] <= alpha {
            return alpha;
        }
    }

    if !in_check && depth >= 3 && s.ply > 0 && !is_pv {
        let u = b.make_null_move();
        s.ply += 1;
        let score = -negamax(b, -beta, -beta + 1, depth - 1 - 2, s, false);
        s.ply -= 1;
        b.unmake_null_move(u);
        if score >= beta {
            return beta;
        }
    }

    s.nodes += 1;
    s.moves.clear();
    b.generate_legal_moves(&mut s.moves);
    if s.moves.is_empty() {
        return if in_check {
            -MATE_SCORE + s.ply as i32
        } else {
            DRAW_CP
        };
    }

    let tt_move = s.tt.probe(zobrist_key).and_then(|e| e.best_move());
    let killers = if s.ply < MAX_PLY {
        s.killers[s.ply]
    } else {
        [None; 2]
    };
    let mut scored_moves: Vec<_> = s
        .moves
        .iter()
        .map(|&m| (score_move(b, m, tt_move, &killers, &s.history), m))
        .collect();
    scored_moves.sort_by_key(|(score, _)| -*score);

    let mut best_score = -MATE_SCORE;
    let mut best_move: Option<Move> = None;

    for (i, (_, m)) in scored_moves.iter().enumerate() {
        let u = b.make_move(*m);
        s.ply += 1;

        let score: i32;
        if i == 0 {
            score = -negamax(b, -beta, -alpha, depth - 1, s, true);
        } else {
            // LMR
            let reduction =
                if depth >= 3 && i >= 3 && !m.capture && m.promotion.is_none() && !in_check {
                    if i >= 6 || depth > 6 { 2 } else { 1 }
                } else {
                    0
                };

            let mut current = -negamax(b, -alpha - 1, -alpha, depth - 1 - reduction, s, false);
            if current > alpha && reduction > 0 {
                current = -negamax(b, -alpha - 1, -alpha, depth - 1, s, false);
            }
            if current > alpha && current < beta {
                score = -negamax(b, -beta, -alpha, depth - 1, s, true);
            } else {
                score = current;
            }
        }

        s.ply -= 1;
        b.unmake_move(*m, u);

        if s.ctrl.time_up(s.nodes) {
            return 0;
        }
        if score > best_score {
            best_score = score;
            best_move = Some(*m);
        }
        if best_score > alpha {
            alpha = best_score;
        }
        if alpha >= beta {
            if !m.capture {
                s.history[b.piece_on[m.from as usize].index()][m.to as usize] += depth * depth;
                s.store_killer(*m);
            }
            s.tt.store(
                zobrist_key,
                depth as i16,
                best_score,
                Bound::Lower,
                best_move,
            );
            return best_score;
        }
    }

    let bound = if best_score <= alpha_orig {
        Bound::Upper
    } else {
        Bound::Exact
    };
    s.tt.store(zobrist_key, depth as i16, best_score, bound, best_move);
    best_score
}

pub fn extract_pv(mut pos: Board, tt: &SharedTransTable, max_len: usize) -> Vec<Move> {
    let mut pv = Vec::with_capacity(max_len);
    for _ in 0..max_len {
        if let Some(e) = tt.probe(pos.zobrist) {
            if let Some(m) = e.best_move() {
                let mut legal_moves = Vec::new();
                pos.generate_legal_moves(&mut legal_moves);
                if legal_moves.contains(&m) {
                    pv.push(m);
                    let _u = pos.make_move(m);
                    continue;
                }
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
    let mut pos = b.clone();
    if is_main_thread {
        tt.tick_age();
    }
    let mut search = Search::new(time_ms, tt, stop_signal, is_main_thread);

    let mut best_move: Option<Move> = None;
    let mut reached_depth = 0;
    let mut score = 0;

    for d in 1..=max_depth {
        let mut alpha = -MATE_SCORE;
        let mut beta = MATE_SCORE;
        if d > 3 {
            alpha = score - 50;
            beta = score + 50;
        }

        loop {
            score = negamax(&mut pos, alpha, beta, d as i32, &mut search, true);
            if search.ctrl.time_up(search.nodes) {
                break;
            }
            if score <= alpha {
                alpha = -MATE_SCORE;
            } else if score >= beta {
                beta = MATE_SCORE;
            } else {
                break;
            }
        }

        if search.ctrl.time_up(search.nodes) && d > 1 {
            break;
        }
        if !is_main_thread && search.ctrl.stop_signal.load(Ordering::Relaxed) {
            break;
        }

        reached_depth = d;
        if let Some(e) = search.tt.probe(pos.zobrist) {
            best_move = e.best_move();
        }

        if is_main_thread {
            let elapsed_ms = search.ctrl.start.elapsed().as_millis() as u64;
            let nps = if elapsed_ms > 0 {
                (search.nodes * 1000) / elapsed_ms
            } else {
                0
            };
            let hashfull = search.tt.hashfull_permill();
            let pv = extract_pv(pos.clone(), search.tt, d);
            let pv_str = pv
                .iter()
                .map(|&m| format_uci(m))
                .collect::<Vec<_>>()
                .join(" ");
            let info_score = to_uci_score(score, search.ply);
            println!(
                "info depth {} score {} nodes {} nps {} time {} hashfull {} pv {}",
                d, info_score, search.nodes, nps, elapsed_ms, hashfull, pv_str
            );
        }

        if score.abs() > MATE_THRESHOLD {
            break;
        }
    }
    (best_move, reached_depth, search.nodes)
}

pub fn best_move_depth(b: &Board, tt: &mut SharedTransTable, depth: usize) -> Option<Move> {
    let stop_signal = Arc::new(AtomicBool::new(false));
    let (m, _, _) = best_move_timed(b, tt, u64::MAX / 4, depth, stop_signal, true);
    m
}

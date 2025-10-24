use crate::board::Board;
use crate::eval::evaluate;
use crate::see::see;
use crate::tt::{Bound, SharedTransTable};
use crate::types::{Color, Move, Piece, PieceKind};
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
const REPEAT_AVOID_MARGIN: i32 = 80;

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

struct Search<'a> {
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
    let adjusted_score = if s > MATE_THRESHOLD {
        s - ply as i32
    } else if s < -MATE_THRESHOLD {
        s + ply as i32
    } else {
        s
    };

    if adjusted_score.abs() > MATE_THRESHOLD {
        let plies_to_mate = MATE_SCORE - adjusted_score.abs();
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

const PIECE_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 20000];

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
        let attacker = b.piece_on[m.from as usize].kind().unwrap();
        let victim = if m.en_passant {
            PieceKind::Pawn
        } else {
            b.piece_on[m.to as usize].kind().unwrap_or(PieceKind::Pawn)
        };
        return 1_000_000 + (PIECE_VALUES[victim as usize] * 10) - PIECE_VALUES[attacker as usize];
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

fn quiesce(b: &mut Board, mut alpha: i32, beta: i32, s: &mut Search) -> i32 {
    if (s.nodes & 4095) == 0 && s.ctrl.time_up(s.nodes) {
        return 0;
    }
    s.nodes += 1;

    let stand_pat = evaluate(b);
    if stand_pat >= beta {
        return beta;
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    s.moves.clear();
    b.generate_legal_moves(&mut s.moves);

    let mut scored_moves = Vec::with_capacity(s.moves.len());
    for &m in s.moves.iter() {
        if !m.capture && m.promotion.is_none() {
            continue;
        }
        if m.capture && see(b, m) < 0 {
            continue;
        }
        let score = score_move(b, m, None, &[None, None], &s.history);
        scored_moves.push((score, m));
    }
    scored_moves.sort_by_key(|(score, _)| -*score);

    for (_, m) in scored_moves {
        let u = b.make_move(m);
        s.ply += 1;
        let score = -quiesce(b, -beta, -alpha, s);
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

    let stand_pat_here = evaluate(b);

    if s.ply > 0 {
        if b.halfmove_clock >= 100 {
            return DRAW_CP;
        }
        if b.is_draw_by_repetition() {
            return repetition_score(stand_pat_here);
        }
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
        depth += 1;
    }
    if depth <= 0 {
        return quiesce(b, alpha, beta, s);
    }

    s.nodes += 1;

    let zobrist_key = b.zobrist;
    if !is_root {
        if let Some(e) = s.tt.probe(zobrist_key) {
            if e.depth >= depth as i16 {
                let score = e.score;
                if e.bound == Bound::Exact {
                    return score;
                }
                if e.bound == Bound::Lower && score >= beta {
                    return score;
                }
                if e.bound == Bound::Upper && score <= alpha {
                    return score;
                }
            }
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

    s.moves.clear();
    b.generate_legal_moves(&mut s.moves);
    if s.moves.is_empty() {
        return if in_check {
            -MATE_SCORE + s.ply as i32
        } else {
            DRAW_CP
        };
    }

    let tt_move = s.tt.probe(zobrist_key).and_then(|e| e.best_move);
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
    let mut moves_searched = 0;

    for (_, m) in scored_moves {
        if !in_check && !is_pv && depth <= 3 && m.capture && see(b, m) < 0 {
            continue;
        }

        let reduction = if depth >= 3 && moves_searched >= 3 && !m.capture && m.promotion.is_none()
        {
            if moves_searched >= 6 { 2 } else { 1 }
        } else {
            0
        };

        let u = b.make_move(m);
        s.ply += 1;

        let score: i32;

        if b.is_draw_by_repetition() {
            if s.ply == 1 && stand_pat_here > REPEAT_AVOID_MARGIN {
                score = -MATE_SCORE + 1;
            } else {
                score = -repetition_score(stand_pat_here);
            }
        } else {
            if moves_searched == 0 {
                score = -negamax(b, -beta, -alpha, depth - 1, s, true);
            } else {
                let mut s1 = -negamax(b, -alpha - 1, -alpha, depth - 1 - reduction, s, false);
                if s1 > alpha && s1 < beta {
                    s1 = -negamax(b, -beta, -alpha, depth - 1, s, false);
                }
                score = s1;
            }
        }

        s.ply -= 1;
        b.unmake_move(m, u);

        if s.ctrl.time_up(s.nodes) {
            return 0;
        }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if best_score > alpha {
            alpha = best_score;
        }

        if alpha >= beta {
            if !m.capture {
                s.history[b.piece_on[m.from as usize].index()][m.to as usize] += depth * depth;
                s.store_killer(m);
            }

            if best_score != DRAW_CP
                && best_score != REPETITION_CONTEMPT
                && best_score != -REPETITION_CONTEMPT
                && best_score != (-MATE_SCORE + 1)
            {
                s.tt.store(
                    zobrist_key,
                    depth as i16,
                    best_score,
                    Bound::Lower,
                    best_move,
                );
            }

            return best_score;
        }

        moves_searched += 1;
    }

    let bound = if best_score <= alpha_orig {
        Bound::Upper
    } else {
        Bound::Exact
    };

    if best_score != DRAW_CP
        && best_score != REPETITION_CONTEMPT
        && best_score != -REPETITION_CONTEMPT
        && best_score != (-MATE_SCORE + 1)
    {
        s.tt.store(zobrist_key, depth as i16, best_score, bound, best_move);
    }

    best_score
}

pub fn extract_pv(mut pos: Board, tt: &SharedTransTable, max_len: usize) -> Vec<Move> {
    let mut pv = Vec::with_capacity(max_len);
    for _ in 0..max_len {
        if let Some(e) = tt.probe(pos.zobrist) {
            if let Some(m) = e.best_move {
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
            best_move = e.best_move;
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

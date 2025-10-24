use crate::board::Board;
use crate::eval::evaluate;
use crate::tt::{Bound, TransTable};
use crate::types::{Color, Move, Piece, PieceKind};
use crate::uci_io::format_uci;
use std::cmp::{max, min};
use std::time::{Duration, Instant};

pub const MATE_SCORE: i32 = 30_000;
const MATE_THRESHOLD: i32 = MATE_SCORE - 512; // A score indicating a mate is near
const MAX_PLY: usize = 128; // Maximum search depth in plies

/// Manages time control for the search.
struct SearchCtrl {
    start: Instant,
    time_budget: Duration,
    stop: bool,
}
impl SearchCtrl {
    fn new(time_ms: u64) -> Self {
        Self {
            start: Instant::now(),
            time_budget: Duration::from_millis(time_ms),
            stop: false,
        }
    }
    /// Checks if the allocated time is up. Checked periodically.
    #[inline]
    fn time_up(&mut self, nodes: u64) -> bool {
        if self.stop {
            return true;
        }
        // Only check the time every 4096 nodes to reduce overhead
        if (nodes & 4095) == 0 {
            if self.start.elapsed() >= self.time_budget {
                self.stop = true;
            }
        }
        self.stop
    }
}

/// Holds all search-related state.
struct Search {
    ctrl: SearchCtrl,
    tt: TransTable,
    nodes: u64,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 13], // [piece][to_square] heuristic
    ply: usize,
}

impl Search {
    fn new(time_ms: u64, tt_size_mb: usize) -> Self {
        Self {
            ctrl: SearchCtrl::new(time_ms),
            tt: TransTable::with_mb(tt_size_mb),
            nodes: 0,
            killers: [[None; 2]; MAX_PLY],
            history: [[0; 64]; 13],
            ply: 0,
        }
    }
    /// Stores a "killer move" for the current ply.
    fn store_killer(&mut self, m: Move) {
        if self.ply >= MAX_PLY {
            return;
        }
        if self.killers[self.ply][0] != Some(m) {
            self.killers[self.ply][1] = self.killers[self.ply][0];
            self.killers[self.ply][0] = Some(m);
        }
    }
}

/// Formats the search score for UCI output (e.g., "cp 120" or "mate 5").
fn to_uci_score(s: i32, ply: usize) -> String {
    // Adjust mate scores to be relative to the root, not the current ply
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

const PIECE_VALUES: [i32; 7] = [100, 320, 330, 500, 900, 20000, 0]; // P, N, B, R, Q, K, Empty

/// Scores a move to guide move ordering.
fn score_move(
    b: &Board,
    m: Move,
    tt_move: Option<Move>,
    killers: &[Option<Move>; 2],
    history: &[[i32; 64]; 13],
) -> i32 {
    if Some(m) == tt_move {
        return 1_000_000;
    }
    if m.capture {
        // Most Valuable Victim - Least Valuable Aggressor (MVV-LVA)
        let attacker = b.piece_on[m.from as usize].kind().unwrap();
        let victim = if m.en_passant {
            PieceKind::Pawn
        } else {
            // If the 'to' square is empty, it must be an en passant capture
            b.piece_on[m.to as usize].kind().unwrap_or(PieceKind::Pawn)
        };
        return 900_000 + PIECE_VALUES[victim as usize] * 10 - PIECE_VALUES[attacker as usize];
    }
    if Some(m) == killers[0] {
        return 800_000;
    }
    if Some(m) == killers[1] {
        return 700_000;
    }
    // For quiet moves, use the history heuristic score
    let piece = b.piece_on[m.from as usize];
    history[piece.index()][m.to as usize]
}

/// Helper function to determine if a move is a check.
/// This is done by making the move, checking if the opponent's king is attacked,
/// and then unmaking the move.
fn is_check(b: &mut Board, m: Move) -> bool {
    let u = b.make_move(m);
    let king = if b.turn == Color::White {
        Piece::WK
    } else {
        Piece::BK
    };
    let ksq = b.piece_bb[king.index()].trailing_zeros() as i32;
    let is_in_check = ksq != 64 && b.is_square_attacked(ksq, b.turn.other());
    b.unmake_move(u);
    is_in_check
}

/// Quiescence search to stabilize the evaluation at the search horizon.
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

    // Delta Pruning
    let big_piece_value = PIECE_VALUES[PieceKind::Queen as usize];
    if stand_pat < alpha - big_piece_value {
        return alpha;
    }

    let mut moves = Vec::with_capacity(64);
    b.generate_legal_moves(&mut moves);

    // In quiescence, we only consider "loud" moves: captures, promotions, and checks.
    let mut scored_moves = Vec::with_capacity(moves.len());
    for m in moves {
        if m.capture || m.promotion.is_some() || is_check(b, m) {
            let score = score_move(b, m, None, &[None, None], &s.history);
            scored_moves.push((score, m));
        }
    }

    scored_moves.sort_by_key(|(score, _)| -*score);

    for (_, m) in scored_moves {
        let u = b.make_move(m);
        s.ply += 1;
        let score = -quiesce(b, -beta, -alpha, s);
        s.ply -= 1;
        b.unmake_move(u);

        if s.ctrl.stop {
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

/// The main negamax search function with alpha-beta pruning.
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

    // Check for draws by rule. These are terminal nodes.
    if s.ply > 0 && (b.halfmove_clock >= 100 || b.is_draw_by_repetition()) {
        return 0; // Return neutral draw score
    }

    // Adjust bounds to avoid mate scores wrapping around
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

    // If we are in check, search one ply deeper. This is critical for finding escapes
    // and for seeing forced checkmating sequences.
    if in_check {
        depth += 1;
    }

    if depth <= 0 {
        return quiesce(b, alpha, beta, s);
    }

    s.nodes += 1;
    let alpha_orig = alpha;
    let zobrist_key = b.zobrist;

    // Transposition Table Probe
    if let Some(e) = s.tt.probe(zobrist_key) {
        if e.depth >= depth as i16 && s.ply > 0 {
            match e.bound {
                Bound::Exact if e.score.abs() < MATE_THRESHOLD => return e.score,
                Bound::Lower if e.score >= beta => return e.score,
                Bound::Upper if e.score <= alpha => return e.score,
                _ => {}
            }
        }
    }

    // Null Move Pruning
    if !in_check && depth >= 3 && s.ply > 0 && !is_pv {
        let u = b.make_null_move();
        s.ply += 1;
        // Search with a reduced depth (R=2 is common)
        let score = -negamax(b, -beta, -beta + 1, depth - 1 - 2, s, false);
        s.ply -= 1;
        b.unmake_null_move(u);

        if score >= beta {
            return beta;
        }
    }

    let mut moves = Vec::with_capacity(64);
    b.generate_legal_moves(&mut moves);

    if moves.is_empty() {
        return if in_check {
            -MATE_SCORE + s.ply as i32
        } else {
            0
        }; // Checkmate or Stalemate
    }

    let tt_move = s.tt.probe(zobrist_key).and_then(|e| e.best_move);
    let killers = if s.ply < MAX_PLY {
        s.killers[s.ply]
    } else {
        [None; 2]
    };
    let mut scored_moves = moves
        .into_iter()
        .map(|m| (score_move(b, m, tt_move, &killers, &s.history), m))
        .collect::<Vec<_>>();
    scored_moves.sort_by_key(|(score, _)| -*score);

    let mut best_score = -MATE_SCORE;
    let mut best_move: Option<Move> = None;
    let mut moves_searched = 0;

    for (_, m) in scored_moves {
        // Late Move Reductions (LMR)
        let reduction = if depth >= 3 && moves_searched >= 3 && !m.capture && m.promotion.is_none()
        {
            if moves_searched >= 6 { 2 } else { 1 }
        } else {
            0
        };

        let u = b.make_move(m);
        s.ply += 1;

        let score = if moves_searched == 0 {
            // Search the first move with a full window.
            -negamax(b, -beta, -alpha, depth - 1, s, true)
        } else {
            // For subsequent moves, first try a null-window search.
            let mut score = -negamax(b, -alpha - 1, -alpha, depth - 1 - reduction, s, false);
            // If it beats alpha, it's better than expected; re-search with the full window.
            if score > alpha && score < beta {
                score = -negamax(b, -beta, -alpha, depth - 1, s, false);
            }
            score
        };

        s.ply -= 1;
        b.unmake_move(u);

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
            // This move caused a beta cutoff
            if !m.capture {
                // Reward quiet moves that are good
                let piece = b.piece_on[m.from as usize];
                s.history[piece.index()][m.to as usize] += depth * depth;
                s.store_killer(m);
            }
            s.tt.store(zobrist_key, depth as i16, best_score, Bound::Lower, Some(m));
            return best_score;
        }
        moves_searched += 1;
    }

    let bound = if best_score <= alpha_orig {
        Bound::Upper
    } else {
        Bound::Exact
    };
    s.tt.store(zobrist_key, depth as i16, best_score, bound, best_move);

    best_score
}

/// Extracts the Principal Variation from the transposition table.
fn extract_pv(mut pos: Board, tt: &TransTable, max_len: usize) -> Vec<Move> {
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

/// Iterative deepening framework for a timed search.
pub fn best_move_timed(b: &Board, time_ms: u64, max_depth: usize) -> (Option<Move>, usize, u64) {
    let mut pos = b.clone();
    let mut search = Search::new(time_ms.saturating_sub(5), 16);
    let mut best_move: Option<Move> = None;
    let mut reached_depth = 0;
    let mut score = 0;

    for d in 1..=max_depth {
        let mut alpha = -MATE_SCORE;
        let mut beta = MATE_SCORE;

        if d > 3 {
            alpha = score - 50; // Set a narrow window around the previous score
            beta = score + 50;
        }

        loop {
            score = negamax(&mut pos, alpha, beta, d as i32, &mut search, true);

            if search.ctrl.time_up(search.nodes) {
                break;
            }

            // If the score is outside the window, we must re-search with a wider one.
            if score <= alpha {
                alpha = -MATE_SCORE; // Fail-low, search again with a wider window
            } else if score >= beta {
                beta = MATE_SCORE; // Fail-high, search again
            } else {
                break; // Score is inside the window, proceed to next depth
            }
        }

        if search.ctrl.time_up(search.nodes) && d > 1 {
            break;
        }
        reached_depth = d;
        if let Some(e) = search.tt.probe(pos.zobrist) {
            best_move = e.best_move;
        }

        let elapsed_ms = search.ctrl.start.elapsed().as_millis() as u64;
        let nps = if elapsed_ms > 0 {
            (search.nodes * 1000) / elapsed_ms
        } else {
            0
        };
        let hashfull = search.tt.hashfull_permill();
        let pv = extract_pv(pos.clone(), &search.tt, d);
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

        if score.abs() > MATE_THRESHOLD {
            break; // Found a mate, stop searching
        }
    }
    (best_move, reached_depth, search.nodes)
}

/// A simplified entry point for searching to a fixed depth.
pub fn best_move_depth(b: &Board, depth: usize) -> Option<Move> {
    let (m, _, _) = best_move_timed(b, u64::MAX / 4, depth);
    m
}

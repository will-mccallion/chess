use crate::board::Board;
use crate::tt::{Bound, TransTable};
use crate::types::{Color, Move, Piece, PieceKind};
use crate::uci_io::format_uci;
use std::cmp::{max, min};
use std::time::{Duration, Instant};

const VAL_PAWN: i32 = 100;
const VAL_N: i32 = 320;
const VAL_B: i32 = 330;
const VAL_R: i32 = 500;
const VAL_Q: i32 = 900;
const VAL_K: i32 = 20_000;
const MATE_SCORE: i32 = 100_000;
const MATE_THRESHOLD: i32 = MATE_SCORE - 256; // Room for ply differences

const MAX_PLY: usize = 128;

// Controls time management for the search.
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
    // Checks if the allocated time is up. To reduce overhead, this check
    // is only performed periodically within the search.
    #[inline]
    fn time_up(&mut self, nodes: u64) -> bool {
        if self.stop {
            return true;
        }
        if (nodes & 4095) == 0 {
            // Check every 4096 nodes
            if self.start.elapsed() >= self.time_budget {
                self.stop = true;
            }
        }
        self.stop
    }
}

// Encapsulates all state for a single search instance.
struct Search {
    ctrl: SearchCtrl,
    tt: TransTable,
    nodes: u64,
    killers: [[Option<Move>; 2]; MAX_PLY], // Killer moves: 2 per ply
    ply: usize,                            // Current search depth from the root
}

impl Search {
    fn new(time_ms: u64, tt_size_mb: usize) -> Self {
        Self {
            ctrl: SearchCtrl::new(time_ms),
            tt: TransTable::with_mb(tt_size_mb),
            nodes: 0,
            killers: [[None; 2]; MAX_PLY],
            ply: 0,
        }
    }

    // Stores a killer move for the current ply.
    fn store_killer(&mut self, m: Move) {
        if self.ply >= MAX_PLY {
            return;
        }
        // Shift the existing killer and insert the new one
        if self.killers[self.ply][0] != Some(m) {
            self.killers[self.ply][1] = self.killers[self.ply][0];
            self.killers[self.ply][0] = Some(m);
        }
    }
}

// Piece-Square tables
static MG_PAWN: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 98, 134, 61, 95, 68, 126, 34, -11, -6, 7, 26, 31, 65, 56, 25, -20, -14,
    13, 6, 21, 23, 12, 17, -23, -27, -2, -5, 12, 17, 6, 10, -25, -26, -4, -4, -10, 3, 3, 33, -12,
    -35, -1, -20, -23, -15, 24, 38, -22, 0, 0, 0, 0, 0, 0, 0, 0,
];
static MG_N: [i32; 64] = [
    -167, -89, -34, -49, 61, -97, -15, -107, -73, -41, 72, 36, 23, 62, 7, -17, -47, 60, 37, 65, 84,
    129, 73, 44, -9, 17, 19, 53, 37, 69, 18, 22, -13, 4, 16, 13, 28, 19, 21, -8, -23, -9, 12, 10,
    19, 17, 25, -16, -29, -53, -12, -3, -1, 18, -14, -19, -105, -21, -58, -33, -17, -28, -19, -23,
];
static MG_B: [i32; 64] = [
    -29, 4, -82, -37, -25, -42, 7, -8, -26, 16, -18, -13, 30, 59, 18, -47, -16, 37, 43, 40, 35, 50,
    37, -2, -4, 5, 19, 50, 37, 37, 7, -2, -6, 13, 13, 26, 34, 12, 10, 4, 0, 15, 15, 15, 14, 27, 18,
    10, 4, 15, 16, 0, 7, 21, 33, 1, -33, -3, -14, -21, -13, -12, -39, -21,
];
static MG_R: [i32; 64] = [
    32, 42, 32, 51, 63, 9, 31, 43, 27, 32, 58, 62, 80, 67, 26, 44, -5, 19, 26, 36, 17, 45, 61, 16,
    -24, -11, 7, 26, 24, 35, -8, -20, -36, -26, -12, -1, 9, -7, 6, -23, -45, -25, -16, -17, 3, 0,
    -5, -33, -44, -16, -20, -9, -1, 11, -6, -71, -19, -13, 1, 17, 16, 7, -37, -26,
];
static MG_Q: [i32; 64] = [
    -28, 0, 29, 12, 59, 44, 43, 45, -24, -39, -5, 1, -16, 57, 28, 54, -13, -17, 7, 8, 29, 56, 47,
    57, -27, -27, -16, -16, -1, 17, -2, 1, -9, -26, -9, -10, -2, -4, 3, -3, -14, 2, -11, -2, -5, 2,
    14, 5, -35, -8, 11, 2, 8, 15, -3, 1, -1, -18, -9, 10, -15, -25, -31, -50,
];
static MG_K: [i32; 64] = [
    -65, 23, 16, -15, -56, -34, 2, 13, 29, -1, -20, -7, -8, -4, -38, -29, -9, 24, 2, -16, -20, 6,
    22, -22, -17, -20, -12, -27, -30, -25, -14, -36, -49, -1, -27, -39, -46, -44, -33, -51, -14,
    -14, -22, -46, -44, -30, -15, -27, 1, 7, -8, -64, -43, -16, 9, 8, -15, 36, 12, -54, 8, -28, 24,
    14,
];

// Mirrors a square for the black pieces.
#[inline]
fn mirror_sq(sq: usize) -> usize {
    (7 - (sq >> 3)) << 3 | (sq & 7)
}

fn eval(b: &Board) -> i32 {
    let mut s = 0i32;
    for sq in 0..64 {
        let p = b.piece_on[sq];
        let val = match p {
            Piece::WP => VAL_PAWN + MG_PAWN[sq],
            Piece::WN => VAL_N + MG_N[sq],
            Piece::WB => VAL_B + MG_B[sq],
            Piece::WR => VAL_R + MG_R[sq],
            Piece::WQ => VAL_Q + MG_Q[sq],
            Piece::WK => VAL_K + MG_K[sq],
            Piece::BP => -(VAL_PAWN + MG_PAWN[mirror_sq(sq)]),
            Piece::BN => -(VAL_N + MG_N[mirror_sq(sq)]),
            Piece::BB => -(VAL_B + MG_B[mirror_sq(sq)]),
            Piece::BR => -(VAL_R + MG_R[mirror_sq(sq)]),
            Piece::BQ => -(VAL_Q + MG_Q[mirror_sq(sq)]),
            Piece::BK => -(VAL_K + MG_K[mirror_sq(sq)]),
            Piece::Empty => 0,
        };
        s += val;
    }
    // Perspective is for the side to move
    if b.turn == Color::White { s } else { -s }
}

const PIECE_VALUES: [i32; 7] = [0, 100, 320, 330, 500, 900, 20000]; // Pawn, N, B, R, Q, K

// Assigns a score to a move for ordering purposes.
// Higher scores are searched first.
fn score_move(b: &Board, m: Move, tt_move: Option<Move>, killers: &[Option<Move>; 2]) -> i32 {
    if Some(m) == tt_move {
        return 100_000;
    }

    if m.promotion.is_some() {
        return 90_000;
    }

    if m.capture {
        let attacker = b.piece_on[m.from as usize].kind().unwrap();
        // For en-passant, the victim is a pawn
        let victim = if m.en_passant {
            PieceKind::Pawn
        } else {
            b.piece_on[m.to as usize].kind().unwrap()
        };
        // Most Valuable Victim - Least Valuable Attacker
        return 80_000 + PIECE_VALUES[victim as usize] * 10 - PIECE_VALUES[attacker as usize];
    }

    if Some(m) == killers[0] {
        return 70_000;
    }

    if Some(m) == killers[1] {
        return 60_000;
    }

    0
}

// A special search that only considers captures and promotions to find a "quiet"
// position before running the static evaluation. This avoids the horizon effect.
fn quiesce(b: &mut Board, mut alpha: i32, mut beta: i32, s: &mut Search) -> i32 {
    s.nodes += 1;
    if s.ctrl.time_up(s.nodes) {
        return 0;
    }

    // Adjust mate scores by ply to prefer shorter mates
    alpha = max(alpha, -MATE_SCORE + s.ply as i32);
    beta = min(beta, MATE_SCORE - s.ply as i32);
    if alpha >= beta {
        return alpha;
    }

    let stand_pat = eval(b);
    if stand_pat >= beta {
        return beta;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    let mut moves = Vec::with_capacity(32);
    b.generate_legal_moves(&mut moves);

    // Keep only captures and promotions
    let mut scored_moves = moves
        .into_iter()
        .filter(|m| m.capture || m.promotion.is_some())
        .map(|m| (score_move(b, m, None, &[None, None]), m))
        .collect::<Vec<_>>();

    scored_moves.sort_by_key(|(score, _)| -*score);

    for (_, m) in scored_moves {
        let u = b.make_move(m);
        s.ply += 1;
        let score = -quiesce(b, -beta, -alpha, s);
        s.ply -= 1;
        b.unmake_move(u);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }
    alpha
}

fn negamax(b: &mut Board, mut alpha: i32, mut beta: i32, depth: i32, s: &mut Search) -> i32 {
    if s.ctrl.time_up(s.nodes) {
        return 0; // Time is up, exit gracefully
    }

    // Mate distance pruning
    alpha = max(alpha, -MATE_SCORE + s.ply as i32);
    beta = min(beta, MATE_SCORE - s.ply as i32);
    if alpha >= beta {
        return alpha;
    }

    // Base case: enter quiescence search at leaf nodes
    if depth <= 0 {
        return quiesce(b, alpha, beta, s);
    }

    s.nodes += 1;
    let alpha_orig = alpha;
    let zobrist_key = b.zobrist;

    // TT Probe
    let tt_move = if let Some(e) = s.tt.probe(zobrist_key) {
        if e.depth >= depth as i16 && s.ply > 0 {
            match e.bound {
                Bound::Exact if (e.score.abs()) < MATE_THRESHOLD => return e.score,
                Bound::Lower if e.score >= beta => return e.score,
                Bound::Upper if e.score <= alpha => return e.score,
                _ => {}
            }
        }
        e.best_move
    } else {
        None
    };

    let mut moves = Vec::with_capacity(64);
    b.generate_legal_moves(&mut moves);

    // Check for mate or stalemate
    if moves.is_empty() {
        let king = if b.turn == Color::White {
            Piece::WK
        } else {
            Piece::BK
        };
        let ksq = b.piece_bb[king.index()].trailing_zeros() as i32;
        return if ksq != 64 && b.is_square_attacked(ksq, b.turn.other()) {
            -MATE_SCORE + s.ply as i32 // Checkmated
        } else {
            0 // Stalemate
        };
    }

    // Move Ordering
    let killers = if s.ply < MAX_PLY {
        s.killers[s.ply]
    } else {
        [None, None]
    };
    let mut scored_moves = moves
        .into_iter()
        .map(|m| (score_move(b, m, tt_move, &killers), m))
        .collect::<Vec<_>>();
    scored_moves.sort_by_key(|(score, _)| -*score);

    let mut best_score = -MATE_SCORE;
    let mut best_move: Option<Move> = None;
    let mut moves_searched = 0;

    for (_, m) in scored_moves {
        // Late Move Reductions (LMR)
        let reduction = if depth >= 3 && moves_searched >= 3 && !m.capture && m.promotion.is_none()
        {
            // Reduce depth for quiet moves that are ordered late
            if moves_searched >= 6 { 2 } else { 1 }
        } else {
            0
        };

        let u = b.make_move(m);
        s.ply += 1;

        // Search with reduced depth
        let mut score = -negamax(b, -beta, -alpha, depth - 1 - reduction, s);

        // If LMR seemed to fail, re-search with full depth
        if reduction > 0 && score > alpha {
            score = -negamax(b, -beta, -alpha, depth - 1, s);
        }

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
            // This is a "fail-high" or beta cutoff
            if !m.capture {
                s.store_killer(m);
            }
            s.tt.store(zobrist_key, depth as i16, best_score, Bound::Lower, Some(m));
            return best_score;
        }
        moves_searched += 1;
    }

    // Store result in TT
    let bound = if best_score <= alpha_orig {
        Bound::Upper
    } else {
        Bound::Exact
    };
    s.tt.store(zobrist_key, depth as i16, best_score, bound, best_move);

    best_score
}

// Extracts the Principal Variation (PV) from the Transposition Table.
fn extract_pv(mut pos: Board, tt: &TransTable, max_len: usize) -> Vec<Move> {
    let mut pv = Vec::with_capacity(max_len);
    for _ in 0..max_len {
        if let Some(e) = tt.probe(pos.zobrist) {
            if let Some(m) = e.best_move {
                let mut legal_moves = Vec::new();
                pos.generate_legal_moves(&mut legal_moves);
                if legal_moves.contains(&m) {
                    pv.push(m);
                    pos.make_move(m);
                    continue;
                }
            }
        }
        break;
    }
    pv
}

/// Time-managed search. Uses iterative deepening until time runs out.
/// Returns (best_move, searched_depth, nodes).
pub fn best_move_timed(b: &Board, time_ms: u64, max_depth: usize) -> (Option<Move>, usize, u64) {
    let mut pos = b.clone();
    let mut search = Search::new(time_ms.saturating_sub(5), 16); // 16MB TT

    let mut best_move: Option<Move> = None;
    let mut reached_depth = 0;

    for d in 1..=max_depth {
        let score = negamax(&mut pos, -MATE_SCORE, MATE_SCORE, d as i32, &mut search);

        if search.ctrl.time_up(search.nodes) && d > 1 {
            // Don't trust results from a search that was stopped early
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

        let info_score = if score.abs() > MATE_THRESHOLD {
            // FIXED: using `score` directly
            let plies_to_mate = MATE_SCORE - score.abs();
            let moves_to_mate = (plies_to_mate + 1) / 2;
            if score > 0 {
                format!("mate {}", moves_to_mate)
            } else {
                format!("mate -{}", moves_to_mate)
            }
        } else {
            format!("cp {}", score)
        };

        println!(
            "info depth {} score {} nodes {} nps {} time {} hashfull {} pv {}",
            d, info_score, search.nodes, nps, elapsed_ms, hashfull, pv_str
        );

        // Stop if mate is found
        if score.abs() > MATE_THRESHOLD {
            break;
        }
    }

    (best_move, reached_depth, search.nodes)
}

/// Depth-limited search (no time cap).
pub fn best_move_depth(b: &Board, depth: usize) -> Option<Move> {
    let (m, _, _) = best_move_timed(b, u64::MAX / 4, depth);
    m
}

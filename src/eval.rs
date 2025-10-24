use crate::board::Board;
use crate::pst::{EG_PST, MG_PST};
use crate::types::{Bitboard, Color, Piece, PieceKind};

const MATERIAL_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 0]; // P, N, B, R, Q, K
const PHASE_VALUES: [i32; 6] = [0, 1, 1, 2, 4, 0]; // P, N, B, R, Q, K

// Bonuses for passed pawns, indexed by rank (1-8). Rank 0 is unused.
const PASSED_PAWN_BONUS: [i32; 8] = [0, 10, 20, 30, 50, 75, 100, 0];

#[inline]
fn file_of(sq: usize) -> usize {
    sq & 7
}

#[inline]
fn rank_of(sq: usize) -> usize {
    sq >> 3
}

#[inline]
fn bit(sq: usize) -> Bitboard {
    1 << sq
}

// King shield masks to evaluate pawn cover
const KING_SHIELD: [Bitboard; 64] = gen_king_shield();
const fn gen_king_shield() -> [Bitboard; 64] {
    let mut masks = [0; 64];
    let mut sq = 0;
    while sq < 64 {
        let f = sq & 7;
        let mut shield = 0;
        // One rank ahead
        shield |= 1 << (sq + 8);
        if f > 0 {
            shield |= 1 << (sq + 7);
        }
        if f < 7 {
            shield |= 1 << (sq + 9);
        }
        masks[sq] = shield;
        sq += 1;
    }
    masks
}

// Mask for files adjacent to a given square's file
const ADJACENT_FILES: [Bitboard; 8] = [
    0x0202020202020202, // a
    0x0505050505050505, // b
    0x0a0a0a0a0a0a0a0a, // c
    0x1414141414141414, // d
    0x2828282828282828, // e
    0x5050505050505050, // f
    0xa0a0a0a0a0a0a0a0, // g
    0x4040404040404040, // h
];

// Mask for all squares "in front" of a given square, for a given color
fn front_span(sq: usize, color: Color) -> Bitboard {
    let file = file_of(sq);
    let own_file_mask = 0x0101010101010101 << file;
    let mask = own_file_mask | ADJACENT_FILES[file];
    if color == Color::White {
        mask & (u64::MAX << (sq + 1))
    } else {
        mask & ((1u64 << sq) - 1)
    }
}

fn is_passed(b: &Board, sq: usize, color: Color) -> bool {
    let opp_pawns = if color == Color::White {
        b.piece_bb[Piece::BP.index()]
    } else {
        b.piece_bb[Piece::WP.index()]
    };
    (front_span(sq, color) & opp_pawns) == 0
}

/// Calculates the evaluation of the board from White's perspective.
fn evaluate_white_pov(b: &Board) -> i32 {
    let mut mg_score = 0;
    let mut eg_score = 0;
    let mut game_phase = 0;

    let wk_sq = b.piece_bb[Piece::WK.index()].trailing_zeros() as usize;
    let bk_sq = b.piece_bb[Piece::BK.index()].trailing_zeros() as usize;

    // King safety (middlegame only)
    let white_pawns = b.piece_bb[Piece::WP.index()];
    let black_pawns = b.piece_bb[Piece::BP.index()];
    mg_score -= (3 - (KING_SHIELD[wk_sq] & white_pawns).count_ones()) as i32 * 15;
    mg_score += (3 - (KING_SHIELD[bk_sq] & black_pawns).count_ones()) as i32 * 15;

    for sq in 0..64 {
        let piece = b.piece_on[sq];
        if piece.is_empty() {
            continue;
        }

        let color = piece.color().unwrap();
        let kind = piece.kind().unwrap();
        let kind_idx = kind as usize;

        let mg_pst = MG_PST[piece.index()][sq];
        let eg_pst = EG_PST[piece.index()][sq];

        if color == Color::White {
            mg_score += MATERIAL_VALUES[kind_idx] + mg_pst;
            eg_score += MATERIAL_VALUES[kind_idx] + eg_pst;
            if kind == PieceKind::Pawn && is_passed(b, sq, Color::White) {
                eg_score += PASSED_PAWN_BONUS[rank_of(sq)];
            }
        } else {
            mg_score -= MATERIAL_VALUES[kind_idx] + mg_pst;
            eg_score -= MATERIAL_VALUES[kind_idx] + eg_pst;
            if kind == PieceKind::Pawn && is_passed(b, sq, Color::Black) {
                eg_score -= PASSED_PAWN_BONUS[7 - rank_of(sq)];
            }
        }

        game_phase += PHASE_VALUES[kind_idx];
    }

    let phase = std::cmp::min(game_phase, 24);
    let final_score = (mg_score * phase + eg_score * (24 - phase)) / 24;

    final_score
}

/// Main entry point. Returns the score from the current side-to-move's perspective.
pub fn evaluate(b: &Board) -> i32 {
    let score_from_white_pov = evaluate_white_pov(b);
    if b.turn == Color::White {
        score_from_white_pov
    } else {
        -score_from_white_pov
    }
}

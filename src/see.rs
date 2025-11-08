// src/see.rs

use crate::board::Board;
use crate::magics;
use crate::types::{Move, Piece, PieceKind};

const PIECE_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 20000]; // P, N, B, R, Q, K

#[inline(always)]
fn val(p: Piece) -> i32 {
    if let Some(kind) = p.kind() {
        PIECE_VALUES[kind as usize]
    } else {
        0
    }
}

fn get_attackers(
    b: &Board,
    sq: usize,
    occupied: u64,
    side: crate::types::Color,
) -> (u64, Option<(Piece, usize)>) {
    let mut attackers = 0u64;
    let mut lva_piece = None;
    let mut lva_sq = 0;

    // Pawns
    let pawn_attacks = if side == crate::types::Color::White {
        magics::BLACK_PAWN_ATTACKS[sq]
    } else {
        magics::WHITE_PAWN_ATTACKS[sq]
    };
    let pawns = b.piece_bb[Piece::from_kind(PieceKind::Pawn, side).index()] & occupied;
    if (pawn_attacks & pawns) != 0 {
        attackers |= pawn_attacks & pawns;
        lva_piece = Some(Piece::from_kind(PieceKind::Pawn, side));
        lva_sq = (pawn_attacks & pawns).trailing_zeros() as usize;
        return (attackers, lva_piece.map(|p| (p, lva_sq)));
    }

    // Knights
    let knights = b.piece_bb[Piece::from_kind(PieceKind::Knight, side).index()] & occupied;
    if (magics::knight_attacks_from(sq) & knights) != 0 {
        attackers |= magics::knight_attacks_from(sq) & knights;
        lva_piece = Some(Piece::from_kind(PieceKind::Knight, side));
        lva_sq = (magics::knight_attacks_from(sq) & knights).trailing_zeros() as usize;
        return (attackers, lva_piece.map(|p| (p, lva_sq)));
    }

    // Bishops & Queens (diagonal)
    let bishops_queens = (b.piece_bb[Piece::from_kind(PieceKind::Bishop, side).index()]
        | b.piece_bb[Piece::from_kind(PieceKind::Queen, side).index()])
        & occupied;
    let bishop_attacks = magics::get_bishop_attacks(sq, occupied);
    if (bishop_attacks & bishops_queens) != 0 {
        attackers |= bishop_attacks & bishops_queens;
        let bishops = b.piece_bb[Piece::from_kind(PieceKind::Bishop, side).index()] & occupied;
        if (bishop_attacks & bishops) != 0 {
            lva_piece = Some(Piece::from_kind(PieceKind::Bishop, side));
            lva_sq = (bishop_attacks & bishops).trailing_zeros() as usize;
            return (attackers, lva_piece.map(|p| (p, lva_sq)));
        }
    }

    // Rooks & Queens (orthogonal)
    let rooks_queens = (b.piece_bb[Piece::from_kind(PieceKind::Rook, side).index()]
        | b.piece_bb[Piece::from_kind(PieceKind::Queen, side).index()])
        & occupied;
    let rook_attacks = magics::get_rook_attacks(sq, occupied);
    if (rook_attacks & rooks_queens) != 0 {
        attackers |= rook_attacks & rooks_queens;
        let rooks = b.piece_bb[Piece::from_kind(PieceKind::Rook, side).index()] & occupied;
        if (rook_attacks & rooks) != 0 && lva_piece.is_none() {
            lva_piece = Some(Piece::from_kind(PieceKind::Rook, side));
            lva_sq = (rook_attacks & rooks).trailing_zeros() as usize;
            return (attackers, lva_piece.map(|p| (p, lva_sq)));
        }
    }

    // Queens (if not found as bishop/rook)
    if lva_piece.is_none() {
        let queens = b.piece_bb[Piece::from_kind(PieceKind::Queen, side).index()] & occupied;
        if (bishop_attacks & queens) != 0 {
            lva_piece = Some(Piece::from_kind(PieceKind::Queen, side));
            lva_sq = (bishop_attacks & queens).trailing_zeros() as usize;
            return (attackers, lva_piece.map(|p| (p, lva_sq)));
        }
        if (rook_attacks & queens) != 0 {
            lva_piece = Some(Piece::from_kind(PieceKind::Queen, side));
            lva_sq = (rook_attacks & queens).trailing_zeros() as usize;
            return (attackers, lva_piece.map(|p| (p, lva_sq)));
        }
    }

    // Kings
    let kings = b.piece_bb[Piece::from_kind(PieceKind::King, side).index()] & occupied;
    if (magics::king_attacks_from(sq) & kings) != 0 {
        attackers |= magics::king_attacks_from(sq) & kings;
        if lva_piece.is_none() {
            lva_piece = Some(Piece::from_kind(PieceKind::King, side));
            lva_sq = (magics::king_attacks_from(sq) & kings).trailing_zeros() as usize;
            return (attackers, lva_piece.map(|p| (p, lva_sq)));
        }
    }

    (attackers, lva_piece.map(|p| (p, lva_sq)))
}

pub fn see(b: &Board, mov: Move) -> i32 {
    if !mov.capture {
        return 0;
    }

    let from_sq = mov.from as usize;
    let to_sq = mov.to as usize;

    let mut gain = [0; 32];
    let mut gain_idx = 1;

    let mut from_piece = b.piece_on[from_sq];
    let mut occupied = b.all_pieces;
    let mut current_turn = b.turn;

    let captured_piece = if mov.en_passant {
        Piece::from_kind(PieceKind::Pawn, b.turn.other())
    } else {
        b.piece_on[to_sq]
    };

    gain[0] = val(captured_piece);

    // Initial attack
    occupied ^= 1u64 << from_sq;

    loop {
        current_turn = current_turn.other();

        let (_, lva) = get_attackers(b, to_sq, occupied, current_turn);

        if let Some((attacker_piece, attacker_sq)) = lva {
            occupied ^= 1u64 << attacker_sq;

            if gain_idx >= 32 {
                break;
            }

            gain[gain_idx] = val(from_piece) - gain[gain_idx - 1];
            gain_idx += 1;

            from_piece = attacker_piece;
        } else {
            break;
        }
    }

    while gain_idx > 1 {
        gain_idx -= 1;
        gain[gain_idx - 1] = -(-gain[gain_idx - 1]).max(gain[gain_idx]);
    }

    gain[0]
}

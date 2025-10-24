use crate::board::Board;
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

fn attackers_to(b: &Board, sq: usize, occupied: u64, color: crate::types::Color) -> u64 {
    let s = sq as i32;
    let mut attackers = 0;

    let pawn = Piece::from_kind(PieceKind::Pawn, color);
    let pawn_bb = b.piece_bb[pawn.index()];

    if color == crate::types::Color::White {
        if s % 8 != 0 && s > 8 {
            attackers |= (1u64 << (s - 9)) & pawn_bb;
        }
        if s % 8 != 7 && s > 8 {
            attackers |= (1u64 << (s - 7)) & pawn_bb;
        }
    } else {
        if s % 8 != 0 && s < 56 {
            attackers |= (1u64 << (s + 7)) & pawn_bb;
        }
        if s % 8 != 7 && s < 56 {
            attackers |= (1u64 << (s + 9)) & pawn_bb;
        }
    };

    attackers |= crate::magics::knight_attacks_from(sq)
        & b.piece_bb[Piece::from_kind(PieceKind::Knight, color).index()];

    attackers |= crate::magics::king_attacks_from(sq)
        & b.piece_bb[Piece::from_kind(PieceKind::King, color).index()];

    attackers |= crate::magics::get_bishop_attacks(sq, occupied)
        & (b.piece_bb[Piece::from_kind(PieceKind::Bishop, color).index()]
            | b.piece_bb[Piece::from_kind(PieceKind::Queen, color).index()]);

    attackers |= crate::magics::get_rook_attacks(sq, occupied)
        & (b.piece_bb[Piece::from_kind(PieceKind::Rook, color).index()]
            | b.piece_bb[Piece::from_kind(PieceKind::Queen, color).index()]);

    attackers
}

fn least_valuable_attacker(
    b: &Board,
    attackers: u64,
    side: crate::types::Color,
) -> Option<(Piece, usize)> {
    for kind in [
        PieceKind::Pawn,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Rook,
        PieceKind::Queen,
        PieceKind::King,
    ] {
        let piece = Piece::from_kind(kind, side);
        let subset = b.piece_bb[piece.index()] & attackers;
        if subset != 0 {
            return Some((piece, subset.trailing_zeros() as usize));
        }
    }

    None
}

pub fn see(b: &Board, mov: Move) -> i32 {
    if !mov.capture {
        return 0;
    }

    let from_sq = mov.from as usize;
    let to_sq = mov.to as usize;

    let mut gain = [0; 32];
    let mut gain_idx = 0;

    let mut from_piece = b.piece_on[from_sq];
    let mut occupied = b.all_pieces;
    let mut current_turn = b.turn;

    let captured_piece = if mov.en_passant {
        Piece::from_kind(PieceKind::Pawn, b.turn.other())
    } else {
        b.piece_on[to_sq]
    };

    gain[gain_idx] = val(captured_piece);
    gain_idx += 1;

    occupied ^= 1u64 << from_sq;

    loop {
        current_turn = current_turn.other();

        let attackers = attackers_to(b, to_sq, occupied, current_turn);
        if attackers == 0 {
            break;
        }

        if let Some((attacker_piece, attacker_sq)) =
            least_valuable_attacker(b, attackers, current_turn)
        {
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

use crate::board::Board;
use crate::magics;
use crate::pawn_hash;
use crate::pst::{EG_PST, MG_PST};
use crate::types::{BK_CASTLE, BQ_CASTLE, Bitboard, Color, Piece, PieceKind, WK_CASTLE, WQ_CASTLE};

const PHASE_VALUES: [i32; 6] = [0, 1, 1, 2, 4, 0]; // P,N,B,R,Q,K
const MAX_PHASE: i32 = 24;

// Pawn structure penalties
const ISOLATED_PAWN_PENALTY: (i32, i32) = (10, 20);
const DOUBLED_PAWN_PENALTY: (i32, i32) = (10, 25);
const BACKWARD_PAWN_PENALTY: (i32, i32) = (8, 15);

// Passed pawn bonuses by rank
const PASSED_PAWN_BONUS_MG: [i32; 8] = [0, 10, 20, 35, 60, 100, 150, 0];
const PASSED_PAWN_BONUS_EG: [i32; 8] = [0, 20, 40, 70, 110, 160, 220, 0];

// Other bonuses
const BISHOP_PAIR_BONUS: (i32, i32) = (40, 60); // Slightly increased
const ROOK_OPEN_FILE_BONUS: i32 = 20;
const ROOK_SEMI_OPEN_FILE_BONUS: i32 = 10;
const ROOK_ON_7TH_BONUS: i32 = 25;
const CASTLING_RIGHTS_BONUS: i32 = 25;

// Penalty for pawns being pushed away from the king's shield
const PAWN_SHIELD_PENALTY: [i32; 3] = [10, 25, 40]; // For 1, 2, or 3+ ranks pushed

// Mobility values per piece type
const KNIGHT_MOBILITY: [i32; 9] = [-30, -20, -10, 0, 5, 10, 15, 20, 25];
const BISHOP_MOBILITY: [i32; 14] = [-30, -20, -10, -5, 0, 5, 10, 15, 20, 25, 30, 35, 40, 45];
const ROOK_MOBILITY: [i32; 15] = [-20, -15, -10, -5, 0, 5, 10, 12, 15, 18, 20, 22, 25, 28, 30];
const QUEEN_MOBILITY: [i32; 28] = [
    -10, -5, 0, 3, 5, 8, 10, 12, 15, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 41, 42, 43,
    44, 45, 46, 47,
];

// King safety constants
const KING_ATTACK_WEIGHTS: [i32; 5] = [20, 30, 50, 90, 0]; // N, B, R, Q, (unused)
// Increased king safety penalties
const KING_SAFETY_TABLE: [i32; 20] = [
    0, 0, 2, 5, 8, 12, 18, 25, 35, 45, 55, 70, 85, 100, 120, 140, 160, 180, 200, 220,
];

pub fn evaluate(b: &Board) -> i32 {
    let score = evaluate_white_pov(b);
    if b.turn == Color::White {
        score
    } else {
        -score
    }
}

fn evaluate_white_pov(b: &Board) -> i32 {
    let mut mg_score = 0;
    let mut eg_score = 0;
    let mut phase = 0;

    let white_pawns = b.piece_bb[Piece::WP.index()];
    let black_pawns = b.piece_bb[Piece::BP.index()];
    let pawn_key =
        b.zob.piece_key(Piece::WP, 0) ^ white_pawns ^ b.zob.piece_key(Piece::BP, 0) ^ black_pawns;

    if let Some((pawn_mg, pawn_eg)) = pawn_hash::pawn_tt().probe(pawn_key) {
        mg_score += pawn_mg;
        eg_score += pawn_eg;
    } else {
        let (pawn_mg, pawn_eg) = evaluate_pawns(white_pawns, black_pawns);
        mg_score += pawn_mg;
        eg_score += pawn_eg;
        pawn_hash::pawn_tt().store(pawn_key, pawn_mg, pawn_eg);
    }

    for p_idx in 1..=12 {
        let piece = Piece::from(p_idx as u8);
        if piece.is_empty() {
            continue;
        }
        let mut bb = b.piece_bb[p_idx];
        if let Some(kind) = piece.kind() {
            phase += PHASE_VALUES[kind as usize] * bb.count_ones() as i32;
        }
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            mg_score += MG_PST[p_idx][sq];
            eg_score += EG_PST[p_idx][sq];
        }
    }

    let (w_mob, b_mob) = evaluate_mobility(b);
    mg_score += w_mob.0 - b_mob.0;
    eg_score += w_mob.1 - b_mob.1;

    let w_king_safety = evaluate_king_safety(b, Color::White);
    let b_king_safety = evaluate_king_safety(b, Color::Black);
    mg_score += b_king_safety - w_king_safety;

    if (b.castle & (WK_CASTLE | WQ_CASTLE)) != 0 {
        mg_score += CASTLING_RIGHTS_BONUS;
    }
    if (b.castle & (BK_CASTLE | BQ_CASTLE)) != 0 {
        mg_score -= CASTLING_RIGHTS_BONUS;
    }

    if (b.piece_bb[Piece::WB.index()]).count_ones() >= 2 {
        mg_score += BISHOP_PAIR_BONUS.0;
        eg_score += BISHOP_PAIR_BONUS.1;
    }
    if (b.piece_bb[Piece::BB.index()]).count_ones() >= 2 {
        mg_score -= BISHOP_PAIR_BONUS.0;
        eg_score -= BISHOP_PAIR_BONUS.1;
    }

    let final_phase = phase.min(MAX_PHASE);
    (mg_score * final_phase + eg_score * (MAX_PHASE - final_phase)) / MAX_PHASE
}

fn evaluate_pawns(white_pawns: Bitboard, black_pawns: Bitboard) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;

    let mut wp = white_pawns;
    while wp != 0 {
        let sq = wp.trailing_zeros() as usize;
        wp &= wp - 1;
        let (m, e) = evaluate_single_pawn(sq, Color::White, white_pawns, black_pawns);
        mg += m;
        eg += e;
    }

    let mut bp = black_pawns;
    while bp != 0 {
        let sq = bp.trailing_zeros() as usize;
        bp &= bp - 1;
        let (m, e) = evaluate_single_pawn(sq, Color::Black, black_pawns, white_pawns);
        mg -= m;
        eg -= e;
    }
    (mg, eg)
}

fn evaluate_single_pawn(
    sq: usize,
    c: Color,
    us_pawns: Bitboard,
    them_pawns: Bitboard,
) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;
    let file = sq % 8;
    let rank = if c == Color::White {
        sq / 8
    } else {
        7 - (sq / 8)
    };

    let file_mask = 0x0101010101010101 << file;
    let adj_files_mask =
        ((file_mask << 1) & !0x0101010101010101) | ((file_mask >> 1) & !0x8080808080808080);

    if (us_pawns & adj_files_mask) == 0 {
        mg -= ISOLATED_PAWN_PENALTY.0;
        eg -= ISOLATED_PAWN_PENALTY.1;
    }

    if (us_pawns & file_mask).count_ones() > 1 {
        mg -= DOUBLED_PAWN_PENALTY.0;
        eg -= DOUBLED_PAWN_PENALTY.1;
    }

    let forward_span = if c == Color::White {
        (adj_files_mask | file_mask) & (u64::MAX << (sq + 1))
    } else {
        (adj_files_mask | file_mask) & ((1u64 << sq) - 1)
    };

    if (them_pawns & forward_span) == 0 {
        mg += PASSED_PAWN_BONUS_MG[rank];
        eg += PASSED_PAWN_BONUS_EG[rank];
    }

    let behind_mask = if c == Color::White {
        adj_files_mask & ((1u64 << sq) - 1)
    } else {
        adj_files_mask & (u64::MAX << (sq + 1))
    };

    let stop_sq = if c == Color::White { sq + 8 } else { sq - 8 };
    let them_attacks_stop = if c == Color::White {
        ((them_pawns << 7) & !0x0101010101010101) | ((them_pawns << 9) & !0x8080808080808080)
    } else {
        ((them_pawns >> 9) & !0x0101010101010101) | ((them_pawns >> 7) & !0x8080808080808080)
    };

    if (us_pawns & behind_mask) == 0 && (them_attacks_stop & (1u64 << stop_sq)) != 0 {
        mg -= BACKWARD_PAWN_PENALTY.0;
        eg -= BACKWARD_PAWN_PENALTY.1;
    }

    (mg, eg)
}

fn evaluate_mobility(b: &Board) -> ((i32, i32), (i32, i32)) {
    let mut w_mg = 0;
    let mut w_eg = 0;
    let mut b_mg = 0;
    let mut b_eg = 0;

    let occ = b.all_pieces;
    let w_occ = b.w_pieces;
    let b_occ = b.b_pieces;

    let mut wn = b.piece_bb[Piece::WN.index()];
    while wn != 0 {
        let sq = wn.trailing_zeros() as usize;
        wn &= wn - 1;
        let mob = (magics::knight_attacks_from(sq) & !w_occ).count_ones() as usize;
        w_mg += KNIGHT_MOBILITY[mob];
        w_eg += KNIGHT_MOBILITY[mob];
    }

    let mut wb = b.piece_bb[Piece::WB.index()];
    while wb != 0 {
        let sq = wb.trailing_zeros() as usize;
        wb &= wb - 1;
        let mob = (magics::get_bishop_attacks(sq, occ) & !w_occ).count_ones() as usize;
        w_mg += BISHOP_MOBILITY[mob];
        w_eg += BISHOP_MOBILITY[mob];
    }

    let mut wr = b.piece_bb[Piece::WR.index()];
    while wr != 0 {
        let sq = wr.trailing_zeros() as usize;
        wr &= wr - 1;
        let mob = (magics::get_rook_attacks(sq, occ) & !w_occ).count_ones() as usize;
        w_mg += ROOK_MOBILITY[mob];
        w_eg += ROOK_MOBILITY[mob];

        let file_mask = 0x0101010101010101 << (sq % 8);
        if (b.piece_bb[Piece::WP.index()] & file_mask) == 0 {
            if (b.piece_bb[Piece::BP.index()] & file_mask) == 0 {
                w_mg += ROOK_OPEN_FILE_BONUS;
            } else {
                w_mg += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        if sq / 8 == 6 {
            w_mg += ROOK_ON_7TH_BONUS;
        }
    }

    let mut wq = b.piece_bb[Piece::WQ.index()];
    while wq != 0 {
        let sq = wq.trailing_zeros() as usize;
        wq &= wq - 1;
        let mob = ((magics::get_rook_attacks(sq, occ) | magics::get_bishop_attacks(sq, occ))
            & !w_occ)
            .count_ones() as usize;
        w_mg += QUEEN_MOBILITY[mob];
        w_eg += QUEEN_MOBILITY[mob];
    }

    let mut bn = b.piece_bb[Piece::BN.index()];
    while bn != 0 {
        let sq = bn.trailing_zeros() as usize;
        bn &= bn - 1;
        let mob = (magics::knight_attacks_from(sq) & !b_occ).count_ones() as usize;
        b_mg += KNIGHT_MOBILITY[mob];
        b_eg += KNIGHT_MOBILITY[mob];
    }

    let mut bb = b.piece_bb[Piece::BB.index()];
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        bb &= bb - 1;
        let mob = (magics::get_bishop_attacks(sq, occ) & !b_occ).count_ones() as usize;
        b_mg += BISHOP_MOBILITY[mob];
        b_eg += BISHOP_MOBILITY[mob];
    }

    let mut br = b.piece_bb[Piece::BR.index()];
    while br != 0 {
        let sq = br.trailing_zeros() as usize;
        br &= br - 1;
        let mob = (magics::get_rook_attacks(sq, occ) & !b_occ).count_ones() as usize;
        b_mg += ROOK_MOBILITY[mob];
        b_eg += ROOK_MOBILITY[mob];
        let file_mask = 0x0101010101010101 << (sq % 8);
        if (b.piece_bb[Piece::BP.index()] & file_mask) == 0 {
            if (b.piece_bb[Piece::WP.index()] & file_mask) == 0 {
                b_mg += ROOK_OPEN_FILE_BONUS;
            } else {
                b_mg += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        if sq / 8 == 1 {
            b_mg += ROOK_ON_7TH_BONUS;
        }
    }

    let mut bq = b.piece_bb[Piece::BQ.index()];
    while bq != 0 {
        let sq = bq.trailing_zeros() as usize;
        bq &= bq - 1;
        let mob = ((magics::get_rook_attacks(sq, occ) | magics::get_bishop_attacks(sq, occ))
            & !b_occ)
            .count_ones() as usize;
        b_mg += QUEEN_MOBILITY[mob];
        b_eg += QUEEN_MOBILITY[mob];
    }

    ((w_mg, w_eg), (b_mg, b_eg))
}

fn evaluate_king_safety(b: &Board, c: Color) -> i32 {
    let king_bb = b.piece_bb[Piece::from_kind(PieceKind::King, c).index()];
    if king_bb == 0 {
        return 0;
    }
    let king_sq = king_bb.trailing_zeros() as usize;

    let mut pawn_shield_penalty = 0;
    let king_file = king_sq % 8;
    let king_rank = king_sq / 8;

    if (c == Color::White && king_rank <= 1) || (c == Color::Black && king_rank >= 6) {
        let pawns = b.piece_bb[Piece::from_kind(PieceKind::Pawn, c).index()];
        let start_file = if king_file > 0 { king_file - 1 } else { 0 };
        let end_file = if king_file < 7 { king_file + 1 } else { 7 };

        for f in start_file..=end_file {
            let file_mask = 0x0101010101010101 << f;
            let pawn_on_file = file_mask & pawns;

            if pawn_on_file == 0 {
                pawn_shield_penalty += PAWN_SHIELD_PENALTY[1]; // Missing pawn
            } else {
                let pawn_sq = pawn_on_file.trailing_zeros() as usize;
                let pawn_rank = pawn_sq / 8;
                let expected_rank = if c == Color::White { 1 } else { 6 };
                let rank_diff = pawn_rank - expected_rank;
                if rank_diff > 0 {
                    pawn_shield_penalty += PAWN_SHIELD_PENALTY[rank_diff.min(2)];
                }
            }
        }
    }

    let king_ring = magics::king_attacks_from(king_sq);
    let them = c.other();

    let mut attack_score = 0;
    let mut them_knights = b.piece_bb[Piece::from_kind(PieceKind::Knight, them).index()];
    while them_knights != 0 {
        let sq = them_knights.trailing_zeros() as usize;
        them_knights &= them_knights - 1;
        if (magics::knight_attacks_from(sq) & king_ring) != 0 {
            attack_score += KING_ATTACK_WEIGHTS[0];
        }
    }
    let mut them_bishops = b.piece_bb[Piece::from_kind(PieceKind::Bishop, them).index()];
    while them_bishops != 0 {
        let sq = them_bishops.trailing_zeros() as usize;
        them_bishops &= them_bishops - 1;
        if (magics::get_bishop_attacks(sq, b.all_pieces) & king_ring) != 0 {
            attack_score += KING_ATTACK_WEIGHTS[1];
        }
    }
    let mut them_rooks = b.piece_bb[Piece::from_kind(PieceKind::Rook, them).index()];
    while them_rooks != 0 {
        let sq = them_rooks.trailing_zeros() as usize;
        them_rooks &= them_rooks - 1;
        if (magics::get_rook_attacks(sq, b.all_pieces) & king_ring) != 0 {
            attack_score += KING_ATTACK_WEIGHTS[2];
        }
    }
    let mut them_queens = b.piece_bb[Piece::from_kind(PieceKind::Queen, them).index()];
    while them_queens != 0 {
        let sq = them_queens.trailing_zeros() as usize;
        them_queens &= them_queens - 1;
        if ((magics::get_rook_attacks(sq, b.all_pieces)
            | magics::get_bishop_attacks(sq, b.all_pieces))
            & king_ring)
            != 0
        {
            attack_score += KING_ATTACK_WEIGHTS[3];
        }
    }

    KING_SAFETY_TABLE[(attack_score / 10).min(19) as usize] + pawn_shield_penalty
}

impl Piece {
    fn from(val: u8) -> Self {
        match val {
            1 => Piece::WP,
            2 => Piece::WN,
            3 => Piece::WB,
            4 => Piece::WR,
            5 => Piece::WQ,
            6 => Piece::WK,
            7 => Piece::BP,
            8 => Piece::BN,
            9 => Piece::BB,
            10 => Piece::BR,
            11 => Piece::BQ,
            12 => Piece::BK,
            _ => Piece::Empty,
        }
    }
}

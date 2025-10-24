use crate::board::Board;
use crate::magics;
use crate::pst::{EG_PST, MG_PST};
use crate::types::{Bitboard, Color, Piece, PieceKind};

const MATERIAL_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 0]; // P, N, B, R, Q, K

const PHASE_VALUES: [i32; 6] = [0, 1, 1, 2, 4, 0]; // P,N,B,R,Q,K
const MAX_PHASE: i32 = 24;

const ISOLATED_PAWN_PENALTY: i32 = 15;
const DOUBLED_PAWN_PENALTY: i32 = 12;
const BACKWARD_PAWN_PENALTY: i32 = 16;

const PASSED_PAWN_BONUS_MG: [i32; 8] = [0, 8, 16, 28, 45, 70, 95, 0];
const PASSED_PAWN_BONUS_EG: [i32; 8] = [0, 12, 24, 40, 65, 100, 140, 0];

const BISHOP_PAIR_BONUS_MG: i32 = 20;
const BISHOP_PAIR_BONUS_EG: i32 = 25;

const ROOK_OPEN_FILE_BONUS: i32 = 16;
const ROOK_SEMIOPEN_FILE_BONUS: i32 = 8;
const ROOK_ON_7TH_BONUS: i32 = 20;

const KNIGHT_OUTPOST_RANK4_BONUS: i32 = 12;
const KNIGHT_OUTPOST_RANK5_BONUS: i32 = 20;
const KNIGHT_OUTPOST_RANK6_BONUS: i32 = 26;

#[derive(Clone, Copy)]
struct MobW {
    mg: i32,
    eg: i32,
}

const MOB_KNIGHT: MobW = MobW { mg: 2, eg: 1 };
const MOB_BISHOP: MobW = MobW { mg: 2, eg: 2 };
const MOB_ROOK: MobW = MobW { mg: 2, eg: 2 };
const MOB_QUEEN: MobW = MobW { mg: 1, eg: 1 };

const KING_ATTACK_WEIGHTS: [i32; 5] = [0, 20, 20, 40, 80]; // N, B, R, Q 
const ATTACK_COUNT_BONUS: [i32; 10] = [0, 50, 75, 88, 94, 97, 99, 100, 100, 100];
const KING_SAFETY_MAX_PENALTY: i32 = 800;

const PAWN_SHIELD_MISSING_PENALTY: i32 = 15;
const PAWN_SHIELD_MISSING_ON_OPEN_FILE_PENALTY: i32 = 40;

const TROPISM_KNIGHT: i32 = 2;
const TROPISM_BISHOP: i32 = 1;
const TROPISM_ROOK: i32 = 1;
const TROPISM_QUEEN: i32 = 2;

const CENTER_CONTROL_OCC_MG: i32 = 6;
const CENTER_CONTROL_ATT_MG: i32 = 2;
const EXT_CENTER_OCC_MG: i32 = 2;
const EXT_CENTER_ATT_MG: i32 = 1;
const CENTER_CONTROL_SCALE_EG: i32 = 1;

pub fn evaluate(b: &Board) -> i32 {
    let s = evaluate_white_pov(b);
    if b.turn == Color::White { s } else { -s }
}

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
    1u64 << sq
}

#[inline]
fn manhattan(a: usize, b: usize) -> i32 {
    let fa = (a & 7) as i32;
    let fb = (b & 7) as i32;
    let ra = (a >> 3) as i32;
    let rb = (b >> 3) as i32;
    (fa - fb).abs() + (ra - rb).abs()
}

// Masks
const FILE_A: Bitboard = 0x0101010101010101;
const FILE_H: Bitboard = 0x8080808080808080;

// Adjacent files mask
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

// Center masks
const CENTER4: Bitboard = (1u64 << 27) | (1u64 << 28) | (1u64 << 35) | (1u64 << 36); // d4,e4,d5,e5
const EXT_CENTER: Bitboard = 0x000000001C3E3E1C; // ranks 3-6 files c-f 

#[inline]
fn pawn_attacks_from(sq: usize, color: Color) -> Bitboard {
    let f = file_of(sq);
    match color {
        Color::White => {
            let mut bb = 0u64;
            if f > 0 && sq < 56 {
                bb |= 1u64 << (sq + 7);
            }
            if f < 7 && sq < 56 {
                bb |= 1u64 << (sq + 9);
            }
            bb
        }
        Color::Black => {
            let mut bb = 0u64;
            if f < 7 && sq >= 8 {
                bb |= 1u64 << (sq - 7);
            }
            if f > 0 && sq >= 8 {
                bb |= 1u64 << (sq - 9);
            }
            bb
        }
    }
}

#[inline]
fn piece_bb(b: &Board, piece: Piece) -> Bitboard {
    b.piece_bb[piece.index()]
}

fn front_span(sq: usize, color: Color) -> Bitboard {
    let file = file_of(sq);
    let own_file = FILE_A << file;
    let mask = own_file | ADJACENT_FILES[file];
    match color {
        Color::White => mask & (u64::MAX << (sq + 1)),
        Color::Black => mask & ((1u64 << sq) - 1),
    }
}

#[inline]
fn is_passed(b: &Board, sq: usize, color: Color) -> bool {
    let opp_pawns = if color == Color::White {
        piece_bb(b, Piece::BP)
    } else {
        piece_bb(b, Piece::WP)
    };
    (front_span(sq, color) & opp_pawns) == 0
}

fn is_backward_pawn(b: &Board, sq: usize, color: Color) -> bool {
    if is_passed(b, sq, color) {
        return false;
    }

    let f = file_of(sq);
    let adj_files = ADJACENT_FILES[f];

    let our_pawns = if color == Color::White {
        piece_bb(b, Piece::WP)
    } else {
        piece_bb(b, Piece::BP)
    };

    let support_mask = if color == Color::White {
        let rank_mask = (!0u64) >> (63 - sq); // bits <= sq
        adj_files & rank_mask & our_pawns
    } else {
        let rank_mask = (!0u64) << sq; // bits >= sq
        adj_files & rank_mask & our_pawns
    };

    if support_mask != 0 {
        return false;
    }

    let front_sq = match color {
        Color::White => {
            if sq < 56 {
                Some(sq + 8)
            } else {
                None
            }
        }
        Color::Black => {
            if sq >= 8 {
                Some(sq - 8)
            } else {
                None
            }
        }
    };

    if let Some(fs) = front_sq {
        let opp_pawns = if color == Color::White {
            piece_bb(b, Piece::BP)
        } else {
            piece_bb(b, Piece::WP)
        };
        let attacked_by_opp_pawn = if color == Color::White {
            ((opp_pawns << 7) & !FILE_A | (opp_pawns << 9) & !FILE_H) & bit(fs) != 0
        } else {
            ((opp_pawns >> 7) & !FILE_H | (opp_pawns >> 9) & !FILE_A) & bit(fs) != 0
        };
        return attacked_by_opp_pawn;
    }

    false
}

fn rook_file_bonuses(
    our_rooks: Bitboard,
    our_pawns: Bitboard,
    opp_pawns: Bitboard,
    us: Color,
) -> i32 {
    let mut score = 0;
    let mut rooks = our_rooks;

    while rooks != 0 {
        let sq = rooks.trailing_zeros() as usize;
        rooks &= rooks - 1;

        let file_mask = FILE_A << file_of(sq);
        let open = (file_mask & our_pawns) == 0;

        if open {
            if (file_mask & opp_pawns) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMIOPEN_FILE_BONUS;
            }
        }

        let r = rank_of(sq);
        match us {
            Color::White if r == 6 => score += ROOK_ON_7TH_BONUS,
            Color::Black if r == 1 => score += ROOK_ON_7TH_BONUS,
            _ => (),
        }
    }

    score
}

fn knight_outpost_bonus(b: &Board, sq: usize, us: Color) -> i32 {
    let our_pawns = if us == Color::White {
        piece_bb(b, Piece::WP)
    } else {
        piece_bb(b, Piece::BP)
    };

    let opp_pawns = if us == Color::White {
        piece_bb(b, Piece::BP)
    } else {
        piece_bb(b, Piece::WP)
    };

    let defended_by_pawn = (our_pawns & pawn_attacks_from(sq, us.opposite())) != 0;
    if !defended_by_pawn {
        return 0;
    }

    let attacked_by_opp_pawn = (opp_pawns & pawn_attacks_from(sq, us)) != 0;
    if attacked_by_opp_pawn {
        return 0;
    }

    let r = rank_of(sq);
    match us {
        Color::White => match r {
            5 => KNIGHT_OUTPOST_RANK6_BONUS,
            4 => KNIGHT_OUTPOST_RANK5_BONUS,
            3 => KNIGHT_OUTPOST_RANK4_BONUS,
            _ => 0,
        },
        Color::Black => match r {
            2 => KNIGHT_OUTPOST_RANK6_BONUS,
            3 => KNIGHT_OUTPOST_RANK5_BONUS,
            4 => KNIGHT_OUTPOST_RANK4_BONUS,
            _ => 0,
        },
    }
}

#[inline]
fn king_ring(sq: usize, color: Color) -> Bitboard {
    let base_ring = magics::king_attacks_from(sq);
    let forward_rank = match color {
        Color::White if sq < 56 => magics::king_attacks_from(sq + 8),
        Color::Black if sq >= 8 => magics::king_attacks_from(sq - 8),
        _ => 0,
    };
    base_ring | forward_rank
}

fn evaluate_white_pov(b: &Board) -> i32 {
    let occ_all = b.all_pieces;

    let mut mg_score = 0i32;
    let mut eg_score = 0i32;
    let mut phase_sum = 0i32;

    let wk_sq = piece_bb(b, Piece::WK).trailing_zeros() as usize;
    let bk_sq = piece_bb(b, Piece::BK).trailing_zeros() as usize;

    for sq in 0..64 {
        let p = b.piece_on[sq];
        if p.is_empty() {
            continue;
        }

        let color = p.color().unwrap();
        let kind = p.kind().unwrap();
        let kind_idx = kind as usize;

        let mg_pst = MG_PST[p.index()][sq];
        let eg_pst = EG_PST[p.index()][sq];
        let mat = MATERIAL_VALUES[kind_idx];

        let mut add_mg = mat + mg_pst;
        let mut add_eg = mat + eg_pst;

        if kind == PieceKind::Pawn {
            if color == Color::White && is_passed(b, sq, Color::White) {
                add_mg += PASSED_PAWN_BONUS_MG[rank_of(sq)];
                add_eg += PASSED_PAWN_BONUS_EG[rank_of(sq)];
            } else if color == Color::Black && is_passed(b, sq, Color::Black) {
                add_mg -= PASSED_PAWN_BONUS_MG[7 - rank_of(sq)];
                add_eg -= PASSED_PAWN_BONUS_EG[7 - rank_of(sq)];
            }
        }

        if color == Color::White {
            mg_score += add_mg;
            eg_score += add_eg;
        } else {
            mg_score -= add_mg;
            eg_score -= add_eg;
        }
        phase_sum += PHASE_VALUES[kind_idx];
    }

    let wp = piece_bb(b, Piece::WP);
    let bp = piece_bb(b, Piece::BP);
    for file in 0..8 {
        let file_mask = FILE_A << file;
        let wc = (wp & file_mask).count_ones();
        let bc = (bp & file_mask).count_ones();
        if wc > 1 {
            mg_score -= (wc as i32 - 1) * DOUBLED_PAWN_PENALTY;
        }
        if bc > 1 {
            mg_score += (bc as i32 - 1) * DOUBLED_PAWN_PENALTY;
        }
        if wc > 0 && (wp & ADJACENT_FILES[file]) == 0 {
            mg_score -= ISOLATED_PAWN_PENALTY;
        }
        if bc > 0 && (bp & ADJACENT_FILES[file]) == 0 {
            mg_score += ISOLATED_PAWN_PENALTY;
        }
    }

    let mut wp_tmp = wp;
    while wp_tmp != 0 {
        let sq = wp_tmp.trailing_zeros() as usize;
        wp_tmp &= wp_tmp - 1;
        if is_backward_pawn(b, sq, Color::White) {
            mg_score -= BACKWARD_PAWN_PENALTY;
        }
    }

    let mut bp_tmp = bp;
    while bp_tmp != 0 {
        let sq = bp_tmp.trailing_zeros() as usize;
        bp_tmp &= bp_tmp - 1;
        if is_backward_pawn(b, sq, Color::Black) {
            mg_score += BACKWARD_PAWN_PENALTY;
        }
    }

    if piece_bb(b, Piece::WB).count_ones() >= 2 {
        mg_score += BISHOP_PAIR_BONUS_MG;
        eg_score += BISHOP_PAIR_BONUS_EG;
    }

    if piece_bb(b, Piece::BB).count_ones() >= 2 {
        mg_score -= BISHOP_PAIR_BONUS_MG;
        eg_score -= BISHOP_PAIR_BONUS_EG;
    }

    let wr_bonus = rook_file_bonuses(piece_bb(b, Piece::WR), wp, bp, Color::White);
    mg_score += wr_bonus;
    eg_score += wr_bonus;

    let br_bonus = rook_file_bonuses(piece_bb(b, Piece::BR), bp, wp, Color::Black);
    mg_score -= br_bonus;
    eg_score -= br_bonus;

    let mut wn = piece_bb(b, Piece::WN);
    while wn != 0 {
        let sq = wn.trailing_zeros() as usize;
        wn &= wn - 1;
        mg_score += knight_outpost_bonus(b, sq, Color::White);
    }

    let mut bn = piece_bb(b, Piece::BN);
    while bn != 0 {
        let sq = bn.trailing_zeros() as usize;
        bn &= bn - 1;
        mg_score -= knight_outpost_bonus(b, sq, Color::Black);
    }

    let white_occ = b.w_pieces;
    let black_occ = b.b_pieces;
    let mut center_mg_white = 0i32;
    let mut center_mg_black = 0i32;

    // White pieces
    for p_idx in 1..=6 {
        let p = Piece::from_index(p_idx);
        let mut bb = piece_bb(b, p);
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            bb &= bb - 1;

            let (atk, mob_w, trop_w, kind) = match p.kind().unwrap() {
                PieceKind::Knight => (
                    magics::knight_attacks_from(sq),
                    MOB_KNIGHT,
                    TROPISM_KNIGHT,
                    PieceKind::Knight,
                ),
                PieceKind::Bishop => (
                    magics::get_bishop_attacks(sq, occ_all),
                    MOB_BISHOP,
                    TROPISM_BISHOP,
                    PieceKind::Bishop,
                ),
                PieceKind::Rook => (
                    magics::get_rook_attacks(sq, occ_all),
                    MOB_ROOK,
                    TROPISM_ROOK,
                    PieceKind::Rook,
                ),
                PieceKind::Queen => (
                    magics::get_bishop_attacks(sq, occ_all) | magics::get_rook_attacks(sq, occ_all),
                    MOB_QUEEN,
                    TROPISM_QUEEN,
                    PieceKind::Queen,
                ),
                PieceKind::Pawn => (
                    pawn_attacks_from(sq, Color::White),
                    MobW { mg: 0, eg: 0 },
                    0,
                    PieceKind::Pawn,
                ),
                _ => (0, MobW { mg: 0, eg: 0 }, 0, PieceKind::King),
            };

            if kind != PieceKind::King && kind != PieceKind::Pawn {
                let mob = (atk & !white_occ).count_ones() as i32;
                mg_score += mob_w.mg * mob;
                eg_score += mob_w.eg * mob;
                mg_score += trop_w * (8 - manhattan(sq, bk_sq)).max(0);
            }

            center_mg_white += CENTER_CONTROL_ATT_MG * (atk & CENTER4).count_ones() as i32
                + EXT_CENTER_ATT_MG * (atk & EXT_CENTER).count_ones() as i32;

            if (bit(sq) & CENTER4) != 0 {
                center_mg_white += CENTER_CONTROL_OCC_MG;
            }

            if (bit(sq) & EXT_CENTER) != 0 {
                center_mg_white += EXT_CENTER_OCC_MG;
            }
        }
    }

    // Black pieces
    for p_idx in 7..=12 {
        let p = Piece::from_index(p_idx);
        let mut bb = piece_bb(b, p);
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            bb &= bb - 1;

            let (atk, mob_w, trop_w, kind) = match p.kind().unwrap() {
                PieceKind::Knight => (
                    magics::knight_attacks_from(sq),
                    MOB_KNIGHT,
                    TROPISM_KNIGHT,
                    PieceKind::Knight,
                ),
                PieceKind::Bishop => (
                    magics::get_bishop_attacks(sq, occ_all),
                    MOB_BISHOP,
                    TROPISM_BISHOP,
                    PieceKind::Bishop,
                ),
                PieceKind::Rook => (
                    magics::get_rook_attacks(sq, occ_all),
                    MOB_ROOK,
                    TROPISM_ROOK,
                    PieceKind::Rook,
                ),
                PieceKind::Queen => (
                    magics::get_bishop_attacks(sq, occ_all) | magics::get_rook_attacks(sq, occ_all),
                    MOB_QUEEN,
                    TROPISM_QUEEN,
                    PieceKind::Queen,
                ),
                PieceKind::Pawn => (
                    pawn_attacks_from(sq, Color::Black),
                    MobW { mg: 0, eg: 0 },
                    0,
                    PieceKind::Pawn,
                ),
                _ => (0, MobW { mg: 0, eg: 0 }, 0, PieceKind::King),
            };

            if kind != PieceKind::King && kind != PieceKind::Pawn {
                let mob = (atk & !black_occ).count_ones() as i32;
                mg_score -= mob_w.mg * mob;
                eg_score -= mob_w.eg * mob;
                mg_score -= trop_w * (8 - manhattan(sq, wk_sq)).max(0);
            }

            center_mg_black += CENTER_CONTROL_ATT_MG * (atk & CENTER4).count_ones() as i32
                + EXT_CENTER_ATT_MG * (atk & EXT_CENTER).count_ones() as i32;

            if (bit(sq) & CENTER4) != 0 {
                center_mg_black += CENTER_CONTROL_OCC_MG;
            }

            if (bit(sq) & EXT_CENTER) != 0 {
                center_mg_black += EXT_CENTER_OCC_MG;
            }
        }
    }

    mg_score += center_mg_white - center_mg_black;
    eg_score += (center_mg_white - center_mg_black) * CENTER_CONTROL_SCALE_EG / 3;

    let mut king_safety_score = 0;

    let w_ring = king_ring(wk_sq, Color::White);
    let mut w_attack_score = 0;
    let mut w_attack_count = 0;

    let mut attackers = piece_bb(b, Piece::BN);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if (magics::knight_attacks_from(sq) & w_ring) != 0 {
            w_attack_score += KING_ATTACK_WEIGHTS[1];
            w_attack_count += 1;
        }
    }

    attackers = piece_bb(b, Piece::BB);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if (magics::get_bishop_attacks(sq, occ_all) & w_ring) != 0 {
            w_attack_score += KING_ATTACK_WEIGHTS[2];
            w_attack_count += 1;
        }
    }

    attackers = piece_bb(b, Piece::BR);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if (magics::get_rook_attacks(sq, occ_all) & w_ring) != 0 {
            w_attack_score += KING_ATTACK_WEIGHTS[3];
            w_attack_count += 1;
        }
    }

    attackers = piece_bb(b, Piece::BQ);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if ((magics::get_bishop_attacks(sq, occ_all) | magics::get_rook_attacks(sq, occ_all))
            & w_ring)
            != 0
        {
            w_attack_score += KING_ATTACK_WEIGHTS[4];
            w_attack_count += 1;
        }
    }

    let w_total_penalty = (w_attack_score * ATTACK_COUNT_BONUS[w_attack_count.min(9)]) / 100;
    king_safety_score -= w_total_penalty.min(KING_SAFETY_MAX_PENALTY);

    let wk_file = file_of(wk_sq);
    let shield_files = [wk_file.saturating_sub(1), wk_file, wk_file + 1];
    for &f in &shield_files {
        if f > 7 {
            continue;
        }

        let file_mask = FILE_A << f;
        let shield_sq_mask = file_mask & (0xFF_u64 << (rank_of(wk_sq) * 8 + 8));

        if (wp & shield_sq_mask) == 0 {
            let is_open_file = (bp & file_mask) == 0;

            king_safety_score -= if is_open_file {
                PAWN_SHIELD_MISSING_ON_OPEN_FILE_PENALTY
            } else {
                PAWN_SHIELD_MISSING_PENALTY
            };
        }
    }

    let b_ring = king_ring(bk_sq, Color::Black);
    let mut b_attack_score = 0;
    let mut b_attack_count = 0;

    attackers = piece_bb(b, Piece::WN);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if (magics::knight_attacks_from(sq) & b_ring) != 0 {
            b_attack_score += KING_ATTACK_WEIGHTS[1];
            b_attack_count += 1;
        }
    }

    attackers = piece_bb(b, Piece::WB);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if (magics::get_bishop_attacks(sq, occ_all) & b_ring) != 0 {
            b_attack_score += KING_ATTACK_WEIGHTS[2];
            b_attack_count += 1;
        }
    }

    attackers = piece_bb(b, Piece::WR);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if (magics::get_rook_attacks(sq, occ_all) & b_ring) != 0 {
            b_attack_score += KING_ATTACK_WEIGHTS[3];
            b_attack_count += 1;
        }
    }

    attackers = piece_bb(b, Piece::WQ);
    while attackers != 0 {
        let sq = attackers.trailing_zeros() as usize;
        attackers &= attackers - 1;

        if ((magics::get_bishop_attacks(sq, occ_all) | magics::get_rook_attacks(sq, occ_all))
            & b_ring)
            != 0
        {
            b_attack_score += KING_ATTACK_WEIGHTS[4];
            b_attack_count += 1;
        }
    }

    let b_total_bonus = (b_attack_score * ATTACK_COUNT_BONUS[b_attack_count.min(9)]) / 100;
    king_safety_score += b_total_bonus.min(KING_SAFETY_MAX_PENALTY);

    let bk_file = file_of(bk_sq);
    let shield_files_b = [bk_file.saturating_sub(1), bk_file, bk_file + 1];

    for &f in &shield_files_b {
        if f > 7 {
            continue;
        }

        let file_mask = FILE_A << f;
        let shield_sq_mask = file_mask & (0xFF_u64 << (rank_of(bk_sq) * 8 - 8));

        if (bp & shield_sq_mask) == 0 {
            let is_open_file = (wp & file_mask) == 0;

            king_safety_score += if is_open_file {
                PAWN_SHIELD_MISSING_ON_OPEN_FILE_PENALTY
            } else {
                PAWN_SHIELD_MISSING_PENALTY
            };
        }
    }

    mg_score += king_safety_score;

    let phase = phase_sum.min(MAX_PHASE);
    (mg_score * phase + eg_score * (MAX_PHASE - phase)) / MAX_PHASE
}

trait Opp {
    fn opposite(self) -> Self;
}

impl Opp for Color {
    fn opposite(self) -> Self {
        if self == Color::White {
            Color::Black
        } else {
            Color::White
        }
    }
}

impl Piece {
    fn from_index(idx: usize) -> Piece {
        match idx {
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

use crate::fen;
use crate::types::*;
use crate::zobrist::Zobrist;
use std::mem;

const KNIGHT_DELTAS: [i32; 8] = [6, 10, 15, 17, -6, -10, -15, -17];
const KING_DELTAS: [i32; 8] = [1, -1, 8, -8, 7, 9, -7, -9];
const DIRS: [i32; 8] = [1, -1, 8, -8, 7, 9, -7, -9]; // rook: 0..4, bishop: 4..8

#[derive(Clone)]
pub struct Board {
    pub piece_bb: [Bitboard; 13], // index by Piece::index()
    pub piece_on: [Piece; 64],

    pub w_pieces: Bitboard,
    pub b_pieces: Bitboard,
    pub all_pieces: Bitboard,

    pub turn: Color,
    pub castle: u8,         // WK/WQ/BK/BQ bitmask
    pub en_passant_sq: i32, // NO_SQ if none
    pub halfmove_clock: i32,
    pub fullmove_number: i32,

    pub zobrist: ZKey,
    pub zob: Zobrist,
}

impl Board {
    pub fn empty() -> Self {
        Self {
            piece_bb: [0; 13],
            piece_on: [Piece::Empty; 64],
            w_pieces: 0,
            b_pieces: 0,
            all_pieces: 0,
            turn: Color::White,
            castle: 0,
            en_passant_sq: NO_SQ,
            halfmove_clock: 0,
            fullmove_number: 1,
            zobrist: 0,
            zob: Zobrist::new(),
        }
    }

    pub fn from_fen(fen_str: &str) -> Result<Self, String> {
        fen::parse_fen(fen_str)
    }

    pub fn place_piece(&mut self, p: Piece, sq: usize) {
        self.piece_on[sq] = p;
    }

    pub fn rebuild_derived(&mut self) {
        self.piece_bb = [0; 13];
        self.w_pieces = 0;
        self.b_pieces = 0;
        for sq in 0..64 {
            let p = self.piece_on[sq];
            if !p.is_empty() {
                self.piece_bb[p.index()] |= 1u64 << sq;
                match p.color() {
                    Some(Color::White) => self.w_pieces |= 1u64 << sq,
                    Some(Color::Black) => self.b_pieces |= 1u64 << sq,
                    _ => {}
                }
            }
        }
        self.all_pieces = self.w_pieces | self.b_pieces;
    }

    pub fn recompute_zobrist(&mut self) {
        let mut h = 0u64;
        for sq in 0..64 {
            let p = self.piece_on[sq];
            if !p.is_empty() {
                h ^= self.zob.piece_key(p, sq);
            }
        }
        h ^= self.zob.castle[(self.castle & 0xF) as usize];
        if self.en_passant_sq != NO_SQ {
            h ^= self.zob.ep_file[(self.en_passant_sq % 8) as usize];
        }
        if self.turn == Color::Black {
            h ^= self.zob.side;
        }
        self.zobrist = h;
    }

    #[inline]
    #[allow(dead_code)]
    fn bit_at(&self, sq: usize) -> Bitboard {
        1u64 << sq
    }

    pub fn is_square_attacked(&self, square: i32, by: Color) -> bool {
        // pawns
        let pawn = if by == Color::White {
            Piece::WP
        } else {
            Piece::BP
        };
        let dir = if by == Color::White { -8 } else { 8 };
        let f = file_of(square);

        let s1 = square + dir - 1;
        if f > 0 && in_board(s1) && self.piece_on[s1 as usize] == pawn {
            return true;
        }
        let s2 = square + dir + 1;
        if f < 7 && in_board(s2) && self.piece_on[s2 as usize] == pawn {
            return true;
        }

        // knights
        let n = if by == Color::White {
            Piece::WN
        } else {
            Piece::BN
        };
        for d in KNIGHT_DELTAS {
            let to = square + d;
            if !in_board(to) {
                continue;
            }
            if (file_of(square) - file_of(to)).abs() <= 2 && self.piece_on[to as usize] == n {
                return true;
            }
        }

        // king
        let k = if by == Color::White {
            Piece::WK
        } else {
            Piece::BK
        };
        for d in KING_DELTAS {
            let to = square + d;
            if !in_board(to) {
                continue;
            }
            if (file_of(square) - file_of(to)).abs() <= 1 && self.piece_on[to as usize] == k {
                return true;
            }
        }

        // sliders
        let rook = if by == Color::White {
            Piece::WR
        } else {
            Piece::BR
        };
        let bishop = if by == Color::White {
            Piece::WB
        } else {
            Piece::BB
        };
        let queen = if by == Color::White {
            Piece::WQ
        } else {
            Piece::BQ
        };

        for (d_idx, d) in DIRS.iter().enumerate() {
            let mut to = square;
            loop {
                let prev = to;
                to += d;
                if !in_board(to) {
                    break;
                }

                // prevent horizontal wrapping
                if (d_idx == 0 || d_idx == 1) && rank_of(to) != rank_of(prev) {
                    break;
                }
                // diagonals must change file by exactly 1 each step
                if d_idx >= 4 && (file_of(to) - file_of(prev)).abs() != 1 {
                    break;
                }

                let p = self.piece_on[to as usize];
                if p.is_empty() {
                    continue;
                }
                if d_idx < 4 && (p == rook || p == queen) {
                    return true;
                }
                if d_idx >= 4 && (p == bishop || p == queen) {
                    return true;
                }
                break;
            }
        }

        false
    }

    pub fn generate_legal_moves(&self, out: &mut Vec<Move>) {
        let mut pseudo = Vec::with_capacity(128);
        self.gen_pawns(&mut pseudo);
        self.gen_leapers(&mut pseudo);
        self.gen_sliders(&mut pseudo);

        out.clear();
        let mut tmp = self.clone();
        for m in pseudo {
            let u = tmp.make_move(m);
            // find my king square after move (turn already flipped)
            let opp = tmp.turn;
            let my = opp.other();
            let my_king = if my == Color::White {
                Piece::WK
            } else {
                Piece::BK
            };
            let king_sq = tmp.piece_bb[my_king.index()].trailing_zeros() as i32;

            let in_check = tmp.is_square_attacked(king_sq, opp);
            tmp.unmake_move(u);
            if !in_check {
                out.push(m);
            }
        }
    }

    fn gen_pawns(&self, out: &mut Vec<Move>) {
        let white = self.turn == Color::White;
        let pawn = if white { Piece::WP } else { Piece::BP };
        let pawns = self.piece_bb[pawn.index()];
        let enemy = if white { self.b_pieces } else { self.w_pieces };
        let dir = if white { 8 } else { -8 };
        let start_rank = if white { 1 } else { 6 };
        let promo_rank = if white { 6 } else { 1 };

        let mut bb = pawns;
        while bb != 0 {
            let from = bb.trailing_zeros() as i32;
            bb &= bb - 1;

            let r = rank_of(from);
            let f = file_of(from);

            let to = from + dir;
            if in_board(to) && (self.all_pieces & (1u64 << to)) == 0 {
                if r == promo_rank {
                    for pk in [
                        PieceKind::Queen,
                        PieceKind::Rook,
                        PieceKind::Bishop,
                        PieceKind::Knight,
                    ] {
                        out.push(Move {
                            from: from as u8,
                            to: to as u8,
                            capture: false,
                            en_passant: false,
                            double_push: false,
                            castle: false,
                            promotion: Some(pk),
                        });
                    }
                } else {
                    out.push(Move::quiet(from as u8, to as u8));
                    if r == start_rank {
                        let to2 = from + 2 * dir;
                        if (self.all_pieces & (1u64 << to2)) == 0 {
                            out.push(Move {
                                from: from as u8,
                                to: to2 as u8,
                                capture: false,
                                en_passant: false,
                                double_push: true,
                                castle: false,
                                promotion: None,
                            });
                        }
                    }
                }
            }
            // captures
            for df in [-1, 1] {
                let cap = from + dir + df;
                if (df == -1 && f == 0) || (df == 1 && f == 7) {
                    continue;
                }
                if !in_board(cap) {
                    continue;
                }
                let cap_bb = 1u64 << cap;
                if (enemy & cap_bb) != 0 {
                    if r == promo_rank {
                        for pk in [
                            PieceKind::Queen,
                            PieceKind::Rook,
                            PieceKind::Bishop,
                            PieceKind::Knight,
                        ] {
                            out.push(Move {
                                from: from as u8,
                                to: cap as u8,
                                capture: true,
                                en_passant: false,
                                double_push: false,
                                castle: false,
                                promotion: Some(pk),
                            });
                        }
                    } else {
                        out.push(Move {
                            from: from as u8,
                            to: cap as u8,
                            capture: true,
                            en_passant: false,
                            double_push: false,
                            castle: false,
                            promotion: None,
                        });
                    }
                }
                if self.en_passant_sq == cap {
                    out.push(Move {
                        from: from as u8,
                        to: cap as u8,
                        capture: true,
                        en_passant: true,
                        double_push: false,
                        castle: false,
                        promotion: None,
                    });
                }
            }
        }
    }

    fn gen_leapers(&self, out: &mut Vec<Move>) {
        let white = self.turn == Color::White;
        let friendly = if white { self.w_pieces } else { self.b_pieces };

        // knights
        let kn = if white { Piece::WN } else { Piece::BN };
        let mut bb = self.piece_bb[kn.index()];
        while bb != 0 {
            let from = bb.trailing_zeros() as i32;
            bb &= bb - 1;
            for d in KNIGHT_DELTAS {
                let to = from + d;
                if !in_board(to) {
                    continue;
                }
                if (file_of(from) - file_of(to)).abs() > 2 {
                    continue;
                }
                if (friendly & (1u64 << to)) != 0 {
                    continue;
                }
                let capture = (self.all_pieces & (1u64 << to)) != 0;
                out.push(Move {
                    from: from as u8,
                    to: to as u8,
                    capture,
                    en_passant: false,
                    double_push: false,
                    castle: false,
                    promotion: None,
                });
            }
        }

        // king + castling
        let king = if white { Piece::WK } else { Piece::BK };
        let from = self.piece_bb[king.index()].trailing_zeros() as i32;
        for d in KING_DELTAS {
            let to = from + d;
            if !in_board(to) {
                continue;
            }
            if (file_of(from) - file_of(to)).abs() > 1 {
                continue;
            }
            if (friendly & (1u64 << to)) != 0 {
                continue;
            }
            let capture = (self.all_pieces & (1u64 << to)) != 0;
            out.push(Move {
                from: from as u8,
                to: to as u8,
                capture,
                en_passant: false,
                double_push: false,
                castle: false,
                promotion: None,
            });
        }

        // castling
        if !self.is_square_attacked(from, self.turn.other()) {
            if white {
                // K-side: e1->g1 (4->6), rook h1 (7->5)
                if (self.castle & WK_CASTLE) != 0
                    && (self.all_pieces & ((1u64 << 5) | (1u64 << 6))) == 0
                    && !self.is_square_attacked(5, Color::Black)
                    && !self.is_square_attacked(6, Color::Black)
                    && self.piece_on[7] == Piece::WR
                {
                    out.push(Move {
                        from: 4,
                        to: 6,
                        capture: false,
                        en_passant: false,
                        double_push: false,
                        castle: true,
                        promotion: None,
                    });
                }
                // Q-side: e1->c1 (4->2), rook a1 (0->3)
                if (self.castle & WQ_CASTLE) != 0
                    && (self.all_pieces & ((1u64 << 1) | (1u64 << 2) | (1u64 << 3))) == 0
                    && !self.is_square_attacked(3, Color::Black)
                    && !self.is_square_attacked(2, Color::Black)
                    && self.piece_on[0] == Piece::WR
                {
                    out.push(Move {
                        from: 4,
                        to: 2,
                        capture: false,
                        en_passant: false,
                        double_push: false,
                        castle: true,
                        promotion: None,
                    });
                }
            } else {
                // K-side: e8->g8 (60->62), rook h8 (63->61)
                if (self.castle & BK_CASTLE) != 0
                    && (self.all_pieces & ((1u64 << 61) | (1u64 << 62))) == 0
                    && !self.is_square_attacked(61, Color::White)
                    && !self.is_square_attacked(62, Color::White)
                    && self.piece_on[63] == Piece::BR
                {
                    out.push(Move {
                        from: 60,
                        to: 62,
                        capture: false,
                        en_passant: false,
                        double_push: false,
                        castle: true,
                        promotion: None,
                    });
                }
                // Q-side: e8->c8 (60->58), rook a8 (56->59)
                if (self.castle & BQ_CASTLE) != 0
                    && (self.all_pieces & ((1u64 << 57) | (1u64 << 58) | (1u64 << 59))) == 0
                    && !self.is_square_attacked(59, Color::White)
                    && !self.is_square_attacked(58, Color::White)
                    && self.piece_on[56] == Piece::BR
                {
                    out.push(Move {
                        from: 60,
                        to: 58,
                        capture: false,
                        en_passant: false,
                        double_push: false,
                        castle: true,
                        promotion: None,
                    });
                }
            }
        }
    }

    fn gen_sliders(&self, out: &mut Vec<Move>) {
        let white = self.turn == Color::White;
        let friendly = if white { self.w_pieces } else { self.b_pieces };
        let enemy = if white { self.b_pieces } else { self.w_pieces };

        // bishop, rook, queen
        let gens = [
            if white { Piece::WB } else { Piece::BB }, // diagonals 4..8
            if white { Piece::WR } else { Piece::BR }, // orth 0..4
            if white { Piece::WQ } else { Piece::BQ }, // both 0..8
        ];
        let ranges = [(4, 8), (0, 4), (0, 8)];

        for (idx, p) in gens.iter().enumerate() {
            let (s, e) = ranges[idx];
            let mut bb = self.piece_bb[p.index()];
            while bb != 0 {
                let from = bb.trailing_zeros() as i32;
                bb &= bb - 1;
                for d_idx in s..e {
                    let d = DIRS[d_idx];
                    let mut to = from;
                    loop {
                        let prev = to;
                        to += d;
                        if !in_board(to) {
                            break;
                        }
                        if (d_idx == 0 || d_idx == 1) && rank_of(to) != rank_of(prev) {
                            break;
                        }
                        if d_idx >= 4 && (file_of(to) - file_of(prev)).abs() != 1 {
                            break;
                        }
                        if (friendly & (1u64 << to)) != 0 {
                            break;
                        }
                        if (enemy & (1u64 << to)) != 0 {
                            out.push(Move {
                                from: from as u8,
                                to: to as u8,
                                capture: true,
                                en_passant: false,
                                double_push: false,
                                castle: false,
                                promotion: None,
                            });
                            break;
                        }
                        out.push(Move::quiet(from as u8, to as u8));
                    }
                }
            }
        }
    }

    pub fn make_move(&mut self, m: Move) -> Undo {
        // Snapshot (simple & correct; you can optimize later)
        let snap = Box::new(self.clone());

        // --- Clear EP zobrist if present
        if self.en_passant_sq != NO_SQ {
            self.zobrist ^= self.zob.ep_file[(self.en_passant_sq % 8) as usize];
        }
        self.en_passant_sq = NO_SQ;

        let from = m.from as usize;
        let to = m.to as usize;
        let moving = self.piece_on[from];

        // remove moving piece (zob & bb)
        self.zobrist ^= self.zob.piece_key(moving, from);
        self.piece_on[from] = Piece::Empty;
        self.piece_bb[moving.index()] ^= (1u64 << from);
        match moving.color() {
            Some(Color::White) => self.w_pieces ^= 1u64 << from,
            Some(Color::Black) => self.b_pieces ^= 1u64 << from,
            _ => {}
        }

        // capture (includes EP)
        if m.capture {
            let cap_sq = if m.en_passant {
                if self.turn == Color::White {
                    (to as i32) - 8
                } else {
                    (to as i32) + 8
                }
            } else {
                to as i32
            } as usize;
            let captured = self.piece_on[cap_sq];
            if !captured.is_empty() {
                self.zobrist ^= self.zob.piece_key(captured, cap_sq);
                self.piece_on[cap_sq] = Piece::Empty;
                self.piece_bb[captured.index()] ^= 1u64 << cap_sq;
                match captured.color() {
                    Some(Color::White) => self.w_pieces ^= 1u64 << cap_sq,
                    Some(Color::Black) => self.b_pieces ^= 1u64 << cap_sq,
                    _ => {}
                }
                // castling rights: if rook captured on home square
                match cap_sq {
                    0 => self.castle &= !WQ_CASTLE,
                    7 => self.castle &= !WK_CASTLE,
                    56 => self.castle &= !BQ_CASTLE,
                    63 => self.castle &= !BK_CASTLE,
                    _ => {}
                }
            }
        }

        // place piece at 'to' (promotion handled next)
        self.piece_on[to] = moving;
        self.piece_bb[moving.index()] ^= 1u64 << to;
        match moving.color() {
            Some(Color::White) => self.w_pieces ^= 1u64 << to,
            Some(Color::Black) => self.b_pieces ^= 1u64 << to,
            _ => {}
        }
        self.zobrist ^= self.zob.piece_key(moving, to);

        // promotion
        if let Some(pk) = m.promotion {
            // remove pawn at 'to'
            self.zobrist ^= self.zob.piece_key(moving, to);
            self.piece_on[to] = Piece::Empty;
            self.piece_bb[moving.index()] ^= 1u64 << to;
            match moving.color() {
                Some(Color::White) => self.w_pieces ^= 1u64 << to,
                Some(Color::Black) => self.b_pieces ^= 1u64 << to,
                _ => {}
            }
            // add promoted piece
            let promo = match (self.turn, pk) {
                (Color::White, PieceKind::Queen) => Piece::WQ,
                (Color::White, PieceKind::Rook) => Piece::WR,
                (Color::White, PieceKind::Bishop) => Piece::WB,
                (Color::White, PieceKind::Knight) => Piece::WN,
                (Color::Black, PieceKind::Queen) => Piece::BQ,
                (Color::Black, PieceKind::Rook) => Piece::BR,
                (Color::Black, PieceKind::Bishop) => Piece::BB,
                (Color::Black, PieceKind::Knight) => Piece::BN,
                _ => unreachable!(),
            };
            self.piece_on[to] = promo;
            self.piece_bb[promo.index()] ^= 1u64 << to;
            match promo.color() {
                Some(Color::White) => self.w_pieces ^= 1u64 << to,
                Some(Color::Black) => self.b_pieces ^= 1u64 << to,
                _ => {}
            }
            self.zobrist ^= self.zob.piece_key(promo, to);
            self.halfmove_clock = 0;
        } else if m.castle {
            // move rook
            let (rook_from, rook_to) = if to > from {
                (to + 1, to - 1)
            } else {
                (to - 2, to + 1)
            };
            let rook_piece = self.piece_on[rook_from];
            debug_assert!(matches!(rook_piece.kind(), Some(PieceKind::Rook)));
            self.zobrist ^= self.zob.piece_key(rook_piece, rook_from);
            self.zobrist ^= self.zob.piece_key(rook_piece, rook_to);
            self.piece_on[rook_from] = Piece::Empty;
            self.piece_on[rook_to] = rook_piece;
            self.piece_bb[rook_piece.index()] ^= (1u64 << rook_from) | (1u64 << rook_to);
            match rook_piece.color().unwrap() {
                Color::White => self.w_pieces ^= (1u64 << rook_from) | (1u64 << rook_to),
                Color::Black => self.b_pieces ^= (1u64 << rook_from) | (1u64 << rook_to),
            }
            self.halfmove_clock += 1;
        } else {
            if m.double_push {
                let ep = if self.turn == Color::White {
                    (from as i32) + 8
                } else {
                    (from as i32) - 8
                };
                self.en_passant_sq = ep;
                self.zobrist ^= self.zob.ep_file[(ep % 8) as usize];
            }
            // halfmove
            if matches!(moving.kind(), Some(PieceKind::Pawn)) || m.capture {
                self.halfmove_clock = 0;
            } else {
                self.halfmove_clock += 1;
            }
        }

        // castling rights update by moved piece/origin/dest
        match moving {
            Piece::WK => self.castle &= !(WK_CASTLE | WQ_CASTLE),
            Piece::BK => self.castle &= !(BK_CASTLE | BQ_CASTLE),
            _ => {}
        }
        match from {
            0 | _ if to == 0 => self.castle &= !WQ_CASTLE,
            7 | _ if to == 7 => self.castle &= !WK_CASTLE,
            56 | _ if to == 56 => self.castle &= !BQ_CASTLE,
            63 | _ if to == 63 => self.castle &= !BK_CASTLE,
            _ => {}
        }

        // zobrist castle update
        self.zobrist ^= self.zob.castle[(snap.castle & 0xF) as usize];
        self.zobrist ^= self.zob.castle[(self.castle & 0xF) as usize];

        // update all_pieces
        self.all_pieces = self.w_pieces | self.b_pieces;

        // side to move & fullmove
        self.zobrist ^= self.zob.side;
        if self.turn == Color::Black {
            self.fullmove_number += 1;
        }
        self.turn = self.turn.other();

        Undo { snap }
    }

    pub fn unmake_move(&mut self, u: Undo) {
        // Restore exactly
        let _ = mem::replace(self, *u.snap);
    }

    pub fn to_fen(&self) -> String {
        fen::to_fen(self)
    }
}

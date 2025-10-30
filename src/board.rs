use crate::fen;
use crate::magics;
use crate::types::*;
use crate::zobrist::Zobrist;

const KNIGHT_DELTAS: [i32; 8] = [6, 10, 15, 17, -6, -10, -15, -17];
const KING_DELTAS: [i32; 8] = [1, -1, 8, -8, 7, 9, -7, -9];

#[derive(Clone)]
pub struct Board {
    pub piece_bb: [Bitboard; 13],
    pub piece_on: [Piece; 64],
    pub w_pieces: Bitboard,
    pub b_pieces: Bitboard,
    pub all_pieces: Bitboard,
    pub turn: Color,
    pub castle: u8,
    pub en_passant_sq: i32,
    pub halfmove_clock: i32,
    pub fullmove_number: i32,
    pub history: Vec<ZKey>,
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
            history: Vec::with_capacity(128),
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

    pub fn count_repetitions(&self) -> usize {
        let current_key = self.zobrist;
        let mut count = 0;

        for &key in self
            .history
            .iter()
            .rev()
            .take(self.halfmove_clock as usize)
            .skip(1)
        {
            if key == current_key {
                count += 1;
            }
        }

        count
    }

    pub fn is_draw_by_repetition(&self) -> bool {
        self.count_repetitions() >= 2
    }

    pub fn is_square_attacked(&self, square: i32, by: Color) -> bool {
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

        let occ = self.all_pieces;
        let rook_like_attackers = if by == Color::White {
            self.piece_bb[Piece::WR.index()] | self.piece_bb[Piece::WQ.index()]
        } else {
            self.piece_bb[Piece::BR.index()] | self.piece_bb[Piece::BQ.index()]
        };

        let bishop_like_attackers = if by == Color::White {
            self.piece_bb[Piece::WB.index()] | self.piece_bb[Piece::WQ.index()]
        } else {
            self.piece_bb[Piece::BB.index()] | self.piece_bb[Piece::BQ.index()]
        };

        if (magics::get_rook_attacks(square as usize, occ) & rook_like_attackers) != 0 {
            return true;
        }

        if (magics::get_bishop_attacks(square as usize, occ) & bishop_like_attackers) != 0 {
            return true;
        }

        false
    }

    pub fn generate_legal_moves(&mut self, out: &mut Vec<Move>) {
        let mut pseudo = Vec::with_capacity(128);
        self.gen_pawns(&mut pseudo);
        self.gen_leapers(&mut pseudo);
        self.gen_sliders(&mut pseudo);
        out.clear();

        for m in pseudo {
            let u = self.make_move(m);
            let opp = self.turn;
            let my = opp.other();

            let my_king = if my == Color::White {
                Piece::WK
            } else {
                Piece::BK
            };

            let king_sq = self.piece_bb[my_king.index()].trailing_zeros() as i32;
            let in_check = self.is_square_attacked(king_sq, opp);
            self.unmake_move(m, u);

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

        if !self.is_square_attacked(from, self.turn.other()) {
            if white {
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
        let occ = self.all_pieces;
        let b_piece = if white { Piece::WB } else { Piece::BB };

        let mut bb = self.piece_bb[b_piece.index()];
        while bb != 0 {
            let from = bb.trailing_zeros() as usize;
            bb &= bb - 1;

            let mut att = magics::get_bishop_attacks(from, occ) & !friendly;
            while att != 0 {
                let to = att.trailing_zeros() as usize;
                att &= att - 1;
                let capture = (enemy & (1u64 << to)) != 0;

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

        let r_piece = if white { Piece::WR } else { Piece::BR };

        let mut rb = self.piece_bb[r_piece.index()];
        while rb != 0 {
            let from = rb.trailing_zeros() as usize;
            rb &= rb - 1;
            let mut att = magics::get_rook_attacks(from, occ) & !friendly;

            while att != 0 {
                let to = att.trailing_zeros() as usize;
                att &= att - 1;
                let capture = (enemy & (1u64 << to)) != 0;

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

        let q_piece = if white { Piece::WQ } else { Piece::BQ };

        let mut qb = self.piece_bb[q_piece.index()];
        while qb != 0 {
            let from = qb.trailing_zeros() as usize;
            qb &= qb - 1;

            let mut att = (magics::get_rook_attacks(from, occ)
                | magics::get_bishop_attacks(from, occ))
                & !friendly;

            while att != 0 {
                let to = att.trailing_zeros() as usize;
                att &= att - 1;
                let capture = (enemy & (1u64 << to)) != 0;
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
    }

    pub fn make_move(&mut self, m: Move) -> Undo {
        let mut undo = Undo {
            captured_piece: Piece::Empty,
            old_castle: self.castle,
            old_en_passant_sq: self.en_passant_sq,
            old_halfmove_clock: self.halfmove_clock,
        };

        if self.en_passant_sq != NO_SQ {
            self.zobrist ^= self.zob.ep_file[(self.en_passant_sq % 8) as usize];
        }
        self.en_passant_sq = NO_SQ;

        let from = m.from as usize;
        let to = m.to as usize;
        let moving = self.piece_on[from];

        self.zobrist ^= self.zob.piece_key(moving, from);
        self.piece_on[from] = Piece::Empty;
        self.piece_bb[moving.index()] ^= 1u64 << from;

        match moving.color() {
            Some(Color::White) => self.w_pieces ^= 1u64 << from,
            Some(Color::Black) => self.b_pieces ^= 1u64 << from,
            _ => {}
        }

        if m.capture {
            let cap_sq = if m.en_passant {
                if self.turn == Color::White {
                    to - 8
                } else {
                    to + 8
                }
            } else {
                to
            };

            let captured = self.piece_on[cap_sq];
            undo.captured_piece = captured;

            if !captured.is_empty() {
                self.zobrist ^= self.zob.piece_key(captured, cap_sq);
                self.piece_on[cap_sq] = Piece::Empty;
                self.piece_bb[captured.index()] ^= 1u64 << cap_sq;
                match captured.color() {
                    Some(Color::White) => self.w_pieces ^= 1u64 << cap_sq,
                    Some(Color::Black) => self.b_pieces ^= 1u64 << cap_sq,
                    _ => {}
                }
            }
        }

        if let Some(pk) = m.promotion {
            let promoted_piece = Piece::from_kind(pk, self.turn);
            self.piece_on[to] = promoted_piece;
            self.piece_bb[promoted_piece.index()] |= 1u64 << to;
            self.zobrist ^= self.zob.piece_key(promoted_piece, to);
        } else {
            self.piece_on[to] = moving;
            self.piece_bb[moving.index()] |= 1u64 << to;
            self.zobrist ^= self.zob.piece_key(moving, to);
        }

        match moving.color() {
            Some(Color::White) => self.w_pieces |= 1u64 << to,
            Some(Color::Black) => self.b_pieces |= 1u64 << to,
            _ => {}
        }

        if m.castle {
            let (rook_from, rook_to) = if to > from {
                (to + 1, to - 1)
            } else {
                (to - 2, to + 1)
            };

            let rook_piece = self.piece_on[rook_from];
            self.zobrist ^= self.zob.piece_key(rook_piece, rook_from);
            self.zobrist ^= self.zob.piece_key(rook_piece, rook_to);
            self.piece_on[rook_from] = Piece::Empty;
            self.piece_on[rook_to] = rook_piece;

            let rook_bb = (1u64 << rook_from) | (1u64 << rook_to);
            self.piece_bb[rook_piece.index()] ^= rook_bb;

            match rook_piece.color().unwrap() {
                Color::White => self.w_pieces ^= rook_bb,
                Color::Black => self.b_pieces ^= rook_bb,
            }
        }

        if m.double_push {
            let ep = if self.turn == Color::White {
                from + 8
            } else {
                from - 8
            };
            self.en_passant_sq = ep as i32;
            self.zobrist ^= self.zob.ep_file[(ep % 8) as usize];
        }

        if matches!(moving.kind(), Some(PieceKind::Pawn)) || m.capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        self.zobrist ^= self.zob.castle[(self.castle & 0xF) as usize];
        match moving {
            Piece::WK => self.castle &= !(WK_CASTLE | WQ_CASTLE),
            Piece::BK => self.castle &= !(BK_CASTLE | BQ_CASTLE),
            _ => {}
        }

        match from {
            0 => self.castle &= !WQ_CASTLE,
            7 => self.castle &= !WK_CASTLE,
            56 => self.castle &= !BQ_CASTLE,
            63 => self.castle &= !BK_CASTLE,
            _ => {}
        }

        if m.capture {
            match to {
                0 => self.castle &= !WQ_CASTLE,
                7 => self.castle &= !WK_CASTLE,
                56 => self.castle &= !BQ_CASTLE,
                63 => self.castle &= !BK_CASTLE,
                _ => {}
            }
        }
        self.zobrist ^= self.zob.castle[(self.castle & 0xF) as usize];

        self.all_pieces = self.w_pieces | self.b_pieces;
        self.zobrist ^= self.zob.side;
        if self.turn == Color::Black {
            self.fullmove_number += 1;
        }

        self.turn = self.turn.other();
        self.history.push(self.zobrist);

        undo
    }

    pub fn unmake_move(&mut self, m: Move, u: Undo) {
        self.history.pop();
        self.zobrist = *self.history.last().unwrap_or(&0); // Restore previous hash from history
        self.turn = self.turn.other();
        if self.turn == Color::Black {
            self.fullmove_number -= 1;
        }

        self.castle = u.old_castle;
        self.en_passant_sq = u.old_en_passant_sq;
        self.halfmove_clock = u.old_halfmove_clock;

        let from = m.from as usize;
        let to = m.to as usize;

        let mut moving = self.piece_on[to];
        if m.promotion.is_some() {
            let promoted_piece = moving;
            moving = Piece::from_kind(PieceKind::Pawn, self.turn);
            self.piece_on[to] = Piece::Empty;
            self.piece_bb[promoted_piece.index()] ^= 1u64 << to;
        } else {
            self.piece_on[to] = Piece::Empty;
        }

        self.piece_on[from] = moving;
        let move_bb = (1u64 << from) | (1u64 << to);
        self.piece_bb[moving.index()] ^= move_bb;
        match moving.color() {
            Some(Color::White) => self.w_pieces ^= move_bb,
            Some(Color::Black) => self.b_pieces ^= move_bb,
            _ => {}
        }

        if m.capture {
            let captured = u.captured_piece;
            let cap_sq = if m.en_passant {
                if self.turn == Color::White {
                    to - 8
                } else {
                    to + 8
                }
            } else {
                to
            };

            self.piece_on[cap_sq] = captured;
            if !captured.is_empty() {
                self.piece_bb[captured.index()] |= 1u64 << cap_sq;
                match captured.color() {
                    Some(Color::White) => self.w_pieces |= 1u64 << cap_sq,
                    Some(Color::Black) => self.b_pieces |= 1u64 << cap_sq,
                    _ => {}
                }
            }
        }

        if m.castle {
            let (rook_from, rook_to) = if to > from {
                (to + 1, to - 1)
            } else {
                (to - 2, to + 1)
            };

            let rook = self.piece_on[rook_to];
            self.piece_on[rook_from] = rook;
            self.piece_on[rook_to] = Piece::Empty;

            let rook_bb = (1u64 << rook_from) | (1u64 << rook_to);
            self.piece_bb[rook.index()] ^= rook_bb;
            match rook.color().unwrap() {
                Color::White => self.w_pieces ^= rook_bb,
                Color::Black => self.b_pieces ^= rook_bb,
            }
        }

        self.all_pieces = self.w_pieces | self.b_pieces;
    }

    pub fn make_null_move(&mut self) -> Undo {
        let undo = Undo {
            captured_piece: Piece::Empty,
            old_castle: self.castle,
            old_en_passant_sq: self.en_passant_sq,
            old_halfmove_clock: self.halfmove_clock,
        };

        if self.en_passant_sq != NO_SQ {
            self.zobrist ^= self.zob.ep_file[(self.en_passant_sq % 8) as usize];
            self.en_passant_sq = NO_SQ;
        }

        self.turn = self.turn.other();
        self.zobrist ^= self.zob.side;
        self.halfmove_clock += 1;
        self.history.push(self.zobrist);

        undo
    }

    pub fn unmake_null_move(&mut self, u: Undo) {
        self.history.pop();
        self.zobrist = *self.history.last().unwrap_or(&0);
        self.turn = self.turn.other();
        self.en_passant_sq = u.old_en_passant_sq;
        self.halfmove_clock = u.old_halfmove_clock;
    }

    pub fn to_fen(&self) -> String {
        fen::to_fen(self)
    }

    pub fn to_san(&self, m: Move, legal_moves: &[Move]) -> String {
        if m.castle {
            return if m.to > m.from { "O-O" } else { "O-O-O" }.to_string();
        }

        let from = m.from as usize;
        let to = m.to as usize;
        let moving_piece = self.piece_on[from];
        let mut san = String::new();

        if let Some(pk) = moving_piece.kind() {
            match pk {
                PieceKind::Pawn => {
                    if m.capture {
                        san.push(file_char(from));
                    }
                }
                _ => {
                    san.push(pk.to_char_upper());
                    let mut ambiguous_moves = Vec::new();
                    for other_move in legal_moves {
                        let other_from = other_move.from as usize;
                        if self.piece_on[other_from].kind() == Some(pk)
                            && other_from != from
                            && other_move.to == m.to
                        {
                            ambiguous_moves.push(other_move);
                        }
                    }

                    if !ambiguous_moves.is_empty() {
                        let mut file_is_unique = true;
                        let mut rank_is_unique = true;

                        for amb_move in &ambiguous_moves {
                            if file_char(amb_move.from as usize) == file_char(from) {
                                file_is_unique = false;
                            }
                            if rank_char(amb_move.from as usize) == rank_char(from) {
                                rank_is_unique = false;
                            }
                        }

                        if file_is_unique {
                            san.push(file_char(from));
                        } else if rank_is_unique {
                            san.push(rank_char(from));
                        } else {
                            san.push_str(&sq_to_str(from));
                        }
                    }
                }
            }
        }

        if m.capture {
            san.push('x');
        }

        san.push_str(&sq_to_str(to));

        if let Some(promo) = m.promotion {
            san.push('=');
            san.push(promo.to_char_upper());
        }

        let mut temp_board = self.clone();
        let undo = temp_board.make_move(m);

        let opp_king_sq = temp_board.piece_bb
            [Piece::from_kind(PieceKind::King, temp_board.turn).index()]
        .trailing_zeros() as i32;

        if temp_board.is_square_attacked(opp_king_sq, self.turn) {
            let mut has_legal_move = false;
            let mut next_moves = Vec::new();
            temp_board.generate_legal_moves(&mut next_moves);

            if !next_moves.is_empty() {
                has_legal_move = true;
            }

            if has_legal_move {
                san.push('+');
            } else {
                san.push('#');
            }
        }

        temp_board.unmake_move(m, undo);

        san
    }
}

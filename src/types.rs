use std::fmt;

pub type Bitboard = u64;
pub type ZKey = u64;

pub const NO_SQ: i32 = -1;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    #[inline]
    pub fn other(self) -> Color {
        if self == Color::White {
            Color::Black
        } else {
            Color::White
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Piece {
    Empty,
    WP,
    WN,
    WB,
    WR,
    WQ,
    WK,
    BP,
    BN,
    BB,
    BR,
    BQ,
    BK,
}

impl Piece {
    #[inline]
    pub fn is_empty(self) -> bool {
        matches!(self, Piece::Empty)
    }
    #[inline]
    pub fn color(self) -> Option<Color> {
        match self {
            Piece::WP | Piece::WN | Piece::WB | Piece::WR | Piece::WQ | Piece::WK => {
                Some(Color::White)
            }
            Piece::BP | Piece::BN | Piece::BB | Piece::BR | Piece::BQ | Piece::BK => {
                Some(Color::Black)
            }
            _ => None,
        }
    }
    #[inline]
    pub fn kind(self) -> Option<PieceKind> {
        match self {
            Piece::WP | Piece::BP => Some(PieceKind::Pawn),
            Piece::WN | Piece::BN => Some(PieceKind::Knight),
            Piece::WB | Piece::BB => Some(PieceKind::Bishop),
            Piece::WR | Piece::BR => Some(PieceKind::Rook),
            Piece::WQ | Piece::BQ => Some(PieceKind::Queen),
            Piece::WK | Piece::BK => Some(PieceKind::King),
            _ => None,
        }
    }
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

impl From<char> for Piece {
    fn from(c: char) -> Self {
        match c {
            'P' => Piece::WP,
            'N' => Piece::WN,
            'B' => Piece::WB,
            'R' => Piece::WR,
            'Q' => Piece::WQ,
            'K' => Piece::WK,
            'p' => Piece::BP,
            'n' => Piece::BN,
            'b' => Piece::BB,
            'r' => Piece::BR,
            'q' => Piece::BQ,
            'k' => Piece::BK,
            _ => Piece::Empty,
        }
    }
}
impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = match self {
            Piece::Empty => '.',
            Piece::WP => 'P',
            Piece::WN => 'N',
            Piece::WB => 'B',
            Piece::WR => 'R',
            Piece::WQ => 'Q',
            Piece::WK => 'K',
            Piece::BP => 'p',
            Piece::BN => 'n',
            Piece::BB => 'b',
            Piece::BR => 'r',
            Piece::BQ => 'q',
            Piece::BK => 'k',
        };
        write!(f, "{c}")
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub capture: bool,
    pub en_passant: bool,
    pub double_push: bool,
    pub castle: bool,
    pub promotion: Option<PieceKind>, // Q/R/B/N
}

impl Move {
    #[inline]
    pub fn quiet(from: u8, to: u8) -> Self {
        Self {
            from,
            to,
            capture: false,
            en_passant: false,
            double_push: false,
            castle: false,
            promotion: None,
        }
    }
}

#[derive(Clone)]
pub struct Undo {
    // For simplicity & correctness: full-state snapshot (fast enough for perft and debugging)
    pub snap: Box<super::board::Board>,
}

// castling rights mask (KQkq) -> bit flags
pub const WK_CASTLE: u8 = 1 << 0;
pub const WQ_CASTLE: u8 = 1 << 1;
pub const BK_CASTLE: u8 = 1 << 2;
pub const BQ_CASTLE: u8 = 1 << 3;

// small helpers
#[inline]
pub fn file_of(sq: i32) -> i32 {
    sq & 7
}
#[inline]
pub fn rank_of(sq: i32) -> i32 {
    sq >> 3
}
#[inline]
pub fn in_board(sq: i32) -> bool {
    (0..64).contains(&sq)
}

use crate::types::{Piece, ZKey};

#[derive(Clone)]
pub struct Zobrist {
    pub piece: [[ZKey; 64]; 13], // Piece::index() 0..12
    pub castle: [ZKey; 16],
    pub ep_file: [ZKey; 8],
    pub side: ZKey,
}

impl Zobrist {
    pub fn new() -> Self {
        fn splitmix64(state: &mut u64) -> u64 {
            *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = *state;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        }
        let mut seed: u64 = 0x1234_5678_9ABC_DEF0;

        let mut piece = [[0u64; 64]; 13];
        for p in 0..13 {
            for sq in 0..64 {
                piece[p][sq] = splitmix64(&mut seed);
            }
        }
        let mut castle = [0u64; 16];
        for c in 0..16 {
            castle[c] = splitmix64(&mut seed);
        }
        let mut ep_file = [0u64; 8];
        for f in 0..8 {
            ep_file[f] = splitmix64(&mut seed);
        }
        let side = splitmix64(&mut seed);
        Self {
            piece,
            castle,
            ep_file,
            side,
        }
    }

    #[inline]
    pub fn piece_key(&self, pc: Piece, sq: usize) -> ZKey {
        self.piece[pc.index()][sq]
    }
}

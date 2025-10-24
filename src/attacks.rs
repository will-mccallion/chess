//! Table-driven rook & bishop attacks without magic numbers.

use crate::types::Bitboard;
use std::sync::OnceLock;

#[inline(always)]
fn bit(sq: usize) -> Bitboard {
    1u64 << sq
}
#[inline(always)]
fn rf_to_sq(r: i32, f: i32) -> usize {
    (r as usize) * 8 + (f as usize)
}

/// Per-square packed attack table for all blocker subsets on its rays.
struct LineAttacks {
    mask: Bitboard,         // relevant ray squares (excluding edges)
    ray_sq_list: [u8; 14],  // order to pack masked blockers into an index
    ray_len: u8,            // number of entries used in ray_sq_list
    table: Box<[Bitboard]>, // dense table of size 2^ray_len
}

impl LineAttacks {
    #[inline(always)]
    fn index_of(&self, masked_blockers: Bitboard) -> usize {
        // Software-PEXT: pack bits in ray_sq_list order
        let mut idx: usize = 0;
        let len = self.ray_len as usize;
        let list = &self.ray_sq_list;
        for i in 0..len {
            let sq = list[i] as usize;
            let bit_is_set = ((masked_blockers >> sq) & 1) as usize;
            idx |= bit_is_set << i;
        }
        idx
    }
}

static ROOKS: OnceLock<Vec<LineAttacks>> = OnceLock::new();
static BISHOPS: OnceLock<Vec<LineAttacks>> = OnceLock::new();

/// Build all tables once.
pub fn init() {
    let rook_tables = (0..64).map(build_rook_table_for).collect::<Vec<_>>();
    let bishop_tables = (0..64).map(build_bishop_table_for).collect::<Vec<_>>();
    let _ = ROOKS.set(rook_tables);
    let _ = BISHOPS.set(bishop_tables);
}

/// Public API
#[inline(always)]
pub fn get_rook_attacks(sq: usize, occupied: Bitboard) -> Bitboard {
    let arr = ROOKS.get().expect("attacks::init() not called");
    let entry = &arr[sq];
    let blockers = occupied & entry.mask;
    let idx = entry.index_of(blockers);
    entry.table[idx]
}

#[inline(always)]
pub fn get_bishop_attacks(sq: usize, occupied: Bitboard) -> Bitboard {
    let arr = BISHOPS.get().expect("attacks::init() not called");
    let entry = &arr[sq];
    let blockers = occupied & entry.mask;
    let idx = entry.index_of(blockers);
    entry.table[idx]
}

fn build_rook_table_for(sq: usize) -> LineAttacks {
    let r0 = (sq / 8) as i32;
    let f0 = (sq % 8) as i32;

    // Collect relevant squares in a stable order: N, S, E, W (excluding edges).
    let mut order: [u8; 14] = [0; 14];
    let mut n = 0usize;

    // North (stop before edge)
    let mut r = r0 + 1;
    while r <= 6 {
        order[n] = rf_to_sq(r, f0) as u8;
        n += 1;
        r += 1;
    }
    // South
    r = r0 - 1;
    while r >= 1 {
        order[n] = rf_to_sq(r, f0) as u8;
        n += 1;
        r -= 1;
    }
    // East
    let mut f = f0 + 1;
    while f <= 6 {
        order[n] = rf_to_sq(r0, f) as u8;
        n += 1;
        f += 1;
    }
    // West
    f = f0 - 1;
    while f >= 1 {
        order[n] = rf_to_sq(r0, f) as u8;
        n += 1;
        f -= 1;
    }

    let mask = build_mask(&order[..n]);
    let table = build_table_for(sq, &order[..n], true);
    LineAttacks {
        mask,
        ray_sq_list: order,
        ray_len: n as u8,
        table,
    }
}

fn build_bishop_table_for(sq: usize) -> LineAttacks {
    let r0 = (sq / 8) as i32;
    let f0 = (sq % 8) as i32;

    let mut order: [u8; 14] = [0; 14];
    let mut n = 0usize;

    // NE
    let (mut r, mut f) = (r0 + 1, f0 + 1);
    while r <= 6 && f <= 6 {
        order[n] = rf_to_sq(r, f) as u8;
        n += 1;
        r += 1;
        f += 1;
    }
    // NW
    (r, f) = (r0 + 1, f0 - 1);
    while r <= 6 && f >= 1 {
        order[n] = rf_to_sq(r, f) as u8;
        n += 1;
        r += 1;
        f -= 1;
    }
    // SE
    (r, f) = (r0 - 1, f0 + 1);
    while r >= 1 && f <= 6 {
        order[n] = rf_to_sq(r, f) as u8;
        n += 1;
        r -= 1;
        f += 1;
    }
    // SW
    (r, f) = (r0 - 1, f0 - 1);
    while r >= 1 && f >= 1 {
        order[n] = rf_to_sq(r, f) as u8;
        n += 1;
        r -= 1;
        f -= 1;
    }

    let mask = build_mask(&order[..n]);
    let table = build_table_for(sq, &order[..n], false);
    LineAttacks {
        mask,
        ray_sq_list: order,
        ray_len: n as u8,
        table,
    }
}

#[inline(always)]
fn build_mask(list: &[u8]) -> Bitboard {
    let mut m = 0u64;
    for &sq in list {
        m |= bit(sq as usize);
    }
    m
}

fn build_table_for(origin_sq: usize, list: &[u8], rook: bool) -> Box<[Bitboard]> {
    let subsets = 1usize << list.len();
    let mut table = vec![0u64; subsets];

    for idx in 0..subsets {
        // Build blockers for this subset
        let mut blockers = 0u64;
        for (i, &sq) in list.iter().enumerate() {
            if (idx >> i) & 1 == 1 {
                blockers |= bit(sq as usize);
            }
        }
        table[idx] = if rook {
            rays_rook_from(origin_sq, blockers)
        } else {
            rays_bishop_from(origin_sq, blockers)
        };
    }
    table.into_boxed_slice()
}

fn rays_rook_from(sq: usize, blockers: Bitboard) -> Bitboard {
    let r0 = (sq / 8) as i32;
    let f0 = (sq % 8) as i32;
    let mut att = 0u64;

    // N
    let mut r = r0 + 1;
    while r <= 7 {
        let b = bit(rf_to_sq(r, f0));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        r += 1;
    }
    // S
    r = r0 - 1;
    while r >= 0 {
        let b = bit(rf_to_sq(r, f0));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        r -= 1;
    }
    // E
    let mut f = f0 + 1;
    while f <= 7 {
        let b = bit(rf_to_sq(r0, f));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        f += 1;
    }
    // W
    f = f0 - 1;
    while f >= 0 {
        let b = bit(rf_to_sq(r0, f));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        f -= 1;
    }
    att
}

fn rays_bishop_from(sq: usize, blockers: Bitboard) -> Bitboard {
    let r0 = (sq / 8) as i32;
    let f0 = (sq % 8) as i32;
    let mut att = 0u64;

    // NE
    let (mut r, mut f) = (r0 + 1, f0 + 1);
    while r <= 7 && f <= 7 {
        let b = bit(rf_to_sq(r, f));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        r += 1;
        f += 1;
    }
    // NW
    (r, f) = (r0 + 1, f0 - 1);
    while r <= 7 && f >= 0 {
        let b = bit(rf_to_sq(r, f));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        r += 1;
        f -= 1;
    }
    // SE
    (r, f) = (r0 - 1, f0 + 1);
    while r >= 0 && f <= 7 {
        let b = bit(rf_to_sq(r, f));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        r -= 1;
        f += 1;
    }
    // SW
    (r, f) = (r0 - 1, f0 - 1);
    while r >= 0 && f >= 0 {
        let b = bit(rf_to_sq(r, f));
        att |= b;
        if blockers & b != 0 {
            break;
        }
        r -= 1;
        f -= 1;
    }
    att
}

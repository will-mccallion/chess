use crate::types::Bitboard;
use std::fs::File;
use std::io::{self, Write};

struct Rng(u64);

impl Rng {
    fn new() -> Self {
        Self(0x1234_5678_9ABC_DEF0)
    }

    fn rand(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn rand_sparse(&mut self) -> u64 {
        self.rand() & self.rand() & self.rand()
    }
}

fn slider_mask(sq: usize, is_rook: bool) -> Bitboard {
    let mut result = 0;
    let r = sq / 8;
    let f = sq % 8;
    let deltas = if is_rook {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
    } else {
        [(1, 1), (1, -1), (-1, 1), (-1, -1)]
    };

    for (dr, df) in deltas {
        let (mut cr, mut cf) = (r as i32 + dr, f as i32 + df);
        while (0..8).contains(&cr) && (0..8).contains(&cf) {
            let next_r = cr + dr;
            let next_f = cf + df;

            if (0..8).contains(&next_r) && (0..8).contains(&next_f) {
                result |= 1u64 << (cr * 8 + cf) as usize;
            }

            cr += dr;
            cf += df;
        }
    }

    result
}

fn slider_attacks(sq: usize, blockers: Bitboard, is_rook: bool) -> Bitboard {
    let mut attacks = 0;
    let r = sq / 8;
    let f = sq % 8;

    let deltas = if is_rook {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
    } else {
        [(1, 1), (1, -1), (-1, 1), (-1, -1)]
    };

    for (dr, df) in deltas {
        let (mut cr, mut cf) = (r as i32 + dr, f as i32 + df);

        while (0..8).contains(&cr) && (0..8).contains(&cf) {
            let to_sq = (cr * 8 + cf) as usize;
            attacks |= 1u64 << to_sq;

            if (blockers & (1u64 << to_sq)) != 0 {
                break;
            }

            cr += dr;
            cf += df;
        }
    }

    attacks
}

fn find_magic_for_sq(sq: usize, is_rook: bool, rng: &mut Rng) -> (u64, Vec<u64>) {
    let mask = slider_mask(sq, is_rook);
    let bits = mask.count_ones();
    let table_size = 1 << bits;

    let mut occupancies = Vec::with_capacity(table_size);
    let mut attacks = Vec::with_capacity(table_size);

    let mut b: Bitboard = 0;

    loop {
        occupancies.push(b);
        attacks.push(slider_attacks(sq, b, is_rook));
        b = (b.wrapping_sub(mask)) & mask;

        if b == 0 {
            break;
        }
    }

    let mut attempts = 0u64;
    loop {
        attempts += 1;
        if attempts.is_multiple_of(100_000) {
            let piece = if is_rook { "Rook" } else { "Bishop" };
            let sq_name = format!(
                "{}{}",
                (b'a' + (sq % 8) as u8) as char,
                (b'1' + (sq / 8) as u8) as char
            );

            eprint!(
                "\rSearching for {} magic on {} (attempts: {})...",
                piece, sq_name, attempts
            );
            io::stderr().flush().unwrap();
        }

        let magic = rng.rand_sparse();
        if (mask.wrapping_mul(magic) >> 56).count_ones() < 6 {
            continue;
        }

        let mut used_indices: Vec<Option<u64>> = vec![None; table_size];
        let mut collision = false;

        for i in 0..table_size {
            let occ = occupancies[i];
            let index = (occ.wrapping_mul(magic) >> (64 - bits)) as usize;
            let current_attack = attacks[i];

            if let Some(existing_attack) = used_indices[index] {
                if existing_attack != current_attack {
                    collision = true;
                    break;
                }
            } else {
                used_indices[index] = Some(current_attack);
            }
        }

        if !collision {
            eprintln!(
                "\rFound {} magic on {} after {} attempts!                ",
                if is_rook { "Rook" } else { "Bishop" },
                format_args!(
                    "{}{}",
                    (b'a' + (sq % 8) as u8) as char,
                    (b'1' + (sq / 8) as u8) as char
                ),
                attempts
            );

            let mut table = vec![0; table_size];
            for i in 0..table_size {
                let occ = occupancies[i];
                let index = (occ.wrapping_mul(magic) >> (64 - bits)) as usize;
                table[index] = attacks[i];
            }
            return (magic, table);
        }
    }
}

pub fn generate_magics_code() {
    let mut rng = Rng::new();
    let mut rook_attack_table = Vec::new();
    let mut bishop_attack_table = Vec::new();

    eprintln!("Finding Rook Magics");
    for sq in 0..64 {
        let (_, mut table) = find_magic_for_sq(sq, true, &mut rng);
        rook_attack_table.append(&mut table);
    }

    eprintln!("\nFinding Bishop Magics");
    for sq in 0..64 {
        let (_, mut table) = find_magic_for_sq(sq, false, &mut rng);
        bishop_attack_table.append(&mut table);
    }

    eprintln!("\nWriting binary attack tables to 'moves/' directory...");
    std::fs::create_dir_all("moves").expect("Failed to create moves/ directory");

    let mut rook_file =
        File::create("moves/rook_attacks.bin").expect("Failed to create rook_attacks.bin");
    for &attack in &rook_attack_table {
        rook_file.write_all(&attack.to_le_bytes()).unwrap();
    }

    let mut bishop_file =
        File::create("moves/bishop_attacks.bin").expect("Failed to create bishop_attacks.bin");
    for &attack in &bishop_attack_table {
        bishop_file.write_all(&attack.to_le_bytes()).unwrap();
    }

    eprintln!(
        "  - Wrote {} bytes to moves/rook_attacks.bin",
        rook_attack_table.len() * 8
    );
    eprintln!(
        "  - Wrote {} bytes to moves/bishop_attacks.bin",
        bishop_attack_table.len() * 8
    );
}

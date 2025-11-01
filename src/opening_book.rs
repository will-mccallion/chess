// FILE: src/opening_book.rs

use crate::board::Board;
use crate::polyglot_zobrist;
use crate::types::{Move, PieceKind};
use byteorder::{BigEndian, ReadBytesExt};
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::OnceLock;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BookEntry {
    pub key: u64,
    pub from: u16,
    pub to: u16,
    pub piece: u16,
    pub promotion: u16,
    pub weight: u16,
    pub _learn: u32,
}

impl BookEntry {
    // Polyglot move encoding is different from the engine's.
    fn to_move(&self) -> Option<Move> {
        let from_sq = self.from as u8;
        let to_sq = self.to as u8;

        let promo_kind = match self.promotion {
            1 => Some(PieceKind::Knight),
            2 => Some(PieceKind::Bishop),
            3 => Some(PieceKind::Rook),
            4 => Some(PieceKind::Queen),
            _ => None,
        };

        let mut m = Move::quiet(from_sq, to_sq);
        m.promotion = promo_kind;
        Some(m)
    }
}

pub struct OpeningBook {
    entries: Vec<BookEntry>,
}

impl OpeningBook {
    fn new(path: &str) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();

        // Polyglot entries are 16 bytes each
        let mut buffer = [0; 16];

        while let Ok(()) = reader.read_exact(&mut buffer) {
            let key = (&buffer[0..8]).read_u64::<BigEndian>()?;
            let raw_move = (&buffer[8..10]).read_u16::<BigEndian>()?;
            let weight = (&buffer[10..12]).read_u16::<BigEndian>()?;
            let learn = (&buffer[12..16]).read_u32::<BigEndian>()?;

            // Decode move data from raw_move
            let to_sq = raw_move & 0x3F;
            let from_sq = (raw_move >> 6) & 0x3F;
            let promo_piece = (raw_move >> 12) & 0x7;

            // This is a custom decoding based on common polyglot move formats
            // It might need adjustments depending on the book generator
            let entry = BookEntry {
                key,
                from: from_sq as u16,
                to: to_sq as u16,
                piece: 0, // Not needed for move conversion
                promotion: promo_piece,
                weight,
                _learn: learn,
            };
            entries.push(entry);
        }

        Ok(OpeningBook { entries })
    }

    fn find_moves(&self, key: u64) -> Vec<Move> {
        let mut moves = Vec::new();
        // Binary search to find the first matching entry
        match self.entries.binary_search_by_key(&key, |e| e.key) {
            Ok(mut index) => {
                let start_index = index;
                // Found at least one entry, go back to find the first one
                while index > 0 && self.entries[index - 1].key == key {
                    index -= 1;
                }
                // Iterate forward and collect all moves for this position
                while index < self.entries.len() && self.entries[index].key == key {
                    if let Some(m) = self.entries[index].to_move() {
                        moves.push(m);
                    }
                    index += 1;
                }

                println!(
                    "info string Found {} potential move(s) in book.",
                    index - start_index
                );
            }
            Err(_) => {
                // No entry found
            }
        }
        moves
    }
}

static BOOK: OnceLock<Option<OpeningBook>> = OnceLock::new();

fn get_book() -> &'static Option<OpeningBook> {
    BOOK.get_or_init(|| {
        // Find the path of the running executable
        if let Ok(mut exe_path) = std::env::current_exe() {
            // Go to its parent directory
            if exe_path.pop() {
                // Append the book file name
                exe_path.push("book.bin");
                // Check if the book exists at that path before trying to load
                if exe_path.exists() {
                    println!("info string Found book at: {}", exe_path.display());
                    // Load the book using the full path
                    return OpeningBook::new(exe_path.to_str().unwrap()).ok();
                } else {
                    println!("info string Book not found at: {}", exe_path.display());
                }
            }
        }
        // Fallback for safety, though it will likely fail in a GUI
        println!("info string Could not determine executable path. Falling back to relative path 'book.bin'.");
        OpeningBook::new("book.bin").ok()
    })
}

pub fn get_book_move(b: &Board) -> Option<String> {
    if let Some(book) = get_book() {
        let key = polyglot_zobrist::calculate_key(b);
        println!("info string Searching book for key: {:x}", key);
        let moves = book.find_moves(key);

        if moves.is_empty() {
            println!("info string No moves found in book for this key.");
            return None;
        }

        // For simplicity, we'll pick the first move.
        // A more advanced engine might pick randomly based on weights.
        let best_move = moves[0];

        let from_sq = best_move.from;
        let to_sq = best_move.to;

        let from_file = (from_sq % 8) + b'a';
        let from_rank = (from_sq / 8) + b'1';
        let to_file = (to_sq % 8) + b'a';
        let to_rank = (to_sq / 8) + b'1';

        let mut uci_move = format!(
            "{}{}{}{}",
            from_file as char, from_rank as char, to_file as char, to_rank as char
        );

        if let Some(promo) = best_move.promotion {
            uci_move.push(match promo {
                PieceKind::Queen => 'q',
                PieceKind::Rook => 'r',
                PieceKind::Bishop => 'b',
                PieceKind::Knight => 'n',
                _ => unreachable!(),
            });
        }

        Some(uci_move)
    } else {
        None
    }
}

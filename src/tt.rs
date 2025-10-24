use crate::types::{Move, ZKey};

/// What kind of bound the stored score represents.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Bound {
    /// Exact score (PV node).
    Exact,
    /// Lower bound (fail-high).
    Lower,
    /// Upper bound (fail-low).
    Upper,
}

/// One transposition-table entry. Keep this tight and POD-like.
#[derive(Copy, Clone, Debug)]
pub struct TTEntry {
    pub key: ZKey,               // full 64-bit Zobrist to avoid false positives
    pub depth: i16,              // search depth this entry was computed at
    pub score: i32,              // score in centipawns from the side to move's POV
    pub bound: Bound,            // Exact / Lower / Upper
    pub best_move: Option<Move>, // move that caused beta-cutoff or PV move
    pub age: u8,                 // simple generation counter for replacement
}

/// A very simple, fixed-size transposition table:
/// - power-of-two number of slots
/// - 1 entry per bucket (always replace if deeper or entry is stale)
pub struct TransTable {
    slots: Vec<Option<TTEntry>>,
    mask: usize,
    age: u8,
}

impl TransTable {
    /// Create a TT with an approximate memory budget in megabytes.
    /// We pack entries in ~32 bytes, so this gives a rough sizing.
    pub fn with_mb(mb: usize) -> Self {
        // Guard against tiny/huge values; aim for a power of two slot count.
        let bytes = mb.saturating_mul(1024 * 1024).max(32);
        let mut slots = (bytes / 32).max(1);
        slots = slots.next_power_of_two();
        let mask = (slots - 1) as usize;

        Self {
            slots: vec![None; slots],
            mask,
            age: 0,
        }
    }

    /// Optional alias.
    pub fn new(size_mb: usize) -> Self {
        Self::with_mb(size_mb)
    }

    /// Bump the generation; useful between searches to prefer fresh entries.
    #[inline]
    pub fn tick_age(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    /// Clear all entries and advance age (so stale entries lose on replacement).
    pub fn clear(&mut self) {
        for s in &mut self.slots {
            *s = None;
        }
        self.tick_age();
    }

    #[inline]
    fn idx(&self, key: ZKey) -> usize {
        // Use low bits; Zobrist is uniform enough for this simple mask.
        (key as usize) & self.mask
    }

    /// Probe the TT for an entry with this exact key.
    #[inline]
    pub fn probe(&self, key: ZKey) -> Option<TTEntry> {
        let i = self.idx(key);
        match self.slots[i] {
            Some(e) if e.key == key => Some(e),
            _ => None,
        }
    }

    /// Store/replace an entry using a simple "deeper or newer" policy.
    pub fn store(
        &mut self,
        key: ZKey,
        depth: i16,
        score: i32,
        bound: Bound,
        best_move: Option<Move>,
    ) {
        let i = self.idx(key);
        let new = TTEntry {
            key,
            depth,
            score,
            bound,
            best_move,
            age: self.age,
        };

        match self.slots[i] {
            None => self.slots[i] = Some(new),
            Some(old) => {
                // Prefer deeper results, or overwrite stale generations.
                if depth > old.depth || old.age != self.age {
                    self.slots[i] = Some(new);
                }
            }
        }
    }

    pub fn hashfull_permill(&self) -> u32 {
        let filled = self.slots.iter().filter(|e| e.is_some()).count();
        ((filled * 1000) / self.slots.len()) as u32
    }
}

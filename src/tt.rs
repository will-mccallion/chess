use crate::types::{Move, ZKey};
use std::sync::{Arc, Mutex};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Copy, Clone, Debug)]
pub struct TTEntry {
    pub key: ZKey,
    pub depth: i16,
    pub score: i32,
    pub bound: Bound,
    pub best_move: Option<Move>,
    pub age: u8,
}

pub struct TransTable {
    slots: Vec<Option<TTEntry>>,
    mask: usize,
    age: u8,
}

impl TransTable {
    fn with_mb(mb: usize) -> Self {
        let bytes = mb.saturating_mul(1024 * 1024).max(32);
        let mut slots = (bytes / 32).max(1);
        slots = slots.next_power_of_two();
        let mask = slots - 1;
        Self {
            slots: vec![None; slots],
            mask,
            age: 0,
        }
    }

    #[inline]
    fn tick_age(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    fn clear(&mut self) {
        for s in &mut self.slots {
            *s = None;
        }
        self.tick_age();
    }

    #[inline]
    fn idx(&self, key: ZKey) -> usize {
        (key as usize) & self.mask
    }

    #[inline]
    fn probe(&self, key: ZKey) -> Option<TTEntry> {
        let i = self.idx(key);
        match self.slots[i] {
            Some(e) if e.key == key => Some(e),
            _ => None,
        }
    }

    fn store(&mut self, key: ZKey, depth: i16, score: i32, bound: Bound, best_move: Option<Move>) {
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
            None => {
                self.slots[i] = Some(new);
            }
            Some(old) => {
                if self.age != old.age || depth >= old.depth {
                    self.slots[i] = Some(new);
                }
            }
        }
    }

    #[inline]
    fn stats(&self) -> (usize, usize) {
        let filled = self.slots.iter().filter(|e| e.is_some()).count();
        (filled, self.slots.len())
    }
}

#[derive(Clone)]
pub struct SharedTransTable {
    shards: Vec<Arc<Mutex<TransTable>>>,
    shard_mask: usize,
}

impl SharedTransTable {
    pub fn new(size_mb: usize) -> Self {
        let shard_count = Self::pick_shard_count();
        let (per_shard, remainder) = if shard_count == 0 {
            (size_mb, 0)
        } else {
            (size_mb / shard_count, size_mb % shard_count)
        };

        let mut shards = Vec::with_capacity(shard_count.max(1));
        let count = shard_count.max(1);
        for i in 0..count {
            let mb = per_shard + if i < remainder { 1 } else { 0 };
            let alloc_mb = mb.max(1);
            shards.push(Arc::new(Mutex::new(TransTable::with_mb(alloc_mb))));
        }

        let shard_mask = count.saturating_sub(1);

        Self { shards, shard_mask }
    }

    #[inline]
    fn pick_shard_count() -> usize {
        let cpus = num_cpus::get().max(1);
        let target = (cpus / 8) + 1;
        target.next_power_of_two().min(8)
    }

    #[inline]
    fn shard_index(&self, key: ZKey) -> usize {
        let mut x = key as u64;
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51afd7ed558ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
        x ^= x >> 33;
        (x as usize) & self.shard_mask
    }

    #[inline]
    fn shard_for(&self, key: ZKey) -> &Arc<Mutex<TransTable>> {
        let idx = if self.shards.len().is_power_of_two() {
            self.shard_index(key)
        } else {
            (key as usize) % self.shards.len()
        };
        &self.shards[idx]
    }

    pub fn probe(&self, key: ZKey) -> Option<TTEntry> {
        let guard = self.shard_for(key).lock().unwrap();
        guard.probe(key)
    }

    pub fn store(&self, key: ZKey, depth: i16, score: i32, bound: Bound, best_move: Option<Move>) {
        let mut guard = self.shard_for(key).lock().unwrap();
        guard.store(key, depth, score, bound, best_move);
    }

    pub fn clear(&self) {
        for shard in &self.shards {
            shard.lock().unwrap().clear();
        }
    }

    pub fn tick_age(&self) {
        for shard in &self.shards {
            shard.lock().unwrap().tick_age();
        }
    }

    pub fn hashfull_permill(&self) -> u32 {
        let mut filled_total = 0usize;
        let mut slots_total = 0usize;
        for shard in &self.shards {
            let guard = shard.lock().unwrap();
            let (filled, total) = guard.stats();
            filled_total += filled;
            slots_total += total;
        }

        if slots_total == 0 {
            0
        } else {
            ((filled_total * 1000) / slots_total) as u32
        }
    }
}

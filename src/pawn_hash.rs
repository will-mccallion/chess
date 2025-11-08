use crate::types::ZKey;
use num_cpus;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Copy, Clone, Default)]
struct PawnEntry {
    key: ZKey,
    mg: i16,
    eg: i16,
}

struct PawnTable {
    slots: Vec<PawnEntry>,
    mask: usize,
}

impl PawnTable {
    fn with_mb(size_mb: usize) -> Self {
        let bytes = (size_mb.max(1)) * 1024 * 1024;
        let num_entries = (bytes / std::mem::size_of::<PawnEntry>()).next_power_of_two();
        Self {
            slots: vec![PawnEntry::default(); num_entries],
            mask: num_entries - 1,
        }
    }

    #[inline]
    fn idx(&self, key: ZKey) -> usize {
        (key as usize) & self.mask
    }

    #[inline]
    fn probe(&self, key: ZKey) -> Option<(i32, i32)> {
        let entry = &self.slots[self.idx(key)];
        if entry.key == key {
            Some((entry.mg as i32, entry.eg as i32))
        } else {
            None
        }
    }

    #[inline]
    fn store(&mut self, key: ZKey, mg: i32, eg: i32) {
        let idx = self.idx(key);
        self.slots[idx] = PawnEntry {
            key,
            mg: mg as i16,
            eg: eg as i16,
        };
    }
}

pub struct SharedPawnTable {
    shards: Vec<Arc<Mutex<PawnTable>>>,
    shard_mask: usize,
}

impl SharedPawnTable {
    pub fn new(size_mb: usize) -> Self {
        let shard_count = (num_cpus::get().max(1)).next_power_of_two();
        let per_shard_mb = (size_mb / shard_count).max(1);
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Arc::new(Mutex::new(PawnTable::with_mb(per_shard_mb))));
        }
        Self {
            shards,
            shard_mask: shard_count - 1,
        }
    }

    #[inline]
    fn shard_for(&self, key: ZKey) -> &Arc<Mutex<PawnTable>> {
        &self.shards[(key as usize) & self.shard_mask]
    }

    #[inline]
    pub fn probe(&self, key: ZKey) -> Option<(i32, i32)> {
        self.shard_for(key).lock().unwrap().probe(key)
    }

    #[inline]
    pub fn store(&self, key: ZKey, mg: i32, eg: i32) {
        self.shard_for(key).lock().unwrap().store(key, mg, eg);
    }
}

static PAWN_TT: OnceLock<SharedPawnTable> = OnceLock::new();

pub fn pawn_tt() -> &'static SharedPawnTable {
    PAWN_TT.get_or_init(|| SharedPawnTable::new(64)) // Default to 64 Slight increase.
}

#[derive(Copy, Clone, Default)]
pub struct TimeControl {
    pub wtime: i64,
    pub btime: i64,
    pub winc: i64,
    pub binc: i64,
    pub movestogo: i32,
    pub move_overhead_ms: i64,
}

impl TimeControl {
    /// Calculates the optimal and maximum time to think for the current move in milliseconds.
    pub fn allocation_ms(&self, side_white: bool) -> (i64, i64) {
        let (time, inc) = if side_white {
            (self.wtime, self.winc)
        } else {
            (self.btime, self.binc)
        };

        if self.movestogo > 0 {
            let divisor = (self.movestogo as i64).min(30);
            let ideal_time = (time / divisor) + (inc * 3 / 4);
            let safe_time = time - self.move_overhead_ms.max(50);
            return (ideal_time.min(safe_time), safe_time);
        }

        let moves_remaining = 40;
        let ideal_time = (time / moves_remaining) + (inc * 3 / 4);

        let max_time = time / 5;

        let hard_limit = time - self.move_overhead_ms.max(50);

        let soft_limit = ideal_time.min(max_time).min(hard_limit).max(5); // Think for at least 5ms

        (soft_limit, hard_limit)
    }
}

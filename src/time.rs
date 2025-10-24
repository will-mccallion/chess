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
    pub fn allocation_ms(&self, side_white: bool) -> (i64, i64) {
        let (time, inc) = if side_white {
            (self.wtime, self.winc)
        } else {
            (self.btime, self.binc)
        };
        let mtg = if self.movestogo > 0 {
            self.movestogo as i64
        } else {
            30
        }; // default bucket
        let overhead = self.move_overhead_ms.max(5);
        // basic split: 1/(mtg+7) of remaining plus some inc
        let soft = (time / (mtg + 7)).max(10) + (inc * 3 / 4);
        let hard = (soft * 3 / 2).min(time - overhead).max(soft + overhead);
        (soft.max(5), hard.max(soft + 5))
    }
}

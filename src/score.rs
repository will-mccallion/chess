#![allow(clippy::manual_range_contains)]

pub const MATE: i32 = 30_000; // "matish" sentinel, far from i32::MAX
pub const INF: i32 = 29_000; // search infinity
pub const MATE_IN_MAX: i32 = MATE - 512; // room for ply offsets

#[inline]
pub fn clamp_eval(s: i32) -> i32 {
    s.max(-INF).min(INF)
}

#[inline]
pub fn is_mate_score(s: i32) -> bool {
    s.abs() >= MATE_IN_MAX
}

#[inline]
pub fn mate_store(s: i32, ply: i32) -> i32 {
    if s > MATE_IN_MAX {
        s + ply
    } else if s < -MATE_IN_MAX {
        s - ply
    } else {
        s
    }
}

#[inline]
pub fn mate_load(s: i32, ply: i32) -> i32 {
    if s > MATE_IN_MAX {
        s - ply
    } else if s < -MATE_IN_MAX {
        s + ply
    } else {
        s
    }
}

#[inline]
pub fn to_uci_score(s: i32) -> String {
    let s = clamp_eval(s);
    if is_mate_score(s) {
        let plies = MATE - s.abs();
        if s > 0 {
            format!("mate {}", (plies + 1) / 2)
        } else {
            format!("mate -{}", (plies + 1) / 2)
        }
    } else {
        format!("cp {}", s)
    }
}

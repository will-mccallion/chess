use crate::board::Board;
use crate::types::Move;

pub fn best_move_depth(b: &Board, depth: usize) -> Option<Move> {
    // copy because we’ll mutate during search
    let mut pos = b.clone();
    let mut best: Option<Move> = None;
    let mut best_score = i32::MIN;

    let mut moves = Vec::new();
    pos.generate_legal_moves(&mut moves);
    if moves.is_empty() {
        return None;
    }

    for m in moves {
        let u = pos.make_move(m);
        let score = -negamax(
            &mut pos,
            depth.saturating_sub(1),
            i32::MIN + 1,
            i32::MAX - 1,
        );
        pos.unmake_move(u);
        if score > best_score {
            best_score = score;
            best = Some(m);
        }
    }
    best
}

fn negamax(b: &mut Board, depth: usize, mut alpha: i32, beta: i32) -> i32 {
    if depth == 0 {
        return eval(b);
    }
    let mut moves = Vec::new();
    b.generate_legal_moves(&mut moves);
    if moves.is_empty() {
        // simplistic: stalemate 0, checkmate -mate
        return 0;
    }
    let mut best = i32::MIN;
    for m in moves {
        let u = b.make_move(m);
        let score = -negamax(b, depth - 1, -beta, -alpha);
        b.unmake_move(u);
        if score > best {
            best = score;
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break;
        }
    }
    best
}

// dead-simple material eval
fn eval(b: &Board) -> i32 {
    use crate::types::Piece::*;
    let mut score = 0i32;
    for (sq, p) in b.piece_on.iter().enumerate() {
        score += match p {
            WP => 100,
            WN => 320,
            WB => 330,
            WR => 500,
            WQ => 900,
            WK => 0,
            BP => -100,
            BN => -320,
            BB => -330,
            BR => -500,
            BQ => -900,
            BK => 0,
            _ => 0,
        };
        // tiny piece-square nudge for pawns to make it move forward a bit
        if matches!(p, WP) {
            score += (sq / 8) as i32 * 2;
        }
        if matches!(p, BP) {
            score -= (7 - (sq / 8)) as i32 * 2;
        }
    }
    // side-to-move bonus helps the engine “do something”
    score + 10
}

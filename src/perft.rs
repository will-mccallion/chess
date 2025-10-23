use crate::board::Board;
use crate::types::Move;

fn perft_inner(b: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut moves = Vec::<Move>::with_capacity(128);
    b.generate_legal_moves(&mut moves);
    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0u64;
    for m in moves {
        let u = b.make_move(m);
        nodes += perft_inner(b, depth - 1);
        b.unmake_move(u);
    }
    nodes
}

pub fn perft(b: &mut Board, depth: usize) -> u64 {
    perft_inner(b, depth)
}

pub fn divide(b: &mut Board, depth: usize) {
    let mut moves = Vec::with_capacity(128);
    b.generate_legal_moves(&mut moves);
    let mut total = 0u64;

    for m in moves {
        let u = b.make_move(m);
        let n = perft_inner(b, depth - 1);
        b.unmake_move(u);
        total += n;

        let from_file = (m.from % 8) + b'a' as u8;
        let from_rank = (m.from / 8) + b'1';
        let to_file = (m.to % 8) + b'a' as u8;
        let to_rank = (m.to / 8) + b'1';
        println!(
            "{}{}{}{}: {}",
            from_file as char, from_rank as char, to_file as char, to_rank as char, n
        );
    }
    println!("Total: {total}");
}

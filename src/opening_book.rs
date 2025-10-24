use crate::board::Board;
use phf::phf_map;

static OPENING_BOOK: phf::Map<&'static str, &'static str> = phf_map! {
    // Whites moves
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -" => "e2e4",

    "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6" => "g1f3", // Nf3
    "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 1 3" => "f1b5", // Bb5 (Ruy Lopez)
    "r1bqkbnr/1p1p1ppp/p1n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4" => "b5a4", // Ruy Lopez: Morphy Defense

    "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6" => "g1f3", // Nf3
    "rnbqkbnr/pp2pppp/3p4/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 3" => "d2d4", // Open Sicilian vs 2...d6
    "r1bqkbnr/pp1ppppp/2n5/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 1 3" => "d2d4", // Open Sicilian vs 2...Nc6
    "rnbqkb1r/pp2pppp/3p1n2/2p5/3PP3/5N2/PPP2PPP/RNBQKB1R w KQkq - 1 4" => "b1c3", // Open Sicilian, main line

    //  Responses to 1...e6 (French Defense)
    "rnbqkbnr/pppp1ppp/4p3/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" => "d2d4",
    "rnbqkbnr/pppp1ppp/4p3/8/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2" => "d7d5",

    //  Responses to 1...c6 (Caro-Kann Defense)
    "rnbqkbnr/pp2pppp/2p5/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" => "d2d4",
    "rnbqkbnr/pp2pppp/2p5/3p4/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2" => "d7e4",

    // d4 Openings
    "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1" => "g8f6", // Indian Defense
    "rnbqkb1r/pppppppp/5n2/8/3P4/8/PPP1PPPP/RNBQKBNR w KQkq - 1 2" => "c2c4", // Main line vs Indian
    "rnbqkb1r/pppp1ppp/4pn2/8/2PP4/8/PP2PPPP/RNBQKBNR w KQkq - 0 3" => "b1c3", // vs Nimzo/Bogo
    "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq - 1 2" => "c2c4", // Queen's Gambit
    "rnbqkbnr/ppp2ppp/4p3/3p4/2PP4/8/PP2PPPP/RNBQKBNR w KQkq - 0 3" => "b1c3", // QGD Exchange variation


    // Blacks moves

    //  Defenses to e4
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" => "c7c5", // Sicilian Defense
    "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2" => "b8c6", // vs Nf3
    "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 0 3" => "a7a6", // Ruy Lopez, Morphy Defense
    "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2" => "d7d6", // Sicilian, 2...d6
    "rnbqkb1r/pp2pppp/3p1n2/2p5/3PP3/2N2N2/PPP2PPP/R1BQKB1R b KQkq - 0 4" => "c5d4", // Sicilian, cxd4
    "rnbqkbnr/pp2pppp/2p5/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" => "c6", // Caro-Kann Defense
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1" => "d7d5", // Scandinavian Defense
    "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" => "e4d5",
    "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" => "e7e5", // Open Game
    "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 2 3" => "g8f6", // Italian Game, Two Knights Defense

    //  Defenses to d4
    "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 1" => "d7d5", // Queen's Pawn Game
    "rnbqkbnr/ppp1pppp/8/3p4/2PP4/8/PP2PPPP/RNBQKBNR b KQkq - 0 2" => "c7c6", // Slav Defense
    "rnbqkb1r/pppppppp/5n2/8/2PP4/8/PP2PPPP/RNBQKBNR b KQkq - 0 2" => "e7e6", // Nimzo/Bogo/QGD setup
    "rnbqkb1r/pppp1ppp/4pn2/8/2PP4/2N5/PP2PPPP/RNBQKB1R b KQkq - 1 3" => "f8b4", // Nimzo-Indian Defense
};

fn fen_key(b: &Board) -> String {
    let fen = b.to_fen();
    let mut parts = fen.split_whitespace();
    let placement = parts.next().unwrap_or("");
    let side = parts.next().unwrap_or("");
    let castle = parts.next().unwrap_or("");
    let ep = parts.next().unwrap_or("");
    format!("{placement} {side} {castle} {ep}")
}

pub fn get_book_move(b: &Board) -> Option<&'static str> {
    let key = fen_key(b);
    OPENING_BOOK.get(key.as_str()).copied()
}

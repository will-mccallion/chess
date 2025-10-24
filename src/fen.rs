use crate::board::Board;
use crate::types::{BK_CASTLE, BQ_CASTLE, Color, NO_SQ, Piece, WK_CASTLE, WQ_CASTLE};

pub fn parse_fen(fen: &str) -> Result<Board, String> {
    let mut b = Board::empty();

    let mut parts = fen.split_whitespace();
    let placement = parts.next().ok_or("missing placement")?;
    let side = parts.next().ok_or("missing side")?;
    let castle = parts.next().ok_or("missing castling")?;
    let ep = parts.next().ok_or("missing en-passant")?;
    let halfmove = parts.next().unwrap_or("0");
    let fullmove = parts.next().unwrap_or("1");

    // Placement
    let mut rank = 7;
    let mut file = 0;
    for ch in placement.chars() {
        match ch {
            '/' => {
                rank -= 1;
                file = 0;
            }
            '1'..='8' => {
                file += (ch as u8 - b'0') as i32;
            }
            c if c.is_ascii_alphabetic() => {
                if !(0..8).contains(&file) || !(0..8).contains(&rank) {
                    return Err("bad board in FEN".into());
                }
                let sq = (rank * 8 + file) as usize;
                let piece = Piece::from(c);
                b.place_piece(piece, sq);
                file += 1;
            }
            _ => return Err("bad char in placement".into()),
        }
    }

    // Side
    b.turn = match side {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err("bad side".into()),
    };

    // Castling
    b.castle = 0;
    if castle != "-" {
        for c in castle.chars() {
            match c {
                'K' => b.castle |= WK_CASTLE,
                'Q' => b.castle |= WQ_CASTLE,
                'k' => b.castle |= BK_CASTLE,
                'q' => b.castle |= BQ_CASTLE,
                _ => return Err("bad castling".into()),
            }
        }
    }

    // EP square
    if ep == "-" {
        b.en_passant_sq = NO_SQ;
    } else {
        let bytes = ep.as_bytes();
        if bytes.len() != 2 {
            return Err("bad ep".into());
        }
        let f = (bytes[0] as char).to_ascii_lowercase() as u8 - b'a';
        let r = (bytes[1] as char) as u8 - b'1';
        if f > 7 || r > 7 {
            return Err("bad ep coord".into());
        }
        b.en_passant_sq = (r as i32) * 8 + (f as i32);
    }

    b.halfmove_clock = halfmove.parse().unwrap_or(0);
    b.fullmove_number = fullmove.parse().unwrap_or(1);

    b.rebuild_derived();
    b.recompute_zobrist(); // consistent after rebuild
    b.history.push(b.zobrist); // Initialize history with the starting position hash
    Ok(b)
}

pub fn to_fen(b: &Board) -> String {
    let mut s = String::new();
    for r in (0..8).rev() {
        let mut empty = 0;
        for f in 0..8 {
            let sq = (r * 8 + f) as usize;
            let p = b.piece_on[sq];
            if p.is_empty() {
                empty += 1;
            } else {
                if empty > 0 {
                    s.push(char::from(b'0' + empty as u8));
                    empty = 0;
                }
                s.push(format!("{p}").chars().next().unwrap());
            }
        }
        if empty > 0 {
            s.push(char::from(b'0' + empty as u8));
        }
        if r != 0 {
            s.push('/');
        }
    }
    s.push(' ');
    s.push(if b.turn == Color::White { 'w' } else { 'b' });
    s.push(' ');

    if b.castle == 0 {
        s.push('-');
    } else {
        if b.castle & WK_CASTLE != 0 {
            s.push('K');
        }
        if b.castle & WQ_CASTLE != 0 {
            s.push('Q');
        }
        if b.castle & BK_CASTLE != 0 {
            s.push('k');
        }
        if b.castle & BQ_CASTLE != 0 {
            s.push('q');
        }
    }

    s.push(' ');
    if b.en_passant_sq == NO_SQ {
        s.push('-');
    } else {
        let f = (b.en_passant_sq % 8) as u8 + b'a';
        let r = (b.en_passant_sq / 8) as u8 + b'1';
        s.push(f as char);
        s.push(r as char);
    }
    s.push(' ');
    s.push_str(&b.halfmove_clock.to_string());
    s.push(' ');
    s.push_str(&b.fullmove_number.to_string());
    s
}

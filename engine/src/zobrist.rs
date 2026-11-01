//! Zobrist keys: a 64-bit key per position (the board and the side to move), the exclusive or of
//! one random number per piece on its square and one for Black to move. A move changes few
//! squares, so the key after it (`zobrist_after`) is the key before it with those squares' numbers
//! toggled; it must always equal the key computed from scratch (`zobrist`).
//!
//! A search with progress pressure (the `fs` part of an agent's name) also mixes in one number per
//! count of quiet plies (`quiet_key`), since its values depend on that count; other searches never
//! do, so their keys are unchanged.
//!
//! The random numbers are fixed (splitmix64 from a constant seed, computed at compile time), so a
//! key is the same in every run, and a search that uses them can be repeated.

use crate::board::{Board, Color};
use crate::movegen::Move;
use crate::square::Square;

/// One step of splitmix64: the next state and the number it gives.
const fn splitmix64(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (state, z ^ (z >> 31))
}

/// The numbers of a piece on a square, indexed by `piece_index` and the square's number, and the
/// number of Black to move.
const KEYS: ([[u64; 64]; 4], u64) = {
    let mut keys = [[0; 64]; 4];
    let mut state = 0x7475_6461_6632_3032; // any fixed seed
    let mut piece = 0;
    while piece < 4 {
        let mut sq = 0;
        while sq < 64 {
            let (next, number) = splitmix64(state);
            keys[piece][sq] = number;
            state = next;
            sq += 1;
        }
        piece += 1;
    }
    let (_, black_to_move) = splitmix64(state);
    (keys, black_to_move)
};

/// The numbers of 0 to 50 plies in a row without a capture, from a seed of their own so that
/// `KEYS` stay as they were.
const QUIET_KEYS: [u64; 51] = {
    let mut keys = [0; 51];
    let mut state = 0x7175_6965_7470_6c79; // any fixed seed
    let mut q = 0;
    while q < 51 {
        let (next, number) = splitmix64(state);
        keys[q] = number;
        state = next;
        q += 1;
    }
    keys
};

/// The number of `quiet` plies in a row without a capture (0 to 50), to mix into a key.
pub fn quiet_key(quiet: u32) -> u64 {
    QUIET_KEYS[quiet as usize]
}

/// 0 white man, 1 white king, 2 black man, 3 black king.
const fn piece_index(color: Color, king: bool) -> usize {
    let color = match color {
        Color::White => 0,
        Color::Black => 2,
    };
    color + king as usize
}

fn piece_key(color: Color, king: bool, sq: Square) -> u64 {
    KEYS.0[piece_index(color, king)][sq.index()]
}

/// The key of `board` with `side` to move, from scratch.
pub fn zobrist(board: &Board, side: Color) -> u64 {
    let mut key = match side {
        Color::White => 0,
        Color::Black => KEYS.1,
    };
    for color in [Color::White, Color::Black] {
        for sq in board.pieces(color) {
            key ^= piece_key(color, board.kings().contains(sq), sq);
        }
    }
    key
}

/// The key after `mv` (a legal move on `board`), given `key`, the key of `board` with the mover
/// to move: the piece leaves `from`, the captured pieces leave the board, the piece (crowned if
/// it promotes) lands on `to`, and the other side is to move.
pub fn zobrist_after(key: u64, board: &Board, mv: &Move) -> u64 {
    let (from, to) = (mv.from(), mv.to());
    let color = if board.pieces(Color::White).contains(from) {
        Color::White
    } else {
        Color::Black
    };
    let king = board.kings().contains(from);
    // `from` and `to` may be the same square (a king's capture that ends where it started): the
    // two toggles then cancel, as they should, unless the piece was crowned.
    let mut key = key ^ KEYS.1 ^ piece_key(color, king, from);
    key ^= piece_key(color, king || mv.promotes(), to);
    for sq in mv.captured() {
        key ^= piece_key(color.opponent(), board.kings().contains(sq), sq);
    }
    key
}

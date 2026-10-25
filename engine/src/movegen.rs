//! Legal moves: the port of `moves()` and `captures()` in engine.js.
//!
//! Moves come out in the order engine.js produces them: pieces from square 0 up, the directions
//! of each piece in the JS order, captures depth first. The referees compare sorted lists, but
//! the prototype's agent breaks ties by applying its random numbers to this order, so keeping it
//! lets us replay its seeded games move for move.

use std::fmt::{self, Write as _};

use crate::bitboard::Bitboard;
use crate::board::{Board, Color, Kind};
use crate::square::{Dir, Square};

/// A legal move. A capture lists every square the piece lands on; each captured piece is the
/// enemy between two consecutive landing squares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    /// `from`, then every square the piece lands on; a quiet move has two squares.
    path: Vec<Square>,
    /// The captured pieces. Their order is not kept: the notation is built from `path`.
    captured: Bitboard,
    /// A man that ends the move on its last rank is crowned.
    promotes: bool,
}

impl Move {
    fn quiet(from: Square, to: Square, promotes: bool) -> Move {
        Move {
            path: vec![from, to],
            captured: Bitboard::EMPTY,
            promotes,
        }
    }

    pub fn from(&self) -> Square {
        self.path[0]
    }

    pub fn to(&self) -> Square {
        self.path[self.path.len() - 1]
    }

    /// `from`, then every square the piece lands on.
    pub fn path(&self) -> &[Square] {
        &self.path
    }

    pub fn captured(&self) -> Bitboard {
        self.captured
    }

    pub fn is_capture(&self) -> bool {
        !self.captured.is_empty()
    }

    pub fn promotes(&self) -> bool {
        self.promotes
    }
}

impl fmt::Display for Move {
    /// The notation of docs/reference.md: `d3-d4`; a capture joins every landing square with `x`
    /// (`a6xa2xf2xf5xc5`); a man crowned at the end of the move gets `=K`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sep = if self.is_capture() { 'x' } else { '-' };
        for (i, sq) in self.path.iter().enumerate() {
            if i > 0 {
                f.write_char(sep)?;
            }
            write!(f, "{sq}")?;
        }
        if self.promotes {
            f.write_str("=K")?;
        }
        Ok(())
    }
}

impl Board {
    /// Every legal move for `side`. If any capture exists, only the captures that take the most
    /// pieces; otherwise every quiet move.
    pub fn legal_moves(&self, side: Color) -> Vec<Move> {
        let captures = CaptureSearch::run(self, side);
        if !captures.is_empty() {
            return captures;
        }
        self.quiet_moves(side)
    }

    /// A man steps one square forward or sideways; a king flies to any empty square in a line.
    fn quiet_moves(&self, side: Color) -> Vec<Move> {
        let empty = self.empty_squares();
        let mut moves = Vec::new();
        for from in self.pieces(side) {
            if self.kings().contains(from) {
                for dir in Dir::ALL {
                    for to in from.ray(dir).take_while(|&s| empty.contains(s)) {
                        moves.push(Move::quiet(from, to, false));
                    }
                }
            } else {
                for dir in [side.forward(), Dir::Right, Dir::Left] {
                    let Some(to) = from.step(dir) else { continue };
                    if empty.contains(to) {
                        moves.push(Move::quiet(from, to, to.rank() == side.last_rank()));
                    }
                }
            }
        }
        moves
    }
}

/// Depth-first search over capture sequences: the port of `captures()` in engine.js.
///
/// The JS keeps its working board, path and best list in variables shared by nested closures.
/// A Rust closure cannot call itself, so that state lives in this struct and the search is a set
/// of methods. The board itself is two bitboards passed down by value (copy-make), so nothing
/// has to be undone after a branch.
struct CaptureSearch {
    side: Color,
    /// The opponent's pieces before the move. At a leaf, the captured pieces are the ones that
    /// are missing from `enemy`.
    start_enemy: Bitboard,
    /// `from`, then the landing squares of the current branch.
    path: Vec<Square>,
    /// The most pieces captured by any sequence found so far.
    best: usize,
    /// Every sequence found so far that captures `best` pieces, in the order found.
    found: Vec<Move>,
}

impl CaptureSearch {
    /// Every capture that takes the most pieces, or an empty list if `side` cannot capture.
    fn run(board: &Board, side: Color) -> Vec<Move> {
        let enemy = board.pieces(side.opponent());
        let mut search = CaptureSearch {
            side,
            start_enemy: enemy,
            path: Vec::new(),
            best: 0,
            found: Vec::new(),
        };
        for from in board.pieces(side) {
            // The moving piece leaves its square at once (engine.js: `w[s] = 0`), so a king may
            // cross that square later in the sequence.
            let empty = board.empty_squares().with(from);
            search.path.clear();
            search.path.push(from);
            if board.kings().contains(from) {
                search.king(from, empty, enemy, None);
            } else {
                search.man(from, empty, enemy, None);
            }
        }
        search.found
    }

    /// A man on `sq` jumps an adjacent enemy forward or sideways and lands just beyond it. It
    /// keeps moving as a man even on its last rank: the crowning waits for the end of the move.
    fn man(&mut self, sq: Square, empty: Bitboard, enemy: Bitboard, last: Option<Dir>) {
        let mut any = false;
        for dir in [self.side.forward(), Dir::Right, Dir::Left] {
            if last == Some(dir.opposite()) {
                continue; // no 180-degree turn between two captures
            }
            let Some(victim) = sq.step(dir) else { continue };
            let Some(land) = victim.step(dir) else {
                continue;
            };
            if enemy.contains(victim) && empty.contains(land) {
                // The captured piece leaves the board at once: later jumps may cross its square.
                self.path.push(land);
                self.man(land, empty.with(victim), enemy.without(victim), Some(dir));
                self.path.pop();
                any = true;
            }
        }
        if !any {
            self.leaf(Kind::Man, enemy);
        }
    }

    /// A king on `sq` flies over empty squares to an enemy and may land on any empty square
    /// beyond it.
    fn king(&mut self, sq: Square, empty: Bitboard, enemy: Bitboard, last: Option<Dir>) {
        let mut any = false;
        for dir in Dir::ALL {
            if last == Some(dir.opposite()) {
                continue; // no 180-degree turn between two captures
            }
            let mut ray = sq.ray(dir);
            let Some(victim) = ray.find(|&s| !empty.contains(s)) else {
                continue;
            };
            if !enemy.contains(victim) {
                continue; // an own piece blocks the line
            }
            // `find` stopped at the victim, so the same iterator goes on beyond it.
            for land in ray.take_while(|&s| empty.contains(s)) {
                self.path.push(land);
                self.king(land, empty.with(victim), enemy.without(victim), Some(dir));
                self.path.pop();
                any = true;
            }
        }
        if !any {
            self.leaf(Kind::King, enemy);
        }
    }

    /// The sequence in `self.path` cannot go on. `kind` is the moving piece (it never changes
    /// during a capture); `enemy` holds the opponent's pieces still on the board.
    ///
    /// Records the sequence as a `Move` if it captures at least one piece and no other sequence
    /// found so far captures more; drops every recorded sequence that captures fewer.
    fn leaf(&mut self, kind: Kind, enemy: Bitboard) {
        let captures = self.path.len() - 1;
        if captures == 0 {
            return; // the piece could not capture at all
        }
        if captures > self.best {
            self.best = captures;
            self.found.clear(); // the shorter sequences are no longer legal
        }
        if captures == self.best {
            let to = self.path[self.path.len() - 1];
            self.found.push(Move {
                path: self.path.clone(),
                captured: self.start_enemy & !enemy,
                promotes: kind == Kind::Man && to.rank() == self.side.last_rank(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(file: u8, rank: u8) -> Square {
        Square::new(file, rank)
    }

    #[test]
    fn notation_of_quiet_moves_and_captures() {
        assert_eq!(Move::quiet(sq(3, 2), sq(3, 3), false).to_string(), "d3-d4");
        assert_eq!(Move::quiet(sq(3, 6), sq(3, 7), true).to_string(), "d7-d8=K");

        // The king capture of docs/reference.md: it jumps a4, c2, f3 and d5.
        let king = Move {
            path: vec![sq(0, 5), sq(0, 1), sq(5, 1), sq(5, 4), sq(2, 4)],
            captured: [sq(0, 3), sq(2, 1), sq(5, 2), sq(3, 4)]
                .into_iter()
                .fold(Bitboard::EMPTY, Bitboard::with),
            promotes: false,
        };
        assert_eq!(king.to_string(), "a6xa2xf2xf5xc5");

        let crowned = Move {
            path: vec![sq(0, 3), sq(0, 5), sq(0, 7)],
            captured: Bitboard::from(sq(0, 4)).with(sq(0, 6)),
            promotes: true,
        };
        assert_eq!(crowned.to_string(), "a4xa6xa8=K");
    }
}

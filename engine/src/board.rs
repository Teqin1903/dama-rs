//! Pieces, the board and the board string of docs/reference.md.

use std::fmt::{self, Write as _};
use std::str::FromStr;

use crate::bitboard::Bitboard;
use crate::movegen::Move;
use crate::square::{Dir, Square};

/// White moves up the board (towards rank 8), Black moves down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// The direction this side's men move forward.
    pub(crate) const fn forward(self) -> Dir {
        match self {
            Color::White => Dir::Up,
            Color::Black => Dir::Down,
        }
    }

    /// The rank where this side's men are crowned: 7 (rank 8) for White, 0 (rank 1) for Black.
    pub(crate) const fn last_rank(self) -> u8 {
        match self {
            Color::White => 7,
            Color::Black => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Man,
    King,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub color: Color,
    pub kind: Kind,
}

impl Piece {
    /// The letter of the board string: `w` `W` `b` `B`.
    pub const fn to_char(self) -> char {
        match (self.color, self.kind) {
            (Color::White, Kind::Man) => 'w',
            (Color::White, Kind::King) => 'W',
            (Color::Black, Kind::Man) => 'b',
            (Color::Black, Kind::King) => 'B',
        }
    }

    /// The piece for a letter of the board string, or `None` if `c` is not `w` `W` `b` `B`.
    pub const fn from_char(c: char) -> Option<Piece> {
        let (color, kind) = match c {
            'w' => (Color::White, Kind::Man),
            'W' => (Color::White, Kind::King),
            'b' => (Color::Black, Kind::Man),
            'B' => (Color::Black, Kind::King),
            _ => return None,
        };
        Some(Piece { color, kind })
    }
}

/// The pieces on the board. The side to move is not part of it: as in engine.js, it travels next
/// to the board.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Board {
    white: Bitboard,
    black: Bitboard,
    /// Kings of both colours; always a subset of `white | black`.
    kings: Bitboard,
}

impl Board {
    /// A board without pieces.
    pub const fn empty() -> Board {
        Board {
            white: Bitboard::EMPTY,
            black: Bitboard::EMPTY,
            kings: Bitboard::EMPTY,
        }
    }

    /// The start position: white men on ranks 2 and 3, black men on ranks 6 and 7.
    pub const fn start() -> Board {
        Board {
            white: Bitboard::from_bits(0x0000_0000_00ff_ff00),
            black: Bitboard::from_bits(0x00ff_ff00_0000_0000),
            kings: Bitboard::EMPTY,
        }
    }

    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        let color = if self.white.contains(sq) {
            Color::White
        } else if self.black.contains(sq) {
            Color::Black
        } else {
            return None;
        };
        let kind = if self.kings.contains(sq) {
            Kind::King
        } else {
            Kind::Man
        };
        Some(Piece { color, kind })
    }

    /// The number of pieces of `color` and `kind` on the board.
    pub fn count(&self, color: Color, kind: Kind) -> u32 {
        let pieces = self.pieces(color);
        match kind {
            Kind::Man => (pieces & !self.kings).len(),
            Kind::King => (pieces & self.kings).len(),
        }
    }

    /// Puts `piece` on `sq`, replacing what was there; `None` empties the square.
    pub fn set(&mut self, sq: Square, piece: Option<Piece>) {
        self.white = self.white.without(sq);
        self.black = self.black.without(sq);
        self.kings = self.kings.without(sq);
        if let Some(Piece { color, kind }) = piece {
            match color {
                Color::White => self.white = self.white.with(sq),
                Color::Black => self.black = self.black.with(sq),
            }
            if kind == Kind::King {
                self.kings = self.kings.with(sq);
            }
        }
    }

    /// The board after `mv`, a legal move on this board: the piece leaves `from`, the captured
    /// pieces leave the board and the piece lands on `to`, crowned if the move promotes.
    /// The port of `apply` in engine.js.
    #[must_use]
    pub fn apply(&self, mv: &Move) -> Board {
        let (from, to, captured) = (mv.from(), mv.to(), mv.captured());
        debug_assert!(
            self.white.contains(from) || self.black.contains(from),
            "no piece on {from}"
        );
        let white = self.white.contains(from);
        let king = self.kings.contains(from) || mv.promotes();
        // `from` is cleared before `to` is set: a king's capture may end on its own start square.
        let mut next = Board {
            white: self.white.without(from) & !captured,
            black: self.black.without(from) & !captured,
            kings: self.kings.without(from) & !captured,
        };
        if white {
            next.white = next.white.with(to);
        } else {
            next.black = next.black.with(to);
        }
        if king {
            next.kings = next.kings.with(to);
        }
        next
    }

    /// All pieces of `color`, men and kings.
    pub(crate) const fn pieces(&self, color: Color) -> Bitboard {
        match color {
            Color::White => self.white,
            Color::Black => self.black,
        }
    }

    /// Kings of both colours.
    pub(crate) const fn kings(&self) -> Bitboard {
        self.kings
    }

    pub(crate) fn empty_squares(&self) -> Bitboard {
        !(self.white | self.black)
    }
}

/// Why a board string could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseBoardError {
    /// The string does not have eight ranks separated by `/`.
    RankCount(usize),
    /// A rank (1-8) does not have eight squares.
    RankLength { rank: u8, len: usize },
    /// A square holds a character other than `w` `W` `b` `B` `.`.
    BadSquare { square: Square, found: char },
}

impl fmt::Display for ParseBoardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseBoardError::RankCount(n) => {
                write!(f, "expected 8 ranks separated by '/', found {n}")
            }
            ParseBoardError::RankLength { rank, len } => {
                write!(f, "rank {rank} has {len} squares, expected 8")
            }
            ParseBoardError::BadSquare { square, found } => {
                write!(
                    f,
                    "unexpected {found:?} on {square}, expected one of w W b B ."
                )
            }
        }
    }
}

impl std::error::Error for ParseBoardError {}

impl FromStr for Board {
    type Err = ParseBoardError;

    /// Reads a board string: eight ranks separated by `/`, rank 8 first, files a to h.
    fn from_str(s: &str) -> Result<Board, ParseBoardError> {
        let ranks: Vec<&str> = s.split('/').collect();
        if ranks.len() != 8 {
            return Err(ParseBoardError::RankCount(ranks.len()));
        }
        let mut board = Board::empty();
        for (rank, text) in (0..8).rev().zip(ranks) {
            let len = text.chars().count();
            if len != 8 {
                return Err(ParseBoardError::RankLength {
                    rank: rank + 1,
                    len,
                });
            }
            for (file, found) in (0..8).zip(text.chars()) {
                let square = Square::new(file, rank);
                let piece = match found {
                    '.' => None,
                    c => Some(
                        Piece::from_char(c).ok_or(ParseBoardError::BadSquare { square, found })?,
                    ),
                };
                board.set(square, piece);
            }
        }
        Ok(board)
    }
}

impl fmt::Display for Board {
    /// Writes the board string: eight ranks separated by `/`, rank 8 first, files a to h.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            for file in 0..8 {
                let c = self
                    .piece_at(Square::new(file, rank))
                    .map_or('.', Piece::to_char);
                f.write_char(c)?;
            }
            if rank > 0 {
                f.write_char('/')?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Board {
    /// The board string, so that failed assertions show a readable board.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Board(\"{self}\")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The start position in docs/reference.md.
    const START: &str = "......../bbbbbbbb/bbbbbbbb/......../......../wwwwwwww/wwwwwwww/........";

    #[test]
    fn start_position_matches_the_reference_string() {
        assert_eq!(Board::start().to_string(), START);
        assert_eq!(START.parse::<Board>(), Ok(Board::start()));
    }

    #[test]
    fn every_piece_letter_round_trips() {
        let text = "W......b/......../......../......../......../......../......../w......B";
        let board: Board = text.parse().unwrap();
        let piece = |color, kind| Some(Piece { color, kind });
        assert_eq!(
            board.piece_at(Square::new(0, 7)),
            piece(Color::White, Kind::King)
        );
        assert_eq!(
            board.piece_at(Square::new(7, 7)),
            piece(Color::Black, Kind::Man)
        );
        assert_eq!(
            board.piece_at(Square::new(0, 0)),
            piece(Color::White, Kind::Man)
        );
        assert_eq!(
            board.piece_at(Square::new(7, 0)),
            piece(Color::Black, Kind::King)
        );
        assert_eq!(board.piece_at(Square::new(3, 3)), None);
        assert_eq!(board.to_string(), text);
    }

    #[test]
    fn set_replaces_and_clears() {
        let d4 = Square::new(3, 3);
        let black_man = Some(Piece {
            color: Color::Black,
            kind: Kind::Man,
        });
        let mut board = Board::empty();
        board.set(
            d4,
            Some(Piece {
                color: Color::White,
                kind: Kind::King,
            }),
        );
        board.set(d4, black_man);
        assert_eq!(board.piece_at(d4), black_man);
        board.set(d4, None);
        assert_eq!(board, Board::empty());
    }

    #[test]
    fn parse_errors_say_what_is_wrong() {
        assert_eq!(
            "......../........".parse::<Board>(),
            Err(ParseBoardError::RankCount(2))
        );
        assert_eq!(
            "......../......../......../......../......../......../......../........."
                .parse::<Board>(),
            Err(ParseBoardError::RankLength { rank: 1, len: 9 })
        );
        assert_eq!(
            "......../......../......../..x...../......../......../......../........"
                .parse::<Board>(),
            Err(ParseBoardError::BadSquare {
                square: Square::new(2, 4),
                found: 'x'
            })
        );
    }

    /// The board after the legal move of `side` written `notation`.
    fn after(board: &str, side: Color, notation: &str) -> Board {
        let board: Board = board.parse().unwrap();
        let mv = board
            .legal_moves(side)
            .into_iter()
            .find(|mv| mv.to_string() == notation)
            .unwrap_or_else(|| panic!("{notation} is not a legal move"));
        board.apply(&mv)
    }

    #[test]
    fn apply_moves_captures_and_crowns() {
        assert_eq!(
            after(START, Color::White, "d3-d4").to_string(),
            "......../bbbbbbbb/bbbbbbbb/......../...w..../www.wwww/wwwwwwww/........"
        );
        // man_continues_as_man in testdata/rules.json: crowned only at the end of the move.
        assert_eq!(
            after(
                "..b...../...b..../...w..../......../......../......../......../........",
                Color::White,
                "d6xd8xb8=K"
            )
            .to_string(),
            ".W....../......../......../......../......../......../......../........"
        );
    }

    #[test]
    fn apply_a_king_capture_that_ends_on_its_start_square() {
        // The king takes f4, h6, b8, a6 and b4, and lands on d4 again.
        let board = after(
            ".b....../......../b......b/......../.b.W.b../......../......../........",
            Color::White,
            "d4xh4xh8xa8xa4xd4",
        );
        assert_eq!(
            board.to_string(),
            "......../......../......../......../...W..../......../......../........"
        );
    }

    #[test]
    fn count_by_colour_and_kind() {
        let start = Board::start();
        assert_eq!(start.count(Color::White, Kind::Man), 16);
        assert_eq!(start.count(Color::Black, Kind::Man), 16);
        assert_eq!(start.count(Color::White, Kind::King), 0);
        let board: Board =
            "W......b/......../......../......../......../......../......../w......B"
                .parse()
                .unwrap();
        for color in [Color::White, Color::Black] {
            for kind in [Kind::Man, Kind::King] {
                assert_eq!(board.count(color, kind), 1, "{color:?} {kind:?}");
            }
        }
    }
}

//! Squares and directions. Squares are numbered rank * 8 + file, as in engine.js.

use std::fmt;

/// One of the 64 squares, numbered rank * 8 + file: a1 = 0, h1 = 7, a8 = 56, h8 = 63.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

impl Square {
    /// The square on `file` (0 = a .. 7 = h) and `rank` (0 = rank 1 .. 7 = rank 8).
    pub const fn new(file: u8, rank: u8) -> Square {
        debug_assert!(file < 8 && rank < 8);
        Square(rank * 8 + file)
    }

    /// The square numbered `index` (0..64).
    pub(crate) const fn from_index(index: u8) -> Square {
        debug_assert!(index < 64);
        Square(index)
    }

    /// 0 = file a .. 7 = file h.
    pub const fn file(self) -> u8 {
        self.0 % 8
    }

    /// 0 = rank 1 .. 7 = rank 8.
    pub const fn rank(self) -> u8 {
        self.0 / 8
    }

    /// The square's number, rank * 8 + file.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The neighbouring square in `dir`, or `None` at the edge of the board.
    pub(crate) fn step(self, dir: Dir) -> Option<Square> {
        let (df, dr) = dir.delta();
        let file = self.file() as i8 + df;
        let rank = self.rank() as i8 + dr;
        if (0..8).contains(&file) && (0..8).contains(&rank) {
            Some(Square::new(file as u8, rank as u8))
        } else {
            None
        }
    }

    /// The squares beyond `self` in `dir`, nearest first, up to the edge of the board.
    pub(crate) fn ray(self, dir: Dir) -> impl Iterator<Item = Square> {
        std::iter::successors(self.step(dir), move |sq| sq.step(dir))
    }
}

impl fmt::Display for Square {
    /// File letter, then rank number: `d4`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", char::from(b'a' + self.file()), self.rank() + 1)
    }
}

/// The four orthogonal directions. Up is towards rank 8, right is towards file h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dir {
    Up,
    Down,
    Right,
    Left,
}

impl Dir {
    /// All four, in the order of `DIRS` in engine.js.
    pub(crate) const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Right, Dir::Left];

    /// The direction after a 180-degree turn.
    pub(crate) const fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Right => Dir::Left,
            Dir::Left => Dir::Right,
        }
    }

    /// (file, rank) change of one step.
    const fn delta(self) -> (i8, i8) {
        match self {
            Dir::Up => (0, 1),
            Dir::Down => (0, -1),
            Dir::Right => (1, 0),
            Dir::Left => (-1, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_numbers_match_engine_js() {
        let cases = [
            (0, 0, "a1", 0),
            (7, 0, "h1", 7),
            (0, 7, "a8", 56),
            (7, 7, "h8", 63),
            (3, 3, "d4", 27),
        ];
        for (file, rank, name, index) in cases {
            let sq = Square::new(file, rank);
            assert_eq!(sq.to_string(), name);
            assert_eq!(sq.index(), index);
            assert_eq!((sq.file(), sq.rank()), (file, rank));
        }
    }

    #[test]
    fn step_stops_at_the_edges() {
        let a1 = Square::new(0, 0);
        let h8 = Square::new(7, 7);
        assert_eq!(a1.step(Dir::Down), None);
        assert_eq!(a1.step(Dir::Left), None);
        assert_eq!(a1.step(Dir::Up), Some(Square::new(0, 1)));
        assert_eq!(a1.step(Dir::Right), Some(Square::new(1, 0)));
        assert_eq!(h8.step(Dir::Up), None);
        assert_eq!(h8.step(Dir::Right), None);
    }

    #[test]
    fn ray_runs_to_the_edge_nearest_first() {
        let d4 = Square::new(3, 3);
        let up: Vec<String> = d4.ray(Dir::Up).map(|sq| sq.to_string()).collect();
        assert_eq!(up, ["d5", "d6", "d7", "d8"]);
        assert_eq!(d4.ray(Dir::Left).count(), 3);
        assert_eq!(Square::new(0, 3).ray(Dir::Left).count(), 0);
    }
}

//! Sets of squares as 64-bit masks.

use std::ops::{BitAnd, BitOr, Not};

use crate::square::Square;

/// A set of squares: bit i is square i (a1 = bit 0, h8 = bit 63), the numbering of engine.js.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bitboard(u64);

/// The mask with only `sq` set.
const fn bit(sq: Square) -> u64 {
    1 << sq.index()
}

impl Bitboard {
    /// The empty set.
    pub const EMPTY: Bitboard = Bitboard(0);

    pub const fn from_bits(bits: u64) -> Bitboard {
        Bitboard(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, sq: Square) -> bool {
        self.0 & bit(sq) != 0
    }

    /// A copy with `sq` added.
    #[must_use]
    pub const fn with(self, sq: Square) -> Bitboard {
        Bitboard(self.0 | bit(sq))
    }

    /// A copy with `sq` removed.
    #[must_use]
    pub const fn without(self, sq: Square) -> Bitboard {
        Bitboard(self.0 & !bit(sq))
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The number of squares in the set.
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }
}

impl From<Square> for Bitboard {
    fn from(sq: Square) -> Bitboard {
        Bitboard(bit(sq))
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;

    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;

    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl Not for Bitboard {
    type Output = Bitboard;

    /// Every square of the board that is not in the set.
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

/// Squares of the set from the lowest number up (a1, b1, .., h8): the order engine.js visits them.
impl Iterator for Bitboard {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        let sq = Square::from_index(self.0.trailing_zeros() as u8);
        self.0 &= self.0 - 1; // clear the lowest set bit
        Some(sq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_from_the_lowest_square_up() {
        let set = Bitboard::EMPTY
            .with(Square::new(7, 7))
            .with(Square::new(3, 3))
            .with(Square::new(0, 0));
        let names: Vec<String> = set.map(|sq| sq.to_string()).collect();
        assert_eq!(names, ["a1", "d4", "h8"]);
    }

    #[test]
    fn with_without_contains_len() {
        let d4 = Square::new(3, 3);
        let e4 = Square::new(4, 3);
        let set = Bitboard::EMPTY.with(d4).with(e4);
        assert!(set.contains(d4) && set.contains(e4));
        assert_eq!(set.len(), 2);

        let set = set.without(d4);
        assert!(!set.contains(d4) && set.contains(e4));
        assert_eq!(set, Bitboard::from(e4));
        assert!(set.without(e4).is_empty());
        assert_eq!((!Bitboard::EMPTY).len(), 64);
    }
}

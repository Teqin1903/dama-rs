//! The transposition table: what a search learned about a position (its Zobrist key), so that the
//! same position reached by another move order, a later iteration or a later move of the game is
//! not searched again, and its best move is tried first when it is.
//!
//! One entry per slot, the slot chosen by the key's low bits; a new entry always replaces the old
//! one. An agent's table lives as long as the agent and is cleared at the start of every game, so
//! a game does not depend on the games played before it (in the arena, a thread's earlier games).

use std::fmt;

/// What an entry's value says about the position's value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Bound {
    /// An empty slot.
    #[default]
    Empty,
    /// No value, only the best move (the value depended on the search path).
    Move,
    Exact,
    /// The value is at least this.
    Lower,
    /// The value is at most this.
    Upper,
}

/// One position's entry.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Entry {
    pub key: u64,
    /// From the view of the side to move, a win's distance counted from this position.
    pub value: f64,
    /// The plies searched below this position before the depth limit (0 in quiescence).
    pub draft: u8,
    pub bound: Bound,
    /// The index of the best move in the position's legal moves.
    pub best: u16,
    /// Whether the search below reached a leaf at the depth limit (see `iterative_deepening`).
    pub horizon: bool,
}

/// How big the table is, and whether the search may use only what it would have found itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableOptions {
    /// The size in megabytes (2^20 bytes); the table has the largest power of two of entries that
    /// fits.
    pub mb: u32,
    /// The referee's mode: use an entry's value only at the same draft (not from a deeper search),
    /// and, with repetitions, only in a position whose value cannot depend on the path (see
    /// `search`). A search with a strict table makes the decision of the same search without one.
    pub strict: bool,
}

impl TableOptions {
    /// The size when the name gives none.
    pub const DEFAULT_MB: u32 = 16;
    /// The largest size a name may ask for.
    pub const MAX_MB: u32 = 65536;
}

impl Default for TableOptions {
    fn default() -> TableOptions {
        TableOptions {
            mb: TableOptions::DEFAULT_MB,
            strict: false,
        }
    }
}

impl fmt::Display for TableOptions {
    /// `16 MB`, `16 MB, strict`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} MB", self.mb)?;
        if self.strict {
            f.write_str(", strict")?;
        }
        Ok(())
    }
}

/// A transposition table.
#[derive(Clone, Debug)]
pub struct Table {
    entries: Vec<Entry>,
    strict: bool,
}

impl Table {
    pub fn new(options: TableOptions) -> Table {
        let bytes = options.mb as usize * (1 << 20);
        let fits = (bytes / size_of::<Entry>()).max(1);
        // The largest power of two that is not more than `fits`.
        let len = 1 << (usize::BITS - 1 - fits.leading_zeros());
        Table {
            entries: vec![Entry::default(); len],
            strict: options.strict,
        }
    }

    /// Empties every slot.
    pub fn clear(&mut self) {
        self.entries.fill(Entry::default());
    }

    /// The number of slots.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn strict(&self) -> bool {
        self.strict
    }

    fn slot(&self, key: u64) -> usize {
        // `len` is a power of two, so the mask keeps the key's low bits.
        key as usize & (self.entries.len() - 1)
    }

    /// The entry of the position with `key`, if its slot holds it.
    pub(crate) fn probe(&self, key: u64) -> Option<Entry> {
        let entry = self.entries[self.slot(key)];
        (entry.bound != Bound::Empty && entry.key == key).then_some(entry)
    }

    /// Puts `entry` in its slot, in place of what was there.
    pub(crate) fn store(&mut self, entry: Entry) {
        let slot = self.slot(entry.key);
        self.entries[slot] = entry;
    }

    /// The number of slots that hold an entry.
    pub fn used(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.bound != Bound::Empty)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_is_a_power_of_two_that_fits() {
        assert_eq!(size_of::<Entry>(), 24);
        let table = Table::new(TableOptions {
            mb: 1,
            strict: false,
        });
        // 2^20 / 24 = 43690.67, so 32768 entries.
        assert_eq!(table.len(), 32768);
        let table = Table::new(TableOptions::default());
        assert_eq!(table.len(), 1 << 19);
    }

    #[test]
    fn an_entry_is_found_by_its_key_and_replaced_in_its_slot() {
        let mut table = Table::new(TableOptions {
            mb: 1,
            strict: false,
        });
        let entry = |key, value| Entry {
            key,
            value,
            draft: 3,
            bound: Bound::Exact,
            best: 2,
            horizon: true,
        };
        assert!(table.probe(5).is_none());
        table.store(entry(5, 0.5));
        assert_eq!(table.probe(5).unwrap().value, 0.5);
        // The same slot, another key: the new entry replaces the old one.
        let other = 5 + table.len() as u64;
        table.store(entry(other, 1.5));
        assert!(table.probe(5).is_none());
        assert_eq!(table.probe(other).unwrap().value, 1.5);
        assert_eq!(table.used(), 1);
        table.clear();
        assert!(table.probe(other).is_none());
        assert_eq!(table.used(), 0);
    }
}

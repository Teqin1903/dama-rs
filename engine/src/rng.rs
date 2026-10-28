//! mulberry32, the seeded random number generator of reference/js/rng.js, ported bit for bit: the
//! same seed gives the same sequence in Rust and in JS.

/// A mulberry32 generator. It is deliberately not `Copy`: a silent copy of a generator would
/// repeat its numbers.
#[derive(Clone, Debug)]
pub struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// The state `mulberry32(seed)` starts from in JS. JS keeps only 32 bits of the seed
    /// (`a |= 0`), so seed -1 there is `u32::MAX` here.
    pub const fn new(seed: u32) -> Mulberry32 {
        Mulberry32 { state: seed }
    }

    /// The next 32-bit output. JS's int32 values carry the same 32 bits as these `u32`s:
    /// `Math.imul` keeps the low 32 bits of a product like `wrapping_mul`, and `>>>` is `>>` on a
    /// `u32`.
    pub fn next_u32(&mut self) -> u32 {
        // a = (a + 0x6D2B79F5) | 0
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let a = self.state;
        // let t = Math.imul(a ^ (a >>> 15), 1 | a)
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        // t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; the ^ brings the sum back to 32 bits
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        // (t ^ (t >>> 14)) >>> 0
        t ^ (t >> 14)
    }

    /// The next number in [0, 1): exactly the double `rnd()` returns in reference/js/rng.js,
    /// because dividing by 2^32 is exact.
    pub fn next_f64(&mut self) -> f64 {
        f64::from(self.next_u32()) / 4_294_967_296.0
    }

    /// An index below `len`, drawn as agent.js draws one: `Math.floor(rnd() * len)`. It draws a
    /// number even when `len` is 1. `next_f64` is below 1, so the index is below `len`.
    pub fn next_index(&mut self, len: usize) -> usize {
        (self.next_f64() * len as f64).floor() as usize
    }
}

/// The finaliser of MurmurHash3 (`fmix32`): a bijection on 32 bits in which every input bit
/// changes about half of the output bits.
const fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

/// A seed for item `index` of `stream`, made from `seed` alone: the arena gives game g its own
/// seeds this way, so that no game depends on the games before it. Different streams (openings,
/// agent A, agent B) keep the seeds of one item apart.
pub const fn derive_seed(seed: u32, stream: u32, index: u32) -> u32 {
    let h = fmix32(seed ^ stream.wrapping_mul(0x9E37_79B9));
    fmix32(h ^ fmix32(index.wrapping_add(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmix32_known_values() {
        // MurmurHash3's finaliser maps 0 to 0; the others are from an independent JS version
        // (Math.imul) of the same five lines.
        assert_eq!(fmix32(0), 0);
        assert_eq!(fmix32(1), 1364076727);
        assert_eq!(derive_seed(1, 1, 0), 2896955211);
        assert_eq!(derive_seed(1, 2, 0), 3020345282);
        assert_eq!(derive_seed(1, 1, 1), 3643503889);
        assert_eq!(derive_seed(u32::MAX, 3, 199), 28924130);
    }

    #[test]
    fn derived_seeds_differ_across_streams_and_indices() {
        let mut seen = std::collections::HashSet::new();
        for stream in 1..=3 {
            for index in 0..1000 {
                assert!(seen.insert(derive_seed(1, stream, index)));
            }
        }
        assert_ne!(derive_seed(1, 1, 0), derive_seed(2, 1, 0));
    }
}

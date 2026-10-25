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

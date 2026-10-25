//! mulberry32 (reference/js/rng.js) ported bit for bit: the same seed gives the same sequence.
//!
//! The expected values come from the prototype itself, from the repository root:
//!
//!     node -e "const mk = require('./reference/js/rng.js');
//!       for (const seed of [0, 1, 2026, 7070, 4294967295]) {
//!         const r = mk(seed), f = [r(), r(), r(), r()];
//!         console.log(seed, f.map((x) => x * 4294967296).join(', '), f.join(', ')); }
//!       const r = mk(2026); let xor = 0, sum = 0n;
//!       for (let i = 0; i < 1e6; i++) {
//!         const x = r() * 4294967296; xor = (xor ^ x) >>> 0; sum += BigInt(x); }
//!       console.log(xor, sum);"

use engine::Mulberry32;

/// (seed, the first four outputs as integers, the same four as `rnd()` returns them)
const RNG_JS: [(u32, [u32; 4], [f64; 4]); 5] = [
    (
        0,
        [1144304738, 1416247, 958946056, 627933444],
        [
            0.26642920868471265,
            0.0003297457005828619,
            0.2232720274478197,
            0.1462021479383111,
        ],
    ),
    (
        1,
        [2693262067, 11749833, 2265367787, 4213581821],
        [
            0.6270739405881613,
            0.002735721180215478,
            0.5274470399599522,
            0.9810509674716741,
        ],
    ),
    (
        2026,
        [1955961175, 1324980858, 2839649622, 2656330836],
        [
            0.45540769933722913,
            0.30849614599719644,
            0.6611574492417276,
            0.6184752183035016,
        ],
    ),
    (
        7070,
        [3668372421, 900722728, 1862683850, 2589293025],
        [
            0.854109511943534,
            0.20971585251390934,
            0.43368987971916795,
            0.6028667616192251,
        ],
    ),
    // seed 4294967295 in JS, which `a |= 0` turns into -1: the same 32 bits
    (
        u32::MAX,
        [3850105811, 813802916, 3073704848, 4054706436],
        [
            0.8964226141106337,
            0.189478256739676,
            0.7156526781618595,
            0.9440599093213677,
        ],
    ),
];

#[test]
fn same_sequence_as_rng_js() {
    for (seed, ints, floats) in RNG_JS {
        let mut as_ints = Mulberry32::new(seed);
        let mut as_floats = Mulberry32::new(seed);
        for (i, (int, float)) in ints.into_iter().zip(floats).enumerate() {
            assert_eq!(as_ints.next_u32(), int, "seed {seed}, number {i}");
            assert_eq!(as_floats.next_f64(), float, "seed {seed}, number {i}");
        }
    }
}

#[test]
fn a_million_numbers_match_rng_js() {
    let mut rng = Mulberry32::new(2026);
    let (mut xor, mut sum) = (0u32, 0u64);
    for _ in 0..1_000_000 {
        let x = rng.next_u32();
        xor ^= x;
        sum += u64::from(x);
    }
    assert_eq!((xor, sum), (3_500_093_726, 2_146_967_370_412_502));
}

/// `Math.floor(rnd() * len)` in JS, from the repository root:
///
///     node -e "const r = require('./reference/js/rng.js')(2026);
///       console.log([3, 10, 7, 1, 64, 2, 1000].map((n) => Math.floor(r() * n)).join(', '));"
#[test]
fn next_index_is_math_floor_of_rnd_times_len() {
    let mut rng = Mulberry32::new(2026);
    let picks: Vec<usize> = [3, 10, 7, 1, 64, 2, 1000]
        .into_iter()
        .map(|len| rng.next_index(len))
        .collect();
    assert_eq!(picks, [1, 3, 4, 0, 9, 0, 682]);
}

#[test]
fn same_seed_same_sequence() {
    let draw = |rng: &mut Mulberry32| -> Vec<u32> { (0..1000).map(|_| rng.next_u32()).collect() };
    let mut a = Mulberry32::new(7070);
    let first = draw(&mut a);
    assert_eq!(first, draw(&mut Mulberry32::new(7070)));
    assert_ne!(first, draw(&mut Mulberry32::new(7071)));

    // A clone goes on from the same state: copying a generator repeats its numbers.
    let mut b = a.clone();
    assert_eq!(draw(&mut a), draw(&mut b));
}

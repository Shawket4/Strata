//! A small seedable PRNG (`xoshiro256**` seeded through `SplitMix64`).
//!
//! Implemented here rather than taken from `rand` so the sequence for a seed is fixed forever
//! and identical on every platform: cluster assignments and layouts must not change when a
//! dependency is upgraded.

/// Deterministic random number generator.
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Rng {
    /// A generator for `seed`.
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        Self {
            s: [
                splitmix(&mut x),
                splitmix(&mut x),
                splitmix(&mut x),
                splitmix(&mut x),
            ],
        }
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        // 53 random bits → exactly representable.
        #[expect(clippy::cast_precision_loss, reason = "53-bit values are exact in f64")]
        let v = (self.next_u64() >> 11) as f64;
        v * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Uniform in `0..n` (`n > 0`), without modulo bias worth caring about for graph sizes.
    pub fn below(&mut self, n: usize) -> usize {
        let n64 = n as u64;
        usize::try_from(self.next_u64() % n64.max(1)).unwrap_or(0)
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_is_fixed() {
        let mut r = Rng::new(42);
        let got: Vec<u64> = (0..3).map(|_| r.next_u64()).collect();
        let mut again = Rng::new(42);
        assert_eq!(got, (0..3).map(|_| again.next_u64()).collect::<Vec<_>>());
        assert_ne!(Rng::new(43).next_u64(), got[0]);
        for _ in 0..1000 {
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
            assert!(r.below(7) < 7);
        }
    }
}

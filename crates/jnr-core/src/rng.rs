//! The simulation's only random number generator (xorshift64*).

/// A seeded xorshift64* generator. Part of the game state (cloned and hashed
/// with it), so randomness replays exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Creates a generator. The seed is mixed (splitmix64) so that small
    /// seeds like 1, 2, 3 give unrelated sequences; the state is never 0.
    pub fn new(seed: u64) -> Rng {
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        Rng {
            state: if z == 0 { 0x2545_f491_4f6c_dd1d } else { z },
        }
    }

    /// The raw state (for hashing).
    pub fn state(&self) -> u64 {
        self.state
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A value in `0..n` (returns 0 for `n == 0`).
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (((self.next_u64() >> 32) * n as u64) >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_seed_dependent() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(1);
        let mut c = Rng::new(2);
        let sa: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let sb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        let sc: Vec<u64> = (0..8).map(|_| c.next_u64()).collect();
        assert_eq!(sa, sb);
        assert_ne!(sa, sc);
        assert_ne!(Rng::new(0).state(), 0);
    }

    #[test]
    fn below_stays_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            assert!(r.below(6) < 6);
        }
        assert_eq!(r.below(0), 0);
    }
}

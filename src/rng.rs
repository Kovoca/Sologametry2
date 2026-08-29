//! Deterministic pseudo-random number generator.
//!
//! Owned rather than pulled from the `rand` crate on purpose: world
//! generation must rebuild the exact same planet from a seed forever, and
//! `rand`'s generators are explicitly allowed to change their output between
//! versions. Owning the algorithm means a seed is a permanent address.

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // SplitMix64 mix so that a small or low-entropy seed (0, 1, 2...)
        // still produces a well-scrambled starting state.
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        // `| 1` guarantees we never hand xorshift the all-zero state it
        // cannot escape.
        Self { state: z | 1 }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform in `[0, 1)`. Takes the top 24 bits, which is exactly the f32
    /// mantissa width, so the result is unbiased.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

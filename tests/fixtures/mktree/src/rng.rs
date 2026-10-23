//! A seeded `SplitMix64` generator. Integer-only, so every platform draws the
//! same sequence and the expanded manifest (and its hash) is the same
//! everywhere.

/// `SplitMix64` (Steele, Lea and Flood): small, fast and good enough to shape
/// a directory tree.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator seeded with `seed`.
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n` (Lemire's multiply-shift; `n` must be non-zero).
    pub fn below(&mut self, n: usize) -> usize {
        let wide = u128::from(self.next_u64()) * n as u128;
        usize::try_from(wide >> 64).unwrap_or(0)
    }

    /// `scale · u^skew` for `u` uniform in `[0, 1)`: with `skew` above 1 most
    /// draws are small and a few approach `scale`.
    pub fn skewed(&mut self, scale: u64, skew: u32) -> u64 {
        let mut x = u128::from(scale);
        for _ in 0..skew {
            x = (x * u128::from(self.next_u64() >> 32)) >> 32;
        }
        u64::try_from(x).unwrap_or(scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_test_01_rng_sequence_is_fixed() {
        // The reference SplitMix64 outputs for seed 0; a change here changes
        // every generated subtree and therefore the manifest hash.
        let mut rng = Rng::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn p0_test_01_rng_ranges_hold() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            assert!(rng.below(13) < 13);
            assert!(rng.skewed(4096, 3) < 4096);
        }
        assert_eq!(rng.skewed(4096, 0), 4096);
    }
}

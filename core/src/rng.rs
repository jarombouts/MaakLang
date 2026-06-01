//! The one source of nondeterminism: a small seeded PRNG owned by the core (LANGUAGE.md §5,
//! ARCHITECTURE.md §10). Never the host's RNG — `reset(seed)` makes replay reproducible.

/// xorshift64* — tiny, fast, deterministic, good enough for a turtle.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

pub const DEFAULT_SEED: u64 = 0x2545_F491_4F6C_DD1D;

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: if seed == 0 { DEFAULT_SEED } else { seed } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform integer in `[lo, hi]` inclusive.
    pub fn range_inclusive(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }

    /// Uniform choice from a non-empty slice.
    pub fn choice<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let i = (self.next_u64() as usize) % items.len();
        &items[i]
    }
}

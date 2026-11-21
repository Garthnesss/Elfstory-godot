//! Deterministic mulberry32 PRNG matching `elfstoryreborn/src/game/rng.ts`.

#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Current internal state (for saves: restoring it continues the
    /// exact same sequence).
    pub fn state(&self) -> u32 {
        self.state
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f64 / 4_294_967_296.0) as f32
    }

    pub fn range_usize(&mut self, max_exclusive: usize) -> usize {
        if max_exclusive <= 1 {
            return 0;
        }
        (self.next_u32() as usize) % max_exclusive
    }
}

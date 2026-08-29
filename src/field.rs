//! A flat 2D scalar grid.
//!
//! Stored as one `Vec<f32>` indexed `y * width + x`, never `Vec<Vec<f32>>`.
//! Nested vectors mean a pointer chase per row and wreck cache locality --
//! and cache locality is what actually decides whether the late-game
//! simulation runs at a playable speed.

#[derive(Clone)]
pub struct Field {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Field {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        self.data[y * self.width + x] = v;
    }

    /// Rescale in place so the values span exactly `0.0..=1.0`.
    pub fn normalise(&mut self) {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &v in &self.data {
            min = min.min(v);
            max = max.max(v);
        }
        let range = (max - min).max(1e-12);
        for v in &mut self.data {
            *v = (*v - min) / range;
        }
    }

    /// Value at quantile `q` (`0.0` = minimum, `1.0` = maximum).
    ///
    /// Deterministic: a total order via `f32::total_cmp` and a plain
    /// nearest-rank pick, so the same field always resolves the same
    /// threshold on every platform.
    pub fn quantile(&self, q: f32) -> f32 {
        let mut sorted = self.data.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let i = (((sorted.len() - 1) as f32) * q.clamp(0.0, 1.0)).round() as usize;
        sorted[i]
    }
}

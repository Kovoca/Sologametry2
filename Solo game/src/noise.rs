//! Value-noise fractal Brownian motion and a ridged multifractal.
//!
//! The X axis wraps. The planet is a cylinder, so noise sampled at the last
//! column must meet the first column with no visible seam -- the coarse
//! lattice is sampled `% grid_width` to make that true. Y clamps instead:
//! the north and south poles are not adjacent.

use crate::field::Field;
use crate::rng::Rng;

#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// One octave of value noise: a coarse random lattice, bilinearly upsampled
/// to the full grid with a smoothstep on the interpolation weights.
fn value_noise(width: usize, height: usize, freq: f32, rng: &mut Rng) -> Field {
    let gw = (freq.round() as usize).max(2);
    let gh = ((freq * height as f32 / width as f32).round() as usize).max(2);

    let mut lattice = vec![0.0f32; gw * gh];
    for v in &mut lattice {
        *v = rng.next_f32();
    }

    let mut out = Field::new(width, height);
    for y in 0..height {
        let fy = y as f32 / (height.max(2) - 1) as f32 * (gh - 1) as f32;
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(gh - 1);
        let ty = smoothstep(fy - y0 as f32);

        for x in 0..width {
            let fx = x as f32 / width as f32 * gw as f32;
            let x0 = (fx.floor() as usize) % gw;
            let x1 = (x0 + 1) % gw;
            let tx = smoothstep(fx - fx.floor());

            let top = lattice[y0 * gw + x0] * (1.0 - tx) + lattice[y0 * gw + x1] * tx;
            let bot = lattice[y1 * gw + x0] * (1.0 - tx) + lattice[y1 * gw + x1] * tx;
            out.data[y * width + x] = top * (1.0 - ty) + bot * ty;
        }
    }
    out
}

/// Fractal Brownian motion: summed octaves of value noise, each octave twice
/// the frequency and half the amplitude of the one before.
pub fn fbm(width: usize, height: usize, octaves: u32, base_freq: f32, rng: &mut Rng) -> Field {
    let mut total = Field::new(width, height);
    let (mut amp, mut freq, mut norm) = (1.0f32, base_freq, 0.0f32);

    for _ in 0..octaves {
        let layer = value_noise(width, height, freq, rng);
        for (t, l) in total.data.iter_mut().zip(&layer.data) {
            *t += amp * l;
        }
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    for v in &mut total.data {
        *v /= norm;
    }
    total
}

/// Ridged multifractal: each octave is folded into sharp crests before it is
/// summed, which produces linear mountain ranges and fault lines rather than
/// the rounded blobs plain fBm gives.
pub fn ridged(width: usize, height: usize, octaves: u32, base_freq: f32, rng: &mut Rng) -> Field {
    let mut total = Field::new(width, height);
    let (mut amp, mut freq, mut norm) = (1.0f32, base_freq, 0.0f32);

    for _ in 0..octaves {
        let layer = value_noise(width, height, freq, rng);
        for (t, &l) in total.data.iter_mut().zip(&layer.data) {
            let n = 1.0 - (l * 2.0 - 1.0).abs();
            *t += amp * n * n;
        }
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    for v in &mut total.data {
        *v /= norm;
    }
    total
}

//! Fixed-point turtle math. No floating-point trig anywhere — movement uses the build-time
//! `SIN_Q12` table (Q12 = ×4096) with quadrant symmetry, so the exact pixels drawn are
//! identical on every target (the determinism rule, ARCHITECTURE.md §10).
//!
//! Positions are stored in Q8 fixed-point (×256) so repeated sub-pixel moves accumulate
//! correctly; only integer pixels are ever emitted to the framebuffer.

use crate::vocab::SIN_Q12;

pub const FRAC_BITS: i32 = 8;
pub const ONE: i64 = 1 << FRAC_BITS; // 256 == 1.0 px

/// sin(deg) in Q12 (×4096), exact integer, for any integer degree.
fn sin_q12(deg: i64) -> i64 {
    let d = ((deg % 360) + 360) % 360; // 0..=359
    let v = match d {
        0..=90 => SIN_Q12[d as usize] as i64,
        91..=180 => SIN_Q12[(180 - d) as usize] as i64,
        181..=270 => -(SIN_Q12[(d - 180) as usize] as i64),
        _ => -(SIN_Q12[(360 - d) as usize] as i64),
    };
    v
}

fn cos_q12(deg: i64) -> i64 {
    sin_q12(deg + 90)
}

/// A turtle position in Q8 fixed-point.
#[derive(Debug, Clone, Copy)]
pub struct FixedPos {
    pub x: i64,
    pub y: i64,
}

impl FixedPos {
    pub fn from_px(x: i64, y: i64) -> Self {
        FixedPos { x: x << FRAC_BITS, y: y << FRAC_BITS }
    }
    pub fn px_x(&self) -> i64 {
        round_to_px(self.x)
    }
    pub fn px_y(&self) -> i64 {
        round_to_px(self.y)
    }
}

fn round_to_px(q8: i64) -> i64 {
    // round half away from zero
    if q8 >= 0 {
        (q8 + ONE / 2) >> FRAC_BITS
    } else {
        -(((-q8) + ONE / 2) >> FRAC_BITS)
    }
}

/// Advance `pos` by `dist` logical pixels along `heading` degrees. Returns the new position.
/// dx = dist·cos(h), dy = dist·sin(h); y increases downward (framebuffer space).
pub fn advance(pos: FixedPos, heading: i64, dist: f64) -> FixedPos {
    // dist in Q8: round to nearest 1/256 px so the whole pipeline stays integer afterwards.
    // (no_std: integer-cast rounding — f64::round is std-only.)
    let scaled = dist * ONE as f64;
    let dist_q8 = if scaled >= 0.0 { (scaled + 0.5) as i64 } else { (scaled - 0.5) as i64 };
    // dx_q8 = dist_q8 * cos_q12 / 4096
    let dx = (dist_q8 * cos_q12(heading)) >> 12;
    let dy = (dist_q8 * sin_q12(heading)) >> 12;
    FixedPos { x: pos.x + dx, y: pos.y + dy }
}

/// Wrap a pixel coordinate into `[0, dim)` (torus topology, LANGUAGE.md §9).
pub fn wrap(p: i64, dim: i64) -> i64 {
    if dim <= 0 {
        return 0;
    }
    ((p % dim) + dim) % dim
}

/// Clamp a pixel coordinate into `[0, dim-1]` (the `klem` policy).
pub fn clamp(p: i64, dim: i64) -> i64 {
    if dim <= 0 {
        return 0;
    }
    p.max(0).min(dim - 1)
}

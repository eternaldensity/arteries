//! Fixed-point value noise, fBm, and ridged noise.
//!
//! Coordinates and outputs are scaled by [`NOISE_SCALE`]. Everything is
//! integer-only and deterministic: the same seed and coordinate always produce
//! the same value on every platform and build.
//!
//! The generator uses this module to build elevation, climate, and geology
//! fields without floating point or platform-dependent rounding.

/// Fixed-point scale for noise coordinates and values.
pub const NOISE_SCALE: i64 = 1_000_000;

/// Maximum octave count accepted by the noise helpers.
pub const MAX_OCTAVES: u32 = 8;

/// SplitMix-style hash for a lattice point.
#[must_use]
pub fn hash(seed: i64, x: i64, y: i64) -> u64 {
    let mut value = (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((x as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add((y as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn lattice(seed: i64, x: i64, y: i64) -> i64 {
    let span = u64::try_from(NOISE_SCALE).unwrap_or(1) + 1;
    i64::try_from(hash(seed, x, y) % span).unwrap_or(0)
}

fn lerp(a: i64, b: i64, t: i64) -> i64 {
    let a = i128::from(a);
    let b = i128::from(b);
    let t = i128::from(t);
    let scale = i128::from(NOISE_SCALE);
    i64::try_from(a + (b - a) * t / scale).unwrap_or(0)
}

fn smooth(t: i64) -> i64 {
    // 3t^2 - 2t^3 in fixed point.
    let t = i128::from(t);
    let scale = i128::from(NOISE_SCALE);
    let value = t * t * (3 * scale - 2 * t) / (scale * scale);
    i64::try_from(value).unwrap_or(0)
}

/// Value noise in `[0, NOISE_SCALE]` at a fixed-point coordinate.
#[must_use]
pub fn value(seed: i64, x: i64, y: i64) -> i64 {
    let x0 = x.div_euclid(NOISE_SCALE);
    let y0 = y.div_euclid(NOISE_SCALE);
    let tx = smooth(x.rem_euclid(NOISE_SCALE));
    let ty = smooth(y.rem_euclid(NOISE_SCALE));
    let top = lerp(lattice(seed, x0, y0), lattice(seed, x0 + 1, y0), tx);
    let bottom = lerp(lattice(seed, x0, y0 + 1), lattice(seed, x0 + 1, y0 + 1), tx);
    lerp(top, bottom, ty)
}

fn scaled_coordinate(value: i64, frequency: i64) -> i64 {
    i64::try_from(i128::from(value) * i128::from(frequency) / i128::from(NOISE_SCALE)).unwrap_or(0)
}

/// Fractal Brownian motion in `[0, NOISE_SCALE]`.
///
/// `octaves` is clamped to `1..=MAX_OCTAVES`. Each octave doubles the frequency
/// and keeps 55% of the previous amplitude.
#[must_use]
pub fn fbm(seed: i64, x: i64, y: i64, octaves: u32) -> i64 {
    let octaves = octaves.clamp(1, MAX_OCTAVES);
    let mut frequency = NOISE_SCALE;
    let mut amplitude = NOISE_SCALE;
    let mut total = 0_i128;
    let mut weight = 0_i128;
    for octave in 0..octaves {
        let sample = i128::from(value(
            seed.wrapping_add(i64::from(octave) * 0x9E37),
            scaled_coordinate(x, frequency),
            scaled_coordinate(y, frequency),
        ));
        total += sample * i128::from(amplitude);
        weight += i128::from(amplitude);
        amplitude = amplitude * 55 / 100;
        frequency = frequency.saturating_mul(2);
    }
    let result = total / weight.max(1);
    i64::try_from(result).unwrap_or(0)
}

/// Ridged fractal noise in `[0, NOISE_SCALE]`.
///
/// Ridges are the inverted absolute difference from the mid-point, which
/// produces mountain-belt-like creases. The octave/frequency behavior matches
/// [`fbm`].
#[must_use]
pub fn ridged(seed: i64, x: i64, y: i64, octaves: u32) -> i64 {
    let octaves = octaves.clamp(1, MAX_OCTAVES);
    let mut frequency = NOISE_SCALE;
    let mut amplitude = NOISE_SCALE;
    let mut total = 0_i128;
    let mut weight = 0_i128;
    for octave in 0..octaves {
        let noise = value(
            seed.wrapping_add(i64::from(octave) * 0x517C),
            scaled_coordinate(x, frequency),
            scaled_coordinate(y, frequency),
        );
        let ridge = NOISE_SCALE - (2 * noise - NOISE_SCALE).abs();
        total += i128::from(ridge) * i128::from(amplitude);
        weight += i128::from(amplitude);
        amplitude = amplitude * 55 / 100;
        frequency = frequency.saturating_mul(2);
    }
    let result = total / weight.max(1);
    i64::try_from(result).unwrap_or(0)
}

/// Domain warp in `[-strength, strength]`, returning a coordinate offset.
#[must_use]
pub fn warp(seed: i64, x: i64, y: i64, strength: i64) -> (i64, i64) {
    let wx = value(seed ^ 0x51C7, x, y) - NOISE_SCALE / 2;
    let wy = value(seed ^ 0x7A31, x, y) - NOISE_SCALE / 2;
    (
        i64::try_from(i128::from(wx) * i128::from(strength) / i128::from(NOISE_SCALE)).unwrap_or(0),
        i64::try_from(i128::from(wy) * i128::from(strength) / i128::from(NOISE_SCALE)).unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_noise_are_deterministic() {
        for seed in [0, 7, -3, 1_000_000_000] {
            for (x, y) in [(0, 0), (123_456, 789_012), (-40_000, 90_000)] {
                assert_eq!(hash(seed, x, y), hash(seed, x, y));
                assert_eq!(value(seed, x, y), value(seed, x, y));
                assert_eq!(fbm(seed, x, y, 4), fbm(seed, x, y, 4));
                assert_eq!(ridged(seed, x, y, 4), ridged(seed, x, y, 4));
            }
        }
    }

    #[test]
    fn noise_stays_in_range() {
        for seed in 0..16 {
            for x in (-3..3).map(|v| v * NOISE_SCALE / 2) {
                for y in (-3..3).map(|v| v * NOISE_SCALE / 2) {
                    assert!((0..=NOISE_SCALE).contains(&value(seed, x, y)));
                    let f = fbm(seed, x, y, 8);
                    assert!(
                        (0..=NOISE_SCALE).contains(&f),
                        "fbm({seed}, {x}, {y}) = {f}"
                    );
                    let r = ridged(seed, x, y, 8);
                    assert!(
                        (0..=NOISE_SCALE).contains(&r),
                        "ridged({seed}, {x}, {y}) = {r}"
                    );
                }
            }
        }
    }

    #[test]
    fn noise_is_smooth_across_lattice_cells() {
        let step = NOISE_SCALE / 4;
        for seed in 0..8 {
            let a = value(seed, 0, 0);
            let b = value(seed, step, 0);
            assert!((a - b).abs() <= NOISE_SCALE / 2);
        }
    }

    #[test]
    fn octave_clamping_is_total() {
        assert_eq!(fbm(7, 0, 0, 0), fbm(7, 0, 0, 1));
        assert_eq!(fbm(7, 0, 0, 99), fbm(7, 0, 0, MAX_OCTAVES));
        assert_eq!(ridged(7, 0, 0, 0), ridged(7, 0, 0, 1));
    }
}

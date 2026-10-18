//! Fixed-point numbers for the simulation: `i32` in 1/256 pixel.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use serde::Deserialize;

/// A fixed-point value in 1/256 pixel ("subpixels").
///
/// Positions, velocities and accelerations in the simulation all use `Fx`.
/// Addition and subtraction saturate at the `i32` limits; multiplication
/// and division of two `Fx` round to the nearest subpixel (ties away from
/// zero) and saturate.
/// In content files (`tuning.toml`) an `Fx` is written as a raw integer in
/// subpixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(transparent)]
pub struct Fx(pub i32);

/// Number of fractional bits in [`Fx`].
pub const FX_SHIFT: u32 = 8;

impl Fx {
    /// 0 px.
    pub const ZERO: Fx = Fx(0);
    /// 1 px (= 256 subpixels).
    pub const ONE: Fx = Fx(1 << FX_SHIFT);
    /// Largest representable value.
    pub const MAX: Fx = Fx(i32::MAX);
    /// Smallest representable value.
    pub const MIN: Fx = Fx(i32::MIN);

    /// Whole pixels to `Fx` (saturating).
    pub const fn from_px(px: i32) -> Fx {
        Fx(px.saturating_mul(1 << FX_SHIFT))
    }

    /// Raw subpixels (1/256 px) to `Fx`.
    pub const fn from_raw(raw: i32) -> Fx {
        Fx(raw)
    }

    /// Raw subpixel value.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Whole pixels, rounded towards negative infinity (floor).
    pub const fn to_px(self) -> i32 {
        self.0 >> FX_SHIFT
    }

    /// Absolute value (saturating: `MIN.abs() == MAX`).
    pub const fn abs(self) -> Fx {
        Fx(self.0.saturating_abs())
    }

    /// -1, 0 or 1 as a plain integer.
    pub const fn signum(self) -> i32 {
        self.0.signum()
    }

    /// Multiplies by a plain integer (saturating).
    pub const fn mul_int(self, n: i32) -> Fx {
        Fx(self.0.saturating_mul(n))
    }

    /// Divides by a plain integer, rounding to nearest (ties away from zero).
    /// Division by zero saturates towards the sign of `self` (0 stays 0).
    pub const fn div_int(self, n: i32) -> Fx {
        Fx(saturate(div_round(self.0 as i64, n as i64)))
    }

    /// Moves `self` towards `target` by at most `step` (`step` >= 0), never
    /// overshooting.
    pub fn approach(self, target: Fx, step: Fx) -> Fx {
        if self < target {
            (self + step).min(target)
        } else {
            (self - step).max(target)
        }
    }
}

/// Integer division rounding to nearest, ties away from zero. `d == 0`
/// returns the largest value with the sign of `n`.
const fn div_round(n: i64, d: i64) -> i64 {
    if d == 0 {
        return if n > 0 {
            i64::MAX
        } else if n < 0 {
            i64::MIN
        } else {
            0
        };
    }
    let half = d.abs() / 2;
    if (n < 0) == (d < 0) {
        (n.abs() + half) / d.abs()
    } else {
        -((n.abs() + half) / d.abs())
    }
}

/// `v` as an `i64` with 16 fractional bits (the numerator of a division).
const fn widen(v: Fx) -> i64 {
    (v.0 as i64) << FX_SHIFT
}

const fn saturate(v: i64) -> i32 {
    if v > i32::MAX as i64 {
        i32::MAX
    } else if v < i32::MIN as i64 {
        i32::MIN
    } else {
        v as i32
    }
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, rhs: Fx) -> Fx {
        Fx(self.0.saturating_add(rhs.0))
    }
}

impl Sub for Fx {
    type Output = Fx;
    fn sub(self, rhs: Fx) -> Fx {
        Fx(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for Fx {
    fn add_assign(&mut self, rhs: Fx) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fx {
    fn sub_assign(&mut self, rhs: Fx) {
        *self = *self - rhs;
    }
}

impl Neg for Fx {
    type Output = Fx;
    fn neg(self) -> Fx {
        Fx(self.0.saturating_neg())
    }
}

impl Mul for Fx {
    type Output = Fx;
    /// Fixed-point product, rounded to nearest subpixel (ties away from zero).
    fn mul(self, rhs: Fx) -> Fx {
        let p = self.0 as i64 * rhs.0 as i64;
        Fx(saturate(div_round(p, 1 << FX_SHIFT)))
    }
}

impl Div for Fx {
    type Output = Fx;
    /// Fixed-point quotient, rounded to nearest subpixel (ties away from
    /// zero). Division by zero saturates towards the sign of `self`.
    fn div(self, rhs: Fx) -> Fx {
        Fx(saturate(div_round(widen(self), rhs.0 as i64)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_conversions() {
        assert_eq!(Fx::ONE.raw(), 256);
        assert_eq!(Fx::from_px(3).raw(), 768);
        assert_eq!(Fx::from_px(-2).to_px(), -2);
        assert_eq!(Fx::from_raw(383).to_px(), 1);
        assert_eq!(Fx::from_raw(-1).to_px(), -1, "to_px floors");
        assert_eq!(Fx::from_raw(-256).to_px(), -1);
        assert_eq!(Fx::from_raw(-257).to_px(), -2);
        assert_eq!(Fx::from_px(i32::MAX), Fx::MAX);
    }

    #[test]
    fn add_sub_neg_saturate() {
        assert_eq!(Fx(5) + Fx(7), Fx(12));
        assert_eq!(Fx(5) - Fx(7), Fx(-2));
        assert_eq!(-Fx(5), Fx(-5));
        assert_eq!(Fx::MAX + Fx(1), Fx::MAX);
        assert_eq!(Fx::MIN - Fx(1), Fx::MIN);
        assert_eq!(-Fx::MIN, Fx::MAX);
        let mut a = Fx(10);
        a += Fx(5);
        a -= Fx(3);
        assert_eq!(a, Fx(12));
    }

    #[test]
    fn mul_rounds_to_nearest() {
        assert_eq!(Fx::from_px(3) * Fx::from_px(2), Fx::from_px(6));
        // 1.5 * 1.5 = 2.25
        assert_eq!(Fx(384) * Fx(384), Fx(576));
        // 1/256 * 1/2 = 1/512 -> ties away from zero -> 1/256
        assert_eq!(Fx(1) * Fx(128), Fx(1));
        assert_eq!(Fx(-1) * Fx(128), Fx(-1));
        // 1/256 * 0.25 = 1/1024 -> rounds to 0
        assert_eq!(Fx(1) * Fx(64), Fx(0));
        assert_eq!(Fx(-3) * Fx(-128), Fx(2), "1.5 rounds away from zero");
        assert_eq!(Fx::MAX * Fx::from_px(2), Fx::MAX);
        assert_eq!(Fx::MAX * Fx::from_px(-2), Fx::MIN);
    }

    #[test]
    fn div_rounds_to_nearest() {
        assert_eq!(Fx::from_px(6) / Fx::from_px(2), Fx::from_px(3));
        // 1 / 3 = 85.33 subpixels -> 85
        assert_eq!(Fx::ONE / Fx::from_px(3), Fx(85));
        // 2 / 3 = 170.67 -> 171
        assert_eq!(Fx::from_px(2) / Fx::from_px(3), Fx(171));
        assert_eq!(Fx::from_px(-2) / Fx::from_px(3), Fx(-171));
        assert_eq!(Fx::from_px(2) / Fx::from_px(-3), Fx(-171));
        assert_eq!(Fx::ONE / Fx::ZERO, Fx::MAX);
        assert_eq!(-Fx::ONE / Fx::ZERO, Fx::MIN);
        assert_eq!(Fx::ZERO / Fx::ZERO, Fx::ZERO);
        assert_eq!(Fx::MAX / Fx(1), Fx::MAX);
    }

    #[test]
    fn integer_ops() {
        assert_eq!(Fx(10).mul_int(3), Fx(30));
        assert_eq!(Fx(10).div_int(4), Fx(3), "2.5 rounds away from zero");
        assert_eq!(Fx(-10).div_int(4), Fx(-3));
        assert_eq!(Fx(9).div_int(4), Fx(2));
        assert_eq!(Fx(9).div_int(0), Fx::MAX);
        assert_eq!(Fx::MAX.mul_int(2), Fx::MAX);
    }

    #[test]
    fn abs_signum_approach() {
        assert_eq!(Fx(-7).abs(), Fx(7));
        assert_eq!(Fx::MIN.abs(), Fx::MAX);
        assert_eq!(Fx(-7).signum(), -1);
        assert_eq!(Fx(0).signum(), 0);
        assert_eq!(Fx(10).approach(Fx(0), Fx(4)), Fx(6));
        assert_eq!(Fx(3).approach(Fx(0), Fx(4)), Fx(0));
        assert_eq!(Fx(-3).approach(Fx(0), Fx(4)), Fx(0));
        assert_eq!(Fx(0).approach(Fx(10), Fx(4)), Fx(4));
    }
}

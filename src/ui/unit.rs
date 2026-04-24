use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Dp(pub f32);

impl Dp {
    pub const ZERO: Self = Self(0.0);

    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn max(self, other: impl Into<Self>) -> Self {
        let other = other.into();
        Self(self.0.max(other.0))
    }

    pub fn min(self, other: impl Into<Self>) -> Self {
        let other = other.into();
        Self(self.0.min(other.0))
    }

    pub fn clamp(self, min: impl Into<Self>, max: impl Into<Self>) -> Self {
        let min = min.into();
        let max = max.into();
        Self(self.0.clamp(min.0, max.0))
    }

    pub fn round(self) -> Self {
        Self(self.0.round())
    }

    pub fn ceil(self) -> Self {
        Self(self.0.ceil())
    }

    pub fn floor(self) -> Self {
        Self(self.0.floor())
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }
}

impl fmt::Debug for Dp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}dp", self.0)
    }
}

impl fmt::Display for Dp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}dp", self.0)
    }
}

impl From<Dp> for f32 {
    fn from(value: Dp) -> Self {
        value.0
    }
}

impl From<f32> for Dp {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<f64> for Dp {
    fn from(value: f64) -> Self {
        Self(value as f32)
    }
}

impl From<i32> for Dp {
    fn from(value: i32) -> Self {
        Self(value as f32)
    }
}

impl From<u32> for Dp {
    fn from(value: u32) -> Self {
        Self(value as f32)
    }
}

impl PartialEq<f32> for Dp {
    fn eq(&self, other: &f32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<f32> for Dp {
    fn partial_cmp(&self, other: &f32) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<Dp> for f32 {
    fn eq(&self, other: &Dp) -> bool {
        *self == other.0
    }
}

impl PartialOrd<Dp> for f32 {
    fn partial_cmp(&self, other: &Dp) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add for Dp {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<f32> for Dp {
    type Output = Self;

    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign for Dp {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl AddAssign<f32> for Dp {
    fn add_assign(&mut self, rhs: f32) {
        self.0 += rhs;
    }
}

impl Sub for Dp {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<f32> for Dp {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl SubAssign for Dp {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl SubAssign<f32> for Dp {
    fn sub_assign(&mut self, rhs: f32) {
        self.0 -= rhs;
    }
}

impl Mul<f32> for Dp {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Mul<Dp> for f32 {
    type Output = Dp;

    fn mul(self, rhs: Dp) -> Self::Output {
        Dp(self * rhs.0)
    }
}

impl Div<f32> for Dp {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Div for Dp {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl Neg for Dp {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

#[derive(Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Sp(pub f32);

impl Sp {
    pub const ZERO: Self = Self(0.0);

    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn max(self, other: impl Into<Self>) -> Self {
        let other = other.into();
        Self(self.0.max(other.0))
    }

    pub fn min(self, other: impl Into<Self>) -> Self {
        let other = other.into();
        Self(self.0.min(other.0))
    }

    pub fn clamp(self, min: impl Into<Self>, max: impl Into<Self>) -> Self {
        let min = min.into();
        let max = max.into();
        Self(self.0.clamp(min.0, max.0))
    }
}

impl fmt::Debug for Sp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}sp", self.0)
    }
}

impl fmt::Display for Sp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}sp", self.0)
    }
}

impl From<Sp> for f32 {
    fn from(value: Sp) -> Self {
        value.0
    }
}

impl From<f32> for Sp {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<f64> for Sp {
    fn from(value: f64) -> Self {
        Self(value as f32)
    }
}

impl From<i32> for Sp {
    fn from(value: i32) -> Self {
        Self(value as f32)
    }
}

impl From<u32> for Sp {
    fn from(value: u32) -> Self {
        Self(value as f32)
    }
}

impl Add for Sp {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Sp {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<f32> for Sp {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f32> for Sp {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Neg for Sp {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

pub const fn dp(value: f32) -> Dp {
    Dp::new(value)
}

pub const fn sp(value: f32) -> Sp {
    Sp::new(value)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UnitContext {
    scale_factor: f32,
    font_scale: f32,
}

impl Default for UnitContext {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

impl UnitContext {
    pub(crate) fn new(scale_factor: f32, font_scale: f32) -> Self {
        Self {
            scale_factor: scale_factor.max(1.0 / 64.0),
            font_scale: font_scale.max(1.0 / 64.0),
        }
    }

    pub(crate) fn scale_factor(self) -> f32 {
        self.scale_factor
    }

    pub(crate) fn resolve_dp(self, value: Dp) -> f32 {
        value.get()
    }

    pub(crate) fn resolve_sp(self, value: Sp) -> f32 {
        value.get() * self.font_scale
    }

    pub(crate) fn logical_to_physical(self, value: f32) -> f32 {
        value * self.scale_factor
    }
}

#[cfg(test)]
mod tests {
    use super::{dp, sp, UnitContext};

    #[test]
    fn resolve_sp_applies_font_scale() {
        let units = UnitContext::new(2.0, 1.5);
        assert_eq!(units.resolve_sp(sp(16.0)), 24.0);
    }

    #[test]
    fn resolve_dp_ignores_font_scale() {
        let units = UnitContext::new(2.0, 1.5);
        assert_eq!(units.resolve_dp(dp(16.0)), 16.0);
    }
}

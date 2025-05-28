use crate::Val;
use bevy_derive::Deref;
use bevy_ecs::component::Component;
use bevy_ecs::prelude::ReflectComponent;
use bevy_math::Affine2;
use bevy_math::Vec2;
use bevy_reflect::prelude::*;
use core::f32::consts::PI;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Div;
use std::ops::DivAssign;
use std::ops::Mul;
use std::ops::MulAssign;
use std::ops::Sub;
use std::ops::SubAssign;

#[derive(Debug, Default, PartialEq, Clone, Copy, Reflect)]
pub struct CVal {
    pub px: f32,
    pub percent: f32,
    pub vw: f32,
    pub vh: f32,
    pub vmin: f32,
    pub vmax: f32,
}

impl Add for CVal {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            px: self.px + other.px,
            percent: self.percent + other.percent,
            vw: self.vw + other.vw,
            vh: self.vh + other.vh,
            vmin: self.vmin + other.vmin,
            vmax: self.vmax + other.vmax,
        }
    }
}

impl Sub for CVal {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            px: self.px - other.px,
            percent: self.percent - other.percent,
            vw: self.vw - other.vw,
            vh: self.vh - other.vh,
            vmin: self.vmin - other.vmin,
            vmax: self.vmax - other.vmax,
        }
    }
}

impl Add<Val> for CVal {
    type Output = Self;

    fn add(mut self, other: Val) -> Self {
        match other {
            Val::Px(px) => self.px += px,
            Val::Percent(percent) => self.percent += percent,
            Val::Vw(vw) => self.vw += vw,
            Val::Vh(vh) => self.vh += vh,
            Val::VMin(vmin) => self.vmin += vmin,
            Val::VMax(vmax) => self.vmax += vmax,
            _ => {}
        }
        self
    }
}

impl Sub<Val> for CVal {
    type Output = Self;

    fn sub(mut self, other: Val) -> Self {
        match other {
            Val::Px(px) => self.px -= px,
            Val::Percent(percent) => self.percent -= percent,
            Val::Vw(vw) => self.vw -= vw,
            Val::Vh(vh) => self.vh -= vh,
            Val::VMin(vmin) => self.vmin -= vmin,
            Val::VMax(vmax) => self.vmax -= vmax,
            _ => {}
        }
        self
    }
}

impl Mul<f32> for CVal {
    type Output = Self;

    fn mul(mut self, other: f32) -> Self {
        self.px *= other;
        self.percent *= other;
        self.vw *= other;
        self.vh *= other;
        self.vmin *= other;
        self.vmax *= other;
        self
    }
}

impl Div<f32> for CVal {
    type Output = Self;

    fn div(mut self, other: f32) -> Self {
        if other != 0. {
            self.px /= other;
            self.percent /= other;
            self.vw /= other;
            self.vh /= other;
            self.vmin /= other;
            self.vmax /= other;
        }
        self
    }
}

impl Mul<CVal> for f32 {
    type Output = CVal;

    fn mul(self, rhs: CVal) -> CVal {
        CVal {
            px: self * rhs.px,
            percent: self * rhs.percent,
            vw: self * rhs.vw,
            vh: self * rhs.vh,
            vmin: self * rhs.vmin,
            vmax: self * rhs.vmax,
        }
    }
}

impl AddAssign for CVal {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl AddAssign<Val> for CVal {
    fn add_assign(&mut self, other: Val) {
        *self = *self + other;
    }
}
impl SubAssign for CVal {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}
impl SubAssign<Val> for CVal {
    fn sub_assign(&mut self, other: Val) {
        *self = *self - other;
    }
}

impl Add<f32> for CVal {
    type Output = Self;

    fn add(mut self, other: f32) -> Self {
        self.px += other;
        self.percent += other;
        self.vw += other;
        self.vh += other;
        self.vmin += other;
        self.vmax += other;
        self
    }
}

impl Sub<f32> for CVal {
    type Output = Self;

    fn sub(mut self, other: f32) -> Self {
        self.px -= other;
        self.percent -= other;
        self.vw -= other;
        self.vh -= other;
        self.vmin -= other;
        self.vmax -= other;
        self
    }
}

impl AddAssign<f32> for CVal {
    fn add_assign(&mut self, other: f32) {
        *self = *self + other;
    }
}

impl SubAssign<f32> for CVal {
    fn sub_assign(&mut self, other: f32) {
        *self = *self - other;
    }
}

impl MulAssign<f32> for CVal {
    fn mul_assign(&mut self, other: f32) {
        *self = *self * other;
    }
}

impl DivAssign<f32> for CVal {
    fn div_assign(&mut self, other: f32) {
        *self = *self / other;
    }
}

impl From<Val> for CVal {
    fn from(val: Val) -> Self {
        let mut cval = Self::default();
        match val {
            Val::Px(px) => cval.px = px,
            Val::Percent(percent) => cval.percent = percent,
            Val::Vw(vw) => cval.vw = vw,
            Val::Vh(vh) => cval.vh = vh,
            Val::VMin(vmin) => cval.vmin = vmin,
            Val::VMax(vmax) => cval.vmax = vmax,
            Val::Auto => {}
        }
        cval
    }
}

impl From<f32> for CVal {
    fn from(px: f32) -> Self {
        Self {
            px,
            percent: 0.,
            vw: 0.,
            vh: 0.,
            vmin: 0.,
            vmax: 0.,
        }
    }
}

impl CVal {
    pub const ZERO: Self = Self {
        px: 0.,
        percent: 0.,
        vw: 0.,
        vh: 0.,
        vmin: 0.,
        vmax: 0.,
    };

    pub const fn resolve(&self, scale_factor: f32, base_size: f32, viewport_size: Vec2) -> f32 {
        self.px * scale_factor
            + self.percent * base_size / 100.
            + self.vw * viewport_size.x / 100.
            + self.vh * viewport_size.y / 100.
            + self.vmin * viewport_size.x.min(viewport_size.y) / 100.
            + self.vmax * viewport_size.x.max(viewport_size.y) / 100.
    }

    pub const fn px(px: f32) -> Self {
        Self {
            px,
            percent: 0.,
            vw: 0.,
            vh: 0.,
            vmin: 0.,
            vmax: 0.,
        }
    }
    pub const fn percent(percent: f32) -> Self {
        Self {
            px: 0.,
            percent,
            vw: 0.,
            vh: 0.,
            vmin: 0.,
            vmax: 0.,
        }
    }
    pub const fn vw(vw: f32) -> Self {
        Self {
            px: 0.,
            percent: 0.,
            vw,
            vh: 0.,
            vmin: 0.,
            vmax: 0.,
        }
    }
    pub const fn vh(vh: f32) -> Self {
        Self {
            px: 0.,
            percent: 0.,
            vw: 0.,
            vh,
            vmin: 0.,
            vmax: 0.,
        }
    }
    pub const fn vmin(vmin: f32) -> Self {
        Self {
            px: 0.,
            percent: 0.,
            vw: 0.,
            vh: 0.,
            vmin,
            vmax: 0.,
        }
    }
    pub const fn vmax(vmax: f32) -> Self {
        Self {
            px: 0.,
            percent: 0.,
            vw: 0.,
            vh: 0.,
            vmin: 0.,
            vmax,
        }
    }
}

/// A pair of [`Val`]s used to representin a 2-dimensional size or offset.
#[derive(Debug, PartialEq, Clone, Copy, Reflect)]
#[reflect(Default, PartialEq, Debug, Clone)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    reflect(Serialize, Deserialize)
)]
pub struct CVal2 {
    /// Translate the node along the x-axis.
    /// `Val::Percent` values are resolved based on the computed width of the Ui Node.
    /// `Val::Auto` is resolved to `0.`.
    pub x: CVal,
    /// Translate the node along the y-axis.
    /// `Val::Percent` values are resolved based on the computed width of the Ui Node.
    /// `Val::Auto` is resolved to `0.`.
    pub y: CVal,
}

impl CVal2 {
    pub const ZERO: Self = Self {
        x: CVal::ZERO,
        y: CVal::ZERO,
    };

    /// Creates a new [`Val2`] where both components are in logical pixels
    pub const fn px(x: f32, y: f32) -> Self {
        Self {
            x: CVal::px(x),
            y: CVal::px(y),
        }
    }

    /// Creates a new [`Val2`] where both components are percentage values
    pub const fn percent(x: f32, y: f32) -> Self {
        Self {
            x: CVal::percent(x),
            y: CVal::percent(y),
        }
    }

    /// Creates a new [`Val2`]
    pub fn new(x: Val, y: Val) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }

    /// Resolves this [`Val2`] from the given `scale_factor`, `parent_size`,
    /// and `viewport_size`.
    ///
    /// Component values of [`Val::Auto`] are resolved to 0.
    pub fn resolve(&self, scale_factor: f32, base_size: Vec2, viewport_size: Vec2) -> Vec2 {
        Vec2::new(
            self.x.resolve(scale_factor, base_size.x, viewport_size),
            self.y.resolve(scale_factor, base_size.y, viewport_size),
        )
    }
}

impl Default for CVal2 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<Vec2> for CVal2 {
    fn from(value: Vec2) -> Self {
        Self {
            x: value.x.into(),
            y: value.y.into(),
        }
    }
}

impl Add<Vec2> for CVal2 {
    type Output = Self;

    fn add(self, other: Vec2) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub<Vec2> for CVal2 {
    type Output = Self;

    fn sub(self, other: Vec2) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl AddAssign<Vec2> for CVal2 {
    fn add_assign(&mut self, other: Vec2) {
        *self = *self + other;
    }
}

impl SubAssign<Vec2> for CVal2 {
    fn sub_assign(&mut self, other: Vec2) {
        *self = *self - other;
    }
}

impl Add for CVal2 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub for CVal2 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl AddAssign for CVal2 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl SubAssign for CVal2 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl Mul<f32> for CVal2 {
    type Output = Self;

    fn mul(self, other: f32) -> Self {
        Self {
            x: self.x * other,
            y: self.y * other,
        }
    }
}
impl Div<f32> for CVal2 {
    type Output = Self;

    fn div(self, other: f32) -> Self {
        Self {
            x: self.x / other,
            y: self.y / other,
        }
    }
}

impl MulAssign<f32> for CVal2 {
    fn mul_assign(&mut self, other: f32) {
        *self = *self * other;
    }
}

impl DivAssign<f32> for CVal2 {
    fn div_assign(&mut self, other: f32) {
        *self = *self / other;
    }
}

impl Mul<CVal2> for f32 {
    type Output = CVal2;

    fn mul(self, rhs: CVal2) -> CVal2 {
        CVal2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

/// Relative 2D transform for UI nodes
///
/// [`UiGlobalTransform`] is automatically inserted whenever [`UiTransform`] is inserted.
#[derive(Component, Debug, PartialEq, Clone, Copy, Reflect)]
#[reflect(Component, Default, PartialEq, Debug, Clone)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    reflect(Serialize, Deserialize)
)]
#[require(UiGlobalTransform)]
pub struct UiTransform {
    /// Translate the node.
    pub translation: CVal2,
    /// Scale the node. A negative value reflects the node in that axis.
    pub scale: Vec2,
    /// Rotate the node clockwise by the given value in radians.
    pub rotation: f32,
}

impl UiTransform {
    pub const IDENTITY: Self = Self {
        translation: CVal2::ZERO,
        scale: Vec2::ONE,
        rotation: 0.,
    };

    /// Creates a UI transform representing a rotation in `angle` radians.
    pub fn from_angle(angle: f32) -> Self {
        Self {
            rotation: angle,
            ..Self::IDENTITY
        }
    }

    /// Creates a UI transform representing a rotation in `angle` degrees.
    pub fn from_angle_deg(angle: f32) -> Self {
        Self {
            rotation: PI * angle / 180.,
            ..Self::IDENTITY
        }
    }

    /// Creates a UI transform representing a responsive translation.
    pub fn from_translation(translation: CVal2) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    /// Creates a UI transform representing a scaling.
    pub fn from_scale(scale: Vec2) -> Self {
        Self {
            scale,
            ..Self::IDENTITY
        }
    }
}

impl Default for UiTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Absolute 2D transform for UI nodes
///
/// [`UiGlobalTransform`]s are updated from [`UiTransform`] and [`Node`](crate::ui_node::Node)
///  in [`ui_layout_system`](crate::layout::ui_layout_system)
#[derive(Component, Debug, PartialEq, Clone, Copy, Reflect, Deref)]
#[reflect(Component, Default, PartialEq, Debug, Clone)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    reflect(Serialize, Deserialize)
)]
pub struct UiGlobalTransform(Affine2);

impl Default for UiGlobalTransform {
    fn default() -> Self {
        Self(Affine2::IDENTITY)
    }
}

impl UiGlobalTransform {
    /// If the transform is invertible returns its inverse.
    /// Otherwise returns `None`.
    #[inline]
    pub fn try_inverse(&self) -> Option<Affine2> {
        (self.matrix2.determinant() != 0.).then_some(self.inverse())
    }
}

impl From<Affine2> for UiGlobalTransform {
    fn from(value: Affine2) -> Self {
        Self(value)
    }
}

impl From<UiGlobalTransform> for Affine2 {
    fn from(value: UiGlobalTransform) -> Self {
        value.0
    }
}

impl From<&UiGlobalTransform> for Affine2 {
    fn from(value: &UiGlobalTransform) -> Self {
        value.0
    }
}

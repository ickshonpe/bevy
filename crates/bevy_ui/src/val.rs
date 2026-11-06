#![expect(
    unsafe_code,
    reason = "`Val` is a tagged union that owns the `Calc` behind its pointer, like a `Box`."
)]

use bevy_math::{StableInterpolate, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_text::{EmSize, FontSize, RemSize, DEFAULT_REM_SIZE_PX};
use core::ops::{Add, Div, DivAssign, Mul, MulAssign, Neg, Sub};
use smallvec::{smallvec, SmallVec};
use thiserror::Error;

#[cfg(feature = "serialize")]
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

use crate::UiRect;

/// All the context needed to resolve a [`Val`] into a length in physical pixels, except the percentage basis.
#[derive(Copy, Clone)]
pub struct ValContext {
    /// Logical pixel lengths are multiplied by the scale factor to get a physical length.
    pub scale_factor: f32,
    /// Size of the target surface in physical pixels.
    pub physical_size: Vec2,
    /// The local font size in logical pixels.
    /// `Val::em` values are normalized, so `em(1)` is `em_size * scale_factor` in physical pixels.
    pub em_size: f32,
    /// The root font size in logical pixels.
    /// `Val::rem` values are normalized, so `rem(1)` is `rem_size * scale_factor` in physical pixels.
    pub rem_size: f32,
}

impl ValContext {
    pub const DEFAULT: Self = Self {
        scale_factor: 1.0,
        physical_size: Vec2::ZERO,
        em_size: DEFAULT_REM_SIZE_PX,
        rem_size: DEFAULT_REM_SIZE_PX,
    };

    /// Create a new [`LayoutContext`] from the window's physical size and scale factor
    #[inline]
    pub(super) const fn new(
        scale_factor: f32,
        physical_size: Vec2,
        em_size: EmSize,
        rem_size: RemSize,
    ) -> Self {
        Self {
            scale_factor,
            physical_size,
            em_size: em_size.0,
            rem_size: rem_size.0,
        }
    }
}

#[cfg(test)]
impl ValContext {
    pub const TEST_CONTEXT: Self = Self {
        physical_size: Vec2::new(1000.0, 1000.0),
        ..Self::DEFAULT
    };
}

impl Default for ValContext {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Represents the possible value types for layout properties.
///
/// This type allows specifying values for various [`Node`](crate::Node) properties in different units,
/// such as logical pixels, percentages, or automatically determined values.
///
/// A `Val` is packed into eight bytes: a tag, then either an `f32` or a pointer to a boxed
/// [`Calc`] expression. It is built with [`Val::AUTO`], [`Val::px`] and the other constructors,
/// and matched on through the [`ValRef`] returned by [`Val::unpack`].
///
/// `Val` also implements [`core::str::FromStr`] to allow parsing values from strings in the format `#.#px`. Whitespaces between the value and unit is allowed. The following units are supported:
/// * `px`: logical pixels
/// * `%`: percentage
/// * `vw`: percentage of the viewport width
/// * `vh`: percentage of the viewport height
/// * `vmin`: percentage of the viewport's smaller dimension
/// * `vmax`: percentage of the viewport's larger dimension
///
/// Additionally, `auto` will be parsed as [`Val::AUTO`].
#[derive(Reflect)]
#[reflect(opaque, Default, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
pub struct Val(ValRepr);

/// The tag is the low nibble of the first byte.
const TAG_MASK: u8 = 0b1111;
const CALC_TAG: u8 = 0;
const AUTO_TAG: u8 = 1;
/// A unit value's tag is `UNIT_TAG + unit as u8`.
const UNIT_TAG: u8 = 2;

/// `AUTO` and the unit values.
#[derive(Clone, Copy)]
#[repr(C)]
struct UnitVariant {
    tag: u8,
    value: f32,
}

/// A pointer to a boxed [`Calc`]. On 64-bit little-endian targets the pointer's low byte is the
/// tag: `Calc` is aligned to 16 bytes, so its low nibble is `CALC_TAG`.
#[derive(Clone, Copy)]
#[repr(C)]
struct CalcVariant {
    #[cfg(not(all(target_pointer_width = "64", target_endian = "little")))]
    tag: u8,
    ptr: *mut Calc,
}

const _: () = assert!(align_of::<Calc>() > TAG_MASK as usize);

/// Both variants start with the tag byte.
#[derive(Clone, Copy)]
#[repr(C)]
union ValRepr {
    unit: UnitVariant,
    calc: CalcVariant,
}

/// A view of a [`Val`] returned by [`Val::unpack`], so that it can be matched on.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize), serde(rename = "Val"))]
pub enum ValRef<'a> {
    /// See [`Val::AUTO`].
    Auto,
    /// See [`Val::px`].
    Px(f32),
    /// See [`Val::percent`].
    Percent(f32),
    /// See [`Val::vw`].
    Vw(f32),
    /// See [`Val::vh`].
    Vh(f32),
    /// See [`Val::vmin`].
    VMin(f32),
    /// See [`Val::vmax`].
    VMax(f32),
    /// See [`Val::em`].
    Em(f32),
    /// See [`Val::rem`].
    Rem(f32),
    /// An expression combining `Val`s with different units, such as `Val::percent(50.0) - Val::px(10.0)`.
    Calc(&'a Calc),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValParseError {
    UnitMissing,
    ValueMissing,
    InvalidValue,
    InvalidUnit,
}

impl core::fmt::Display for ValParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ValParseError::UnitMissing => write!(f, "unit missing"),
            ValParseError::ValueMissing => write!(f, "value missing"),
            ValParseError::InvalidValue => write!(f, "invalid value"),
            ValParseError::InvalidUnit => write!(f, "invalid unit"),
        }
    }
}

impl core::str::FromStr for Val {
    type Err = ValParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        if s.eq_ignore_ascii_case("auto") {
            return Ok(Val::AUTO);
        }

        let Some(end_of_number) = s
            .bytes()
            .position(|c| !(c.is_ascii_digit() || c == b'.' || c == b'-' || c == b'+'))
        else {
            return Err(ValParseError::UnitMissing);
        };

        if end_of_number == 0 {
            return Err(ValParseError::ValueMissing);
        }

        let (value, unit) = s.split_at(end_of_number);

        let value: f32 = value.parse().map_err(|_| ValParseError::InvalidValue)?;

        let unit = unit.trim();

        if unit.eq_ignore_ascii_case("px") {
            Ok(Val::px(value))
        } else if unit.eq_ignore_ascii_case("%") {
            Ok(Val::percent(value))
        } else if unit.eq_ignore_ascii_case("vw") {
            Ok(Val::vw(value))
        } else if unit.eq_ignore_ascii_case("vh") {
            Ok(Val::vh(value))
        } else if unit.eq_ignore_ascii_case("vmin") {
            Ok(Val::vmin(value))
        } else if unit.eq_ignore_ascii_case("vmax") {
            Ok(Val::vmax(value))
        } else if unit.eq_ignore_ascii_case("rem") {
            Ok(Val::rem(value))
        } else if unit.eq_ignore_ascii_case("em") {
            Ok(Val::em(value))
        } else {
            Err(ValParseError::InvalidUnit)
        }
    }
}

impl core::fmt::Debug for Val {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.unpack().fmt(f)
    }
}

impl PartialEq for Val {
    fn eq(&self, other: &Self) -> bool {
        match (self.unpack(), other.unpack()) {
            (ValRef::Auto, ValRef::Auto) => true,
            (ValRef::Calc(a), ValRef::Calc(b)) => a == b,
            _ => match (self.unit(), self.inner(), other.unit(), other.inner()) {
                (Some(unit), Some(a), Some(other_unit), Some(b)) if unit == other_unit => a == b,
                // All zero-value variants are considered equal.
                (Some(_), Some(0.), Some(_), Some(0.)) => true,
                _ => false,
            },
        }
    }
}

impl Clone for Val {
    fn clone(&self) -> Self {
        match self.calc() {
            Some(calc) => Self::boxed(Box::new(calc.clone())),
            None => Self(self.0),
        }
    }
}

impl Drop for Val {
    fn drop(&mut self) {
        if self.tag() == CALC_TAG {
            // SAFETY: As in `calc`.
            drop(unsafe { Box::from_raw(self.0.calc.ptr) });
        }
    }
}

// SAFETY: A calc `Val` uniquely owns its `Box<Calc>`, which is `Send` and `Sync`.
unsafe impl Send for Val {}
// SAFETY: As above.
unsafe impl Sync for Val {}

#[cfg(feature = "serialize")]
impl serde::Serialize for Val {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.unpack().serialize(serializer)
    }
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for Val {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// The owned form of [`ValRef`], which is how a `Val` is serialized.
        #[derive(serde::Deserialize)]
        #[serde(rename = "Val")]
        enum Repr {
            Auto,
            Px(f32),
            Percent(f32),
            Vw(f32),
            Vh(f32),
            VMin(f32),
            VMax(f32),
            Em(f32),
            Rem(f32),
            Calc(Calc),
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Auto => Val::AUTO,
            Repr::Px(value) => Val::px(value),
            Repr::Percent(value) => Val::percent(value),
            Repr::Vw(value) => Val::vw(value),
            Repr::Vh(value) => Val::vh(value),
            Repr::VMin(value) => Val::vmin(value),
            Repr::VMax(value) => Val::vmax(value),
            Repr::Em(value) => Val::em(value),
            Repr::Rem(value) => Val::rem(value),
            Repr::Calc(calc) => Val::boxed(Box::new(calc)),
        })
    }
}

impl Val {
    pub const DEFAULT: Self = Self::AUTO;

    /// Zero. In [`Val::px`], but zero is the same in any unit.
    pub const ZERO: Self = Self::px(0.0);

    /// The largest finite [`Val::px`] value.
    pub const MAX: Self = Self::px(f32::MAX);

    /// Automatically determine the value based on the context and other [`Node`](crate::Node) properties.
    ///
    /// Treated as zero in expressions: `Val::AUTO + Val::px(10.0)` is `Val::px(10.0)`.
    pub const AUTO: Self = Self(ValRepr {
        unit: UnitVariant {
            tag: AUTO_TAG,
            value: 0.0,
        },
    });

    /// A value in logical pixels.
    pub const fn px(value: f32) -> Self {
        Self::leaf(ValUnit::Px, value)
    }

    /// A percentage of the parent node's length along a specific axis.
    ///
    /// If the UI node has no parent, the percentage is calculated based on the window's length
    /// along the corresponding axis.
    ///
    /// The chosen axis depends on the [`Node`](crate::Node) field set:
    /// * For `flex_basis`, the percentage is relative to the main-axis length determined by the `flex_direction`.
    /// * For `gap`, `min_size`, `size`, and `max_size`:
    ///   - `width` is relative to the parent's width.
    ///   - `height` is relative to the parent's height.
    /// * For `margin`, `padding`, and `border` values: the percentage is relative to the parent node's width.
    /// * For positions, `left` and `right` are relative to the parent's width, while `bottom` and `top` are relative to the parent's height.
    pub const fn percent(value: f32) -> Self {
        Self::leaf(ValUnit::Percent, value)
    }

    /// A percentage of the viewport width.
    pub const fn vw(value: f32) -> Self {
        Self::leaf(ValUnit::Vw, value)
    }

    /// A percentage of the viewport height.
    pub const fn vh(value: f32) -> Self {
        Self::leaf(ValUnit::Vh, value)
    }

    /// A percentage of the viewport's smaller dimension.
    pub const fn vmin(value: f32) -> Self {
        Self::leaf(ValUnit::VMin, value)
    }

    /// A percentage of the viewport's larger dimension.
    pub const fn vmax(value: f32) -> Self {
        Self::leaf(ValUnit::VMax, value)
    }

    /// A multiple of the node's own font size.
    ///
    /// `1em` is the node's font size, so `Val::em(2.0)` is twice that.
    /// Resolved from the node's [`EmSize`] component.
    pub const fn em(value: f32) -> Self {
        Self::leaf(ValUnit::Em, value)
    }

    /// A multiple of the root font size.
    ///
    /// Unlike [`Val::em`] this ignores the node's own font size, so it means the same
    /// length anywhere in the hierarchy. Resolved from the [`RemSize`] resource.
    pub const fn rem(value: f32) -> Self {
        Self::leaf(ValUnit::Rem, value)
    }

    /// Construct a plain unit value.
    pub(crate) const fn leaf(unit: ValUnit, value: f32) -> Self {
        Self(ValRepr {
            unit: UnitVariant {
                tag: UNIT_TAG + unit as u8,
                value,
            },
        })
    }

    /// Construct an expression, taking ownership of the box.
    fn boxed(calc: Box<Calc>) -> Self {
        Self(ValRepr {
            calc: CalcVariant {
                #[cfg(not(all(target_pointer_width = "64", target_endian = "little")))]
                tag: CALC_TAG,
                ptr: Box::into_raw(calc),
            },
        })
    }

    const fn tag(&self) -> u8 {
        // SAFETY: Both variants write the first byte. For `CalcVariant` on 64-bit little-endian
        // targets it's the low byte of a pointer aligned past `TAG_MASK`, so the masked tag is
        // `CALC_TAG`.
        unsafe { self.0.unit.tag & TAG_MASK }
    }

    /// Returns a view of this `Val` that can be matched on.
    pub fn unpack(&self) -> ValRef<'_> {
        if let Some(calc) = self.calc() {
            return ValRef::Calc(calc);
        }
        let (Some(unit), Some(value)) = (self.unit(), self.inner()) else {
            return ValRef::Auto;
        };
        match unit {
            ValUnit::Px => ValRef::Px(value),
            ValUnit::Percent => ValRef::Percent(value),
            ValUnit::Vw => ValRef::Vw(value),
            ValUnit::Vh => ValRef::Vh(value),
            ValUnit::VMin => ValRef::VMin(value),
            ValUnit::VMax => ValRef::VMax(value),
            ValUnit::Em => ValRef::Em(value),
            ValUnit::Rem => ValRef::Rem(value),
        }
    }

    /// Returns a `Val`'s units if it isn't auto or an expression, otherwise `None`.
    pub(crate) const fn unit(&self) -> Option<ValUnit> {
        let tag = self.tag();
        if tag < UNIT_TAG {
            None
        } else {
            Some(ValUnit::ALL[(tag - UNIT_TAG) as usize])
        }
    }

    /// Returns the `Val`'s raw inner value without its units, or `None` if it's `Auto` or an expression.
    pub(crate) const fn inner(&self) -> Option<f32> {
        if self.tag() < UNIT_TAG {
            return None;
        }
        // SAFETY: Unit tags are only written by `leaf`, together with a value.
        Some(unsafe { self.0.unit.value })
    }

    fn inner_mut(&mut self) -> Option<&mut f32> {
        if self.tag() < UNIT_TAG {
            return None;
        }
        // SAFETY: As in `inner`.
        Some(unsafe { &mut self.0.unit.value })
    }

    /// Returns the expression if the `Val` is one, otherwise `None`.
    fn calc(&self) -> Option<&Calc> {
        if self.tag() != CALC_TAG {
            return None;
        }
        // SAFETY: The calc tag is only written by `boxed`, with a pointer from `Box::into_raw`
        // that is freed only by `drop` or `into_calc`, which consume the `Val`.
        Some(unsafe { &*self.0.calc.ptr })
    }

    fn calc_mut(&mut self) -> Option<&mut Calc> {
        if self.tag() != CALC_TAG {
            return None;
        }
        // SAFETY: As in `calc`.
        Some(unsafe { &mut *self.0.calc.ptr })
    }

    /// Takes the expression out of a calc `Val`, or `None` for any other `Val`.
    pub(crate) fn into_calc(self) -> Option<Box<Calc>> {
        if self.tag() != CALC_TAG {
            return None;
        }
        // SAFETY: As in `calc`. `self` is forgotten so the box isn't freed again.
        let calc = unsafe { Box::from_raw(self.0.calc.ptr) };
        core::mem::forget(self);
        Some(calc)
    }

    /// Returns a [`UiRect`] with its `left` equal to this value,
    /// and all other fields set to `Val::ZERO`.
    ///
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).left();
    ///
    /// assert_eq!(ui_rect.left, Val::px(1.));
    /// assert_eq!(ui_rect.right, Val::ZERO);
    /// assert_eq!(ui_rect.top, Val::ZERO);
    /// assert_eq!(ui_rect.bottom, Val::ZERO);
    /// ```
    pub fn left(self) -> UiRect {
        UiRect::left(self)
    }

    /// Returns a [`UiRect`] with its `right` equal to this value,
    /// and all other fields set to `Val::ZERO`.
    ///
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).right();
    ///
    /// assert_eq!(ui_rect.left, Val::ZERO);
    /// assert_eq!(ui_rect.right, Val::px(1.));
    /// assert_eq!(ui_rect.top, Val::ZERO);
    /// assert_eq!(ui_rect.bottom, Val::ZERO);
    /// ```
    pub fn right(self) -> UiRect {
        UiRect::right(self)
    }

    /// Returns a [`UiRect`] with its `top` equal to this value,
    /// and all other fields set to `Val::ZERO`.
    ///
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).top();
    ///
    /// assert_eq!(ui_rect.left, Val::ZERO);
    /// assert_eq!(ui_rect.right, Val::ZERO);
    /// assert_eq!(ui_rect.top, Val::px(1.));
    /// assert_eq!(ui_rect.bottom, Val::ZERO);
    /// ```
    pub fn top(self) -> UiRect {
        UiRect::top(self)
    }

    /// Returns a [`UiRect`] with its `bottom` equal to this value,
    /// and all other fields set to `Val::ZERO`.
    ///
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).bottom();
    ///
    /// assert_eq!(ui_rect.left, Val::ZERO);
    /// assert_eq!(ui_rect.right, Val::ZERO);
    /// assert_eq!(ui_rect.top, Val::ZERO);
    /// assert_eq!(ui_rect.bottom, Val::px(1.));
    /// ```
    pub fn bottom(self) -> UiRect {
        UiRect::bottom(self)
    }

    /// Returns a [`UiRect`] with all its fields equal to this value.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).all();
    ///
    /// assert_eq!(ui_rect.left, Val::px(1.));
    /// assert_eq!(ui_rect.right, Val::px(1.));
    /// assert_eq!(ui_rect.top, Val::px(1.));
    /// assert_eq!(ui_rect.bottom, Val::px(1.));
    /// ```
    pub fn all(self) -> UiRect {
        UiRect::all(self)
    }

    /// Returns a [`UiRect`] with all its `left` and `right` equal to this value,
    /// and its `top` and `bottom` set to `Val::ZERO`.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).horizontal();
    ///
    /// assert_eq!(ui_rect.left, Val::px(1.));
    /// assert_eq!(ui_rect.right, Val::px(1.));
    /// assert_eq!(ui_rect.top, Val::ZERO);
    /// assert_eq!(ui_rect.bottom, Val::ZERO);
    /// ```
    pub fn horizontal(self) -> UiRect {
        UiRect::horizontal(self)
    }

    /// Returns a [`UiRect`] with all its `top` and `bottom` equal to this value,
    /// and its `left` and `right` set to `Val::ZERO`.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ui::{UiRect, Val};
    /// #
    /// let ui_rect = Val::px(1.).vertical();
    ///
    /// assert_eq!(ui_rect.left, Val::ZERO);
    /// assert_eq!(ui_rect.right, Val::ZERO);
    /// assert_eq!(ui_rect.top, Val::px(1.));
    /// assert_eq!(ui_rect.bottom, Val::px(1.));
    /// ```
    pub fn vertical(self) -> UiRect {
        UiRect::vertical(self)
    }

    /// Creates an expression that returns the minimum of two `Val`s when resolved.
    /// If both `Val`s have the same units, their minimum is returned immediately.
    pub fn min(self, other: Val) -> Val {
        self.combine(other, ReduceOp::Min)
    }

    /// Creates an expression that returns the maximum of two `Val`s when resolved.
    /// If both `Val`s have the same units, their maximum is returned immediately.
    pub fn max(self, other: Val) -> Val {
        self.combine(other, ReduceOp::Max)
    }

    /// Creates an expression that returns `self` clamped between `min` and `max` when resolved.
    /// If the bounds and self are all the same units, the clamped value of `self` is returned immediately.
    pub fn clamp(self, min: Val, max: Val) -> Val {
        let [min, value, max] = [min, self, max].map(Val::map_auto_to_zero);
        let term = |val: &Val| Some((val.unit()?, val.inner()?));
        if let (Some(lo), Some(hi)) = (term(&min), term(&max))
            && lo.0 == hi.0
        {
            // Inverted bounds resolve to the minimum, whatever the value.
            if lo.1 > hi.1 {
                return min;
            }
            if let Some(v) = term(&value)
                && v.0 == lo.0
            {
                return Val::leaf(v.0, lo.1.max(v.1.min(hi.1)));
            }
        }
        CalcOp::Clamp(min, value, max).into()
    }

    /// `Auto` is zero in expressions.
    fn map_auto_to_zero(self) -> Val {
        if matches!(self.unpack(), ValRef::Auto) {
            Val::ZERO
        } else {
            self
        }
    }
}

impl Default for Val {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Add for Val {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.combine(rhs, ReduceOp::Sum)
    }
}

impl Sub for Val {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self.combine(-rhs, ReduceOp::Sum)
    }
}

impl Mul<f32> for Val {
    type Output = Val;

    fn mul(mut self, rhs: f32) -> Self::Output {
        self *= rhs;
        self
    }
}

impl MulAssign<f32> for Val {
    fn mul_assign(&mut self, rhs: f32) {
        if matches!(self.unpack(), ValRef::Auto) {
            return;
        }
        if rhs == 0.0 {
            *self = Val::ZERO;
        } else if let Some(value) = self.inner_mut() {
            *value *= rhs;
        } else if let Some(calc) = self.calc_mut() {
            calc.0.scale(rhs);
        }
    }
}

impl Div<f32> for Val {
    type Output = Val;

    fn div(mut self, rhs: f32) -> Self::Output {
        self /= rhs;
        self
    }
}

impl DivAssign<f32> for Val {
    fn div_assign(&mut self, rhs: f32) {
        if let Some(value) = self.inner_mut() {
            *value /= rhs;
        } else if let Some(calc) = self.calc_mut() {
            calc.0.scale(rhs.recip());
        }
    }
}

impl Neg for Val {
    type Output = Val;

    fn neg(self) -> Self::Output {
        self * -1.0
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Error)]
pub enum ValArithmeticError {
    #[error("the given variant of Val is not evaluable (non-numeric)")]
    NonEvaluable,
    #[error("the given variants of Val have incompatible units")]
    IncompatibleUnits,
}

impl Val {
    /// Resolves this [`Val`] to a value in physical pixels from the given `physical_base_value` and `context`.
    ///
    /// Returns a [`ValArithmeticError::NonEvaluable`] if the [`Val`] is impossible to resolve into a concrete value.
    pub fn resolve(
        &self,
        physical_base_value: f32,
        context: &ValContext,
    ) -> Result<f32, ValArithmeticError> {
        match self.unpack() {
            ValRef::Percent(value) => Ok(physical_base_value * value / 100.0),
            ValRef::Px(value) => Ok(value * context.scale_factor),
            ValRef::Vw(value) => Ok(context.physical_size.x * value / 100.0),
            ValRef::Vh(value) => Ok(context.physical_size.y * value / 100.0),
            ValRef::VMin(value) => {
                Ok(context.physical_size.x.min(context.physical_size.y) * value / 100.0)
            }
            ValRef::VMax(value) => {
                Ok(context.physical_size.x.max(context.physical_size.y) * value / 100.0)
            }
            ValRef::Em(value) => Ok(value * context.em_size * context.scale_factor),
            ValRef::Rem(value) => Ok(value * context.rem_size * context.scale_factor),
            ValRef::Auto => Err(ValArithmeticError::NonEvaluable),
            ValRef::Calc(op) => op.0.resolve(physical_base_value, context),
        }
    }
}

impl TryFrom<Val> for FontSize {
    type Error = ValArithmeticError;

    /// Converts a [`Val`] into a [`FontSize`].
    ///
    /// [`Val::em`] becomes [`FontSize::Rem`]: em is relative to the font size of the node
    /// itself, which is the very thing being defined here, so it resolves against the root
    /// size instead to avoid the circularity.
    ///
    /// Expressions cannot be converted.
    fn try_from(val: Val) -> Result<Self, Self::Error> {
        Ok(match val.unpack() {
            ValRef::Auto => FontSize::Rem(1.),
            ValRef::Px(px) => FontSize::Px(px),
            ValRef::Percent(percent) => FontSize::Rem(percent / 100.),
            ValRef::Vw(vw) => FontSize::Vw(vw),
            ValRef::Vh(vh) => FontSize::Vh(vh),
            ValRef::VMin(vmin) => FontSize::VMin(vmin),
            ValRef::VMax(vmax) => FontSize::VMax(vmax),
            ValRef::Rem(rem) => FontSize::Rem(rem),
            ValRef::Em(em) => FontSize::Rem(em), // see comment above
            ValRef::Calc(_) => return Err(ValArithmeticError::NonEvaluable),
        })
    }
}

impl StableInterpolate for Val {
    fn interpolate_stable(&self, other: &Self, t: f32) -> Self {
        self.clone() * (1. - t) + other.clone() * t
    }
}

/// All the types that should be able to be used in the [`Val`] enum should implement this trait.
///
/// Instead of just implementing `Into<Val>` a custom trait is added.
/// This is done in order to prevent having to define a default unit, which could lead to confusion especially for newcomers.
pub trait ValNum {
    /// Called by the [`Val`] helper functions to convert the implementing type to an `f32` that can
    /// be used by [`Val`].
    fn val_num_f32(self) -> f32;
}

macro_rules! impl_to_val_num {
    ($($impl_type:ty),*$(,)?) => {
        $(
            impl ValNum for $impl_type {
                fn val_num_f32(self) -> f32 {
                    self as f32
                }
            }
        )*
    };
}

impl_to_val_num!(f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// Returns a [`Val::AUTO`] where the value is automatically determined
/// based on the context and other [`Node`](crate::Node) properties.
pub const fn auto() -> Val {
    Val::AUTO
}

/// Returns a [`Val::px`] representing a value in logical pixels.
pub fn px<T: ValNum>(value: T) -> Val {
    Val::px(value.val_num_f32())
}

/// Returns a [`Val::em`] representing a value proportional to the node's own font size, represented by an [`EmSize`] component.
pub fn em<T: ValNum>(value: T) -> Val {
    Val::em(value.val_num_f32())
}

/// Returns a [`Val::rem`] representing a value proportional to the root font size.
pub fn rem<T: ValNum>(value: T) -> Val {
    Val::rem(value.val_num_f32())
}

/// Returns a [`Val::percent`] representing a percentage of the parent node's length
/// along a specific axis.
///
/// If the UI node has no parent, the percentage is based on the window's length
/// along that axis.
///
/// Axis rules:
/// * For `flex_basis`, the percentage is relative to the main-axis length determined by the `flex_direction`.
/// * For `gap`, `min_size`, `size`, and `max_size`:
///   - `width` is relative to the parent's width.
///   - `height` is relative to the parent's height.
/// * For `margin`, `padding`, and `border` values: the percentage is relative to the parent's width.
/// * For positions, `left` and `right` are relative to the parent's width, while `bottom` and `top` are relative to the parent's height.
pub fn percent<T: ValNum>(value: T) -> Val {
    Val::percent(value.val_num_f32())
}

/// Returns a [`Val::vw`] representing a percentage of the viewport width.
pub fn vw<T: ValNum>(value: T) -> Val {
    Val::vw(value.val_num_f32())
}

/// Returns a [`Val::vh`] representing a percentage of the viewport height.
pub fn vh<T: ValNum>(value: T) -> Val {
    Val::vh(value.val_num_f32())
}

/// Returns a [`Val::vmin`] representing a percentage of the viewport's smaller dimension.
pub fn vmin<T: ValNum>(value: T) -> Val {
    Val::vmin(value.val_num_f32())
}

/// Returns a [`Val::vmax`] representing a percentage of the viewport's larger dimension.
pub fn vmax<T: ValNum>(value: T) -> Val {
    Val::vmax(value.val_num_f32())
}

/// Wrapper around an opteration on `Val`s.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(opaque, PartialEq, Debug, Clone)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    reflect(Serialize, Deserialize)
)]
#[repr(transparent)]
pub struct Calc(pub(crate) CalcOp);

/// Stores a max of 3 `Val`s inline so that the sizes of `CalcOp`s variants holding `CalcTerms`
/// matches `Clamp`.
pub(crate) type CalcTerms = SmallVec<[Val; 3]>;

/// An operation performed on two or more `Val`s.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(PartialEq, Debug, Clone)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    reflect(Serialize, Deserialize)
)]
#[repr(align(16))]
pub(crate) enum CalcOp {
    /// The sum of the terms.
    Sum(CalcTerms),
    /// The smallest of the terms.
    Min(CalcTerms),
    /// The largest of the terms.
    Max(CalcTerms),
    /// Clamp the second `Val` between the first and third.
    Clamp(Val, Val, Val),
}

impl CalcOp {
    /// The operation applied to the terms of a `Sum`, `Min` or `Max`.
    fn op(&self) -> Option<ReduceOp> {
        match self {
            Self::Sum(_) => Some(ReduceOp::Sum),
            Self::Min(_) => Some(ReduceOp::Min),
            Self::Max(_) => Some(ReduceOp::Max),
            Self::Clamp(..) => None,
        }
    }

    /// Resolve a `CalcOp` to a value in physical pixels.
    pub fn resolve(
        &self,
        physical_base_value: f32,
        context: &ValContext,
    ) -> Result<f32, ValArithmeticError> {
        let resolve = |val: &Val| val.resolve(physical_base_value, context);
        let reduce = |terms: &[Val], init: f32, op: fn(f32, f32) -> f32| {
            terms
                .iter()
                .map(resolve)
                .try_fold(init, |acc, value| Ok(op(acc, value?)))
        };
        match self {
            Self::Sum(terms) => reduce(terms, 0.0, |a, b| a + b),
            Self::Min(terms) => reduce(terms, f32::INFINITY, f32::min),
            Self::Max(terms) => reduce(terms, f32::NEG_INFINITY, f32::max),
            Self::Clamp(min, value, max) => {
                Ok(resolve(min)?.max(resolve(value)?.min(resolve(max)?)))
            }
        }
    }

    /// Multiply an operation by a scalar.
    /// If the scale is negative, the direction of the operation is changed.
    /// If min is greater than max, clamp always returns min and therefore negation always returns -min.
    pub(crate) fn scale(&mut self, k: f32) {
        use core::mem::take;
        match self {
            Self::Sum(terms) | Self::Min(terms) | Self::Max(terms) => {
                for term in terms {
                    *term *= k;
                }
            }
            Self::Clamp(min, value, max) => {
                *min *= k;
                *value *= k;
                *max *= k;
            }
        }
        if k < 0.0 {
            match self {
                Self::Min(terms) => *self = Self::Max(take(terms)),
                Self::Max(terms) => *self = Self::Min(take(terms)),
                Self::Clamp(min, value, max) => {
                    match (min.unit(), min.inner(), max.unit(), max.inner()) {
                        (Some(a), Some(lo), Some(b), Some(hi)) if a == b && lo >= hi => {
                            core::mem::swap(min, max);
                        }
                        _ => {
                            *self = Self::Min(smallvec![
                                take(min),
                                Self::Max(smallvec![take(value), take(max)]).into(),
                            ]);
                        }
                    }
                }
                Self::Sum(_) => {}
            }
        }
    }
}

impl From<CalcOp> for Val {
    fn from(op: CalcOp) -> Self {
        Self::boxed(Box::new(Calc(op)))
    }
}

/// The unit of a numeric [`Val`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Reflect)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub(crate) enum ValUnit {
    /// Use `Val::px` units.
    Px,
    /// Use `Val::percent` units.
    Percent,
    /// Use `Val::vw` units.
    Vw,
    /// Use `Val::vh` units.
    Vh,
    /// Use `Val::vmin` units.
    VMin,
    /// Use `Val::vmax` units.
    VMax,
    /// Use `Val::em` units.
    Em,
    /// Use `Val::rem` units.
    Rem,
}

impl ValUnit {
    /// Every unit, in discriminant order.
    pub const ALL: [Self; 8] = [
        Self::Px,
        Self::Percent,
        Self::Vw,
        Self::Vh,
        Self::VMin,
        Self::VMax,
        Self::Em,
        Self::Rem,
    ];

    /// Resolve a `ValUnit` and `f32` length to physical coords except for percentage values.
    /// `Percentage` values depend on the size of the parent, and can't be resolved except during layout.
    #[inline]
    pub(crate) fn resolve_physical_except_percent(self, value: f32, context: &ValContext) -> Val {
        match self {
            ValUnit::Percent => Val::percent(value),
            ValUnit::Px => Val::px(value * context.scale_factor),
            ValUnit::Vw => Val::px(context.physical_size.x * value / 100.0),
            ValUnit::Vh => Val::px(context.physical_size.y * value / 100.0),
            ValUnit::VMin => Val::px(context.physical_size.min_element() * value / 100.0),
            ValUnit::VMax => Val::px(context.physical_size.max_element() * value / 100.0),
            ValUnit::Em => Val::px(value * context.em_size * context.scale_factor),
            ValUnit::Rem => Val::px(value * context.rem_size * context.scale_factor),
        }
    }
}

/// The operation a [`CalcOp::Sum`], [`CalcOp::Min`] or [`CalcOp::Max`] applies to its terms.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReduceOp {
    Sum,
    Min,
    Max,
}

impl ReduceOp {
    /// Apply the operation to unitless values
    pub(crate) fn apply(self) -> fn(f32, f32) -> f32 {
        match self {
            Self::Sum => |a, b| a + b,
            Self::Min => f32::min,
            Self::Max => f32::max,
        }
    }

    /// The [`Val`] applying this operation to `terms`.
    fn calc(self, terms: CalcTerms) -> Val {
        match self {
            Self::Sum => CalcOp::Sum(terms),
            Self::Min => CalcOp::Min(terms),
            Self::Max => CalcOp::Max(terms),
        }
        .into()
    }
}

impl Val {
    /// Combines two `Val`s with an operation. Terms with the same unit are folded together and
    /// nested expressions with the same operation are flattened, so the result is a unit value
    /// when it can be, and otherwise a [`Calc`] with the unit terms first.
    pub(crate) fn combine(self, rhs: Val, op: ReduceOp) -> Val {
        let mut leaves = [None; ValUnit::ALL.len()];
        let mut rest: SmallVec<[Val; 3]> = SmallVec::new();
        self.push_terms(op, &mut leaves, &mut rest);
        rhs.push_terms(op, &mut leaves, &mut rest);
        let mut terms: CalcTerms = ValUnit::ALL
            .into_iter()
            .zip(leaves)
            .filter_map(|(unit, value)| Some(Val::leaf(unit, value?)))
            // Zero terms don't change a sum.
            .filter(|leaf| op != ReduceOp::Sum || leaf.inner() != Some(0.0))
            .collect();
        terms.extend(rest);
        match terms.len() {
            // All terms eliminated, return 0
            0 => Val::ZERO,
            // There is only one `Val` term in the list, so remove it from the list and return it
            1 => terms.remove(0),
            // Return a `Val` wrapping the remaining terms in a `CalcOp`
            _ => op.calc(terms),
        }
    }

    /// Folds the unit terms of `self` under `op` into `leaves`, indexed by unit, and appends any
    /// other terms to `rest`.
    fn push_terms(
        self,
        op: ReduceOp,
        leaves: &mut [Option<f32>; ValUnit::ALL.len()],
        rest: &mut SmallVec<[Val; 3]>,
    ) {
        let val = self.map_auto_to_zero();
        if val.calc().is_some_and(|calc| calc.0.op() == Some(op)) {
            let Calc(calc) = *val.into_calc().unwrap();
            if let CalcOp::Sum(terms) | CalcOp::Min(terms) | CalcOp::Max(terms) = calc {
                for term in terms {
                    term.push_terms(op, leaves, rest);
                }
            }
        } else {
            match (val.unit(), val.inner()) {
                (Some(unit), Some(value)) => {
                    let leaf = &mut leaves[unit as usize];
                    *leaf = Some(leaf.map_or(value, |folded| op.apply()(folded, value)));
                }
                _ => rest.push(val),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::vec2;

    use super::*;

    #[test]
    fn val_evaluate() {
        let size = 250.;
        let viewport_size = vec2(1000., 500.);
        let em_size = EmSize(10.);
        let rem_size = RemSize(20.);

        assert_eq!(
            Val::percent(80.)
                .resolve(size, &ValContext::new(1., viewport_size, em_size, rem_size))
                .unwrap(),
            size * 0.8
        );
        // Check em and rem
        assert_eq!(
            Val::em(2.)
                .resolve(size, &ValContext::new(1., viewport_size, em_size, rem_size))
                .unwrap(),
            em_size.0 * 2.
        );
        assert_eq!(
            Val::rem(5.)
                .resolve(size, &ValContext::new(1., viewport_size, em_size, rem_size))
                .unwrap(),
            rem_size.0 * 5.
        );
        // Check scale_factor is considered
        assert_eq!(
            Val::em(2.)
                .resolve(size, &ValContext::new(2., viewport_size, em_size, rem_size))
                .unwrap(),
            em_size.0 * 2. * 2.
        );
        assert_eq!(
            Val::rem(5.)
                .resolve(size, &ValContext::new(2., viewport_size, em_size, rem_size))
                .unwrap(),
            rem_size.0 * 5. * 2.
        );
    }

    #[test]
    fn val_resolve_px() {
        let size = 250.;
        let viewport_size = vec2(1000., 500.);

        let result = Val::px(10.)
            .resolve(
                size,
                &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0)),
            )
            .unwrap();
        assert_eq!(result, 10.);

        let result = Val::px(10.)
            .resolve(
                size,
                &ValContext::new(3., viewport_size, EmSize(20.0), RemSize(20.0)),
            )
            .unwrap();
        assert_eq!(result, 30.);
        let result = Val::px(10.)
            .resolve(
                size,
                &ValContext::new(0.25, viewport_size, EmSize(20.0), RemSize(20.0)),
            )
            .unwrap();
        assert_eq!(result, 2.5);

        let result = Val::em(1.)
            .resolve(
                size,
                &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0)),
            )
            .unwrap();
        assert_eq!(result, 20.0);
        let result = Val::rem(1.)
            .resolve(
                size,
                &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0)),
            )
            .unwrap();
        assert_eq!(result, 20.0);
    }

    #[cfg(not(all(target_pointer_width = "64", target_endian = "big")))]
    #[test]
    fn val_is_eight_bytes() {
        assert_eq!(size_of::<Val>(), 8);
    }

    #[test]
    fn val_resolve_viewport_coords() {
        let size = 250.;
        let viewport_size = vec2(500., 500.);

        for value in (-10..10).map(|value| value as f32) {
            // for a square viewport there should be no difference between `Vw` and `Vh` and between `Vmin` and `Vmax`.
            assert_eq!(
                Val::vw(value).resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                ),
                Val::vh(value).resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
            );
            assert_eq!(
                Val::vmin(value).resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                ),
                Val::vmax(value).resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
            );
            assert_eq!(
                Val::vmin(value).resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                ),
                Val::vw(value).resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
            );
        }

        let viewport_size = vec2(1000., 500.);
        assert_eq!(
            Val::vw(100.)
                .resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
                .unwrap(),
            1000.
        );
        assert_eq!(
            Val::vh(100.)
                .resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
                .unwrap(),
            500.
        );
        assert_eq!(
            Val::vw(60.)
                .resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
                .unwrap(),
            600.
        );
        assert_eq!(
            Val::vh(40.)
                .resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
                .unwrap(),
            200.
        );
        assert_eq!(
            Val::vmin(50.)
                .resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
                .unwrap(),
            250.
        );
        assert_eq!(
            Val::vmax(75.)
                .resolve(
                    size,
                    &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0))
                )
                .unwrap(),
            750.
        );
    }

    #[test]
    fn val_auto_is_non_evaluable() {
        let size = 250.;
        let viewport_size = vec2(1000., 500.);
        let resolve_auto = Val::AUTO.resolve(
            size,
            &ValContext::new(1., viewport_size, EmSize(20.0), RemSize(20.0)),
        );

        assert_eq!(resolve_auto, Err(ValArithmeticError::NonEvaluable));
    }

    #[test]
    fn val_str_parse() {
        assert_eq!("auto".parse::<Val>(), Ok(Val::AUTO));
        assert_eq!("Auto".parse::<Val>(), Ok(Val::AUTO));
        assert_eq!("AUTO".parse::<Val>(), Ok(Val::AUTO));

        assert_eq!("3px".parse::<Val>(), Ok(Val::px(3.)));
        assert_eq!("3 px".parse::<Val>(), Ok(Val::px(3.)));
        assert_eq!("3.5px".parse::<Val>(), Ok(Val::px(3.5)));
        assert_eq!("-3px".parse::<Val>(), Ok(Val::px(-3.)));
        assert_eq!("3.5 PX".parse::<Val>(), Ok(Val::px(3.5)));

        assert_eq!("3%".parse::<Val>(), Ok(Val::percent(3.)));
        assert_eq!("3 %".parse::<Val>(), Ok(Val::percent(3.)));
        assert_eq!("3.5%".parse::<Val>(), Ok(Val::percent(3.5)));
        assert_eq!("-3%".parse::<Val>(), Ok(Val::percent(-3.)));

        assert_eq!("3vw".parse::<Val>(), Ok(Val::vw(3.)));
        assert_eq!("3 vw".parse::<Val>(), Ok(Val::vw(3.)));
        assert_eq!("3.5vw".parse::<Val>(), Ok(Val::vw(3.5)));
        assert_eq!("-3vw".parse::<Val>(), Ok(Val::vw(-3.)));
        assert_eq!("3.5 VW".parse::<Val>(), Ok(Val::vw(3.5)));

        assert_eq!("3vh".parse::<Val>(), Ok(Val::vh(3.)));
        assert_eq!("3 vh".parse::<Val>(), Ok(Val::vh(3.)));
        assert_eq!("3.5vh".parse::<Val>(), Ok(Val::vh(3.5)));
        assert_eq!("-3vh".parse::<Val>(), Ok(Val::vh(-3.)));
        assert_eq!("3.5 VH".parse::<Val>(), Ok(Val::vh(3.5)));

        assert_eq!("3vmin".parse::<Val>(), Ok(Val::vmin(3.)));
        assert_eq!("3 vmin".parse::<Val>(), Ok(Val::vmin(3.)));
        assert_eq!("3.5vmin".parse::<Val>(), Ok(Val::vmin(3.5)));
        assert_eq!("-3vmin".parse::<Val>(), Ok(Val::vmin(-3.)));
        assert_eq!("3.5 VMIN".parse::<Val>(), Ok(Val::vmin(3.5)));

        assert_eq!("3vmax".parse::<Val>(), Ok(Val::vmax(3.)));
        assert_eq!("3 vmax".parse::<Val>(), Ok(Val::vmax(3.)));
        assert_eq!("3.5vmax".parse::<Val>(), Ok(Val::vmax(3.5)));
        assert_eq!("-3vmax".parse::<Val>(), Ok(Val::vmax(-3.)));
        assert_eq!("3.5 VMAX".parse::<Val>(), Ok(Val::vmax(3.5)));

        assert_eq!("3em".parse::<Val>(), Ok(Val::em(3.)));
        assert_eq!("3rem".parse::<Val>(), Ok(Val::rem(3.)));

        assert_eq!("".parse::<Val>(), Err(ValParseError::UnitMissing));
        assert_eq!(
            "hello world".parse::<Val>(),
            Err(ValParseError::ValueMissing)
        );
        assert_eq!("3".parse::<Val>(), Err(ValParseError::UnitMissing));
        assert_eq!("3.5".parse::<Val>(), Err(ValParseError::UnitMissing));
        assert_eq!("3pxx".parse::<Val>(), Err(ValParseError::InvalidUnit));
        assert_eq!("3.5pxx".parse::<Val>(), Err(ValParseError::InvalidUnit));
        assert_eq!("3-3px".parse::<Val>(), Err(ValParseError::InvalidValue));
        assert_eq!("3.5-3px".parse::<Val>(), Err(ValParseError::InvalidValue));
    }

    #[test]
    fn default_val_equals_const_default_val() {
        assert_eq!(Val::default(), Val::DEFAULT);
    }

    #[test]
    fn val_constructor_fns_return_correct_val_variant() {
        assert_eq!(auto(), Val::AUTO);
        assert_eq!(px(0.0), Val::px(0.0));
        assert_eq!(percent(0.0), Val::percent(0.0));
        assert_eq!(vw(0.0), Val::vw(0.0));
        assert_eq!(vh(0.0), Val::vh(0.0));
        assert_eq!(vmin(0.0), Val::vmin(0.0));
        assert_eq!(vmax(0.0), Val::vmax(0.0));
    }

    #[test]
    fn sums_fold() {
        let sum = Val::px(10.0) + Val::percent(50.0);
        assert!(matches!(sum.unpack(), ValRef::Calc(_)), "got {sum:?}");
        assert_eq!(Val::percent(50.0) + Val::px(10.0), sum);
        assert_eq!(Val::px(1.0) + Val::px(2.0), Val::px(3.0));
        assert_eq!(sum.clone() - Val::px(10.0), Val::percent(50.0));
        assert_eq!(Val::px(0.0) + Val::percent(0.0), Val::ZERO);
        assert_ne!(sum, Val::px(10.0));
        assert_eq!(
            (Val::px(1.0) + Val::percent(2.0)) + (Val::px(3.0) + Val::em(4.0)),
            Val::em(4.0) + Val::px(4.0) + Val::percent(2.0)
        );
    }

    #[test]
    fn min_and_max_fold() {
        assert_eq!(Val::px(10.0).min(Val::px(20.0)), Val::px(10.0));
        assert_eq!(Val::px(10.0).max(Val::px(20.0)), Val::px(20.0));
        let min = Val::px(10.0).min(Val::percent(50.0));
        assert!(matches!(min.unpack(), ValRef::Calc(_)));
        assert_eq!(Val::percent(50.0).min(Val::px(10.0)), min);
        assert_eq!(-min.clone(), Val::px(-10.0).max(Val::percent(-50.0)));
        assert!(matches!(
            min.clone().max(Val::px(5.0)).unpack(),
            ValRef::Calc(_)
        ));
        let resolve = |value: &Val| {
            value.resolve(
                100.0,
                &ValContext::new(1.0, Vec2::ZERO, EmSize(0.0), RemSize(0.0)),
            )
        };
        assert_eq!(resolve(&min), Ok(10.0));
        assert_eq!(resolve(&Val::px(10.0).max(Val::percent(50.0))), Ok(50.0));
        assert_eq!(resolve(&(min * -2.0)), Ok(-20.0));
    }

    #[test]
    fn negating_min_and_max_negates_their_results() {
        assert_eq!(-Val::px(10.0).max(Val::px(20.0)), Val::px(-20.0));
        assert_eq!(-Val::px(10.0).min(Val::px(20.0)), Val::px(-10.0));
        let resolve = |value: &Val, base: f32| {
            value.resolve(
                base,
                &ValContext::new(1.0, Vec2::ZERO, EmSize(0.0), RemSize(0.0)),
            )
        };
        for expr in [
            Val::px(10.0).max(Val::percent(50.0)),
            Val::px(10.0).min(Val::percent(50.0)),
        ] {
            for base in [0.0, 20.0, 100.0] {
                let expected = resolve(&expr, base).unwrap();
                assert_eq!(
                    resolve(&-expr.clone(), base),
                    Ok(-expected),
                    "{expr:?} at {base}"
                );
                assert_eq!(
                    resolve(&(expr.clone() * -2.0), base),
                    Ok(-2.0 * expected),
                    "{expr:?} at {base}"
                );
            }
        }
    }

    #[test]
    fn negating_a_clamp_negates_its_result() {
        let clamp = Val::percent(50.0).clamp(Val::px(100.0), Val::percent(20.0));
        let resolve = |value: &Val, base: f32| {
            value.resolve(
                base,
                &ValContext::new(1.0, Vec2::ZERO, EmSize(0.0), RemSize(0.0)),
            )
        };
        for base in [50.0, 100.0, 150.0, 300.0, 1000.0] {
            let expected = resolve(&clamp, base).unwrap();
            assert_eq!(resolve(&-clamp.clone(), base), Ok(-expected), "base {base}");
            assert_eq!(
                resolve(&(clamp.clone() * -2.0), base),
                Ok(-2.0 * expected),
                "base {base}"
            );
            assert_eq!(
                resolve(&(Val::ZERO - clamp.clone()), base),
                Ok(-expected),
                "base {base}"
            );
        }
    }

    #[test]
    fn clamp_folds() {
        let clamp = Val::percent(80.0).clamp(Val::px(100.0), Val::px(300.0));
        assert!(matches!(clamp.unpack(), ValRef::Calc(_)), "got {clamp:?}");
        assert_eq!(
            Val::percent(80.0).clamp(Val::px(300.0), Val::px(100.0)),
            Val::px(300.0)
        );
        let resolve = |value: &Val, base: f32| {
            value.resolve(
                base,
                &ValContext::new(1.0, Vec2::ZERO, EmSize(0.0), RemSize(0.0)),
            )
        };
        assert_eq!(resolve(&clamp, 100.0), Ok(100.0));
        assert_eq!(resolve(&clamp, 250.0), Ok(200.0));
        assert_eq!(resolve(&clamp, 1000.0), Ok(300.0));
        assert_eq!(resolve(&-clamp.clone(), 250.0), Ok(-200.0));
        // Same-unit bounds are swapped in place rather than rebuilt as `min`/`max`.
        assert_eq!(
            -clamp.clone(),
            Val::percent(-80.0).clamp(Val::px(-300.0), Val::px(-100.0))
        );
        assert_eq!(resolve(&(clamp.clone() * -2.0), 250.0), Ok(-400.0));
        assert_eq!(
            clamp.clone() * 2.0,
            Val::percent(160.0).clamp(Val::px(200.0), Val::px(600.0))
        );
        // Interpolation is arithmetic, so the halfway value resolves to the mean of the two.
        let other = Val::percent(0.0).clamp(Val::px(0.0), Val::px(100.0));
        assert_eq!(
            resolve(&clamp.interpolate_stable(&other, 0.5), 250.0),
            Ok(100.0)
        );
        assert!(matches!(
            (Val::percent(80.0) + Val::px(1.0))
                .clamp(Val::px(100.0), Val::px(300.0))
                .unpack(),
            ValRef::Calc(_)
        ));
    }

    #[test]
    fn terms_interpolate_term_wise() {
        let a = Val::px(0.0) + Val::percent(0.0);
        let b = Val::px(10.0) + Val::percent(100.0);
        assert_eq!(
            a.interpolate_stable(&b, 0.5),
            Val::px(5.0) + Val::percent(50.0)
        );
        assert_eq!(
            a.interpolate_stable(&(Val::px(1.0) + Val::em(1.0)), 0.5),
            Val::px(0.5) + Val::em(0.5)
        );
    }

    #[test]
    fn resolve_expressions() {
        let value =
            Val::percent(50.0) - Val::px(10.0) + Val::em(2.0) + Val::rem(1.0) + Val::vw(1.0);
        let resolve = |value: &Val| {
            value.resolve(
                400.0,
                &ValContext::new(2.0, Vec2::new(1000.0, 500.0), EmSize(12.0), RemSize(20.0)),
            )
        };
        assert_eq!(resolve(&value), Ok(278.0));
        assert_eq!(resolve(&(value.clone() * 2.0)), Ok(556.0));
        assert_eq!(resolve(&(value.clone() / 2.0)), Ok(139.0));
        assert_eq!(resolve(&-value.clone()), Ok(-278.0));
        assert_eq!(
            resolve(&CalcOp::Min(smallvec![value.clone(), Val::px(100.0)]).into()),
            Ok(200.0)
        );
        assert_eq!(
            resolve(&CalcOp::Max(smallvec![value.clone(), Val::px(200.0)]).into()),
            Ok(400.0)
        );
        assert_eq!(
            resolve(&CalcOp::Clamp(Val::px(100.0), value.clone(), Val::px(125.0)).into()),
            Ok(250.0)
        );
        assert_eq!(
            resolve(&CalcOp::Clamp(Val::px(200.0), value.clone(), Val::px(100.0)).into()),
            Ok(400.0)
        );
        let mut scaled = value;
        scaled *= 2.0;
        scaled /= 4.0;
        assert_eq!(resolve(&scaled), Ok(139.0));
        assert_eq!(resolve(&(Val::AUTO + Val::px(1.0))), Ok(2.0));
        assert_eq!(resolve(&Val::AUTO), Err(ValArithmeticError::NonEvaluable));
    }

    #[test]
    fn negative_scale_swaps_min_and_max() {
        let resolve = |value: Val| {
            value.resolve(
                0.0,
                &ValContext::new(1.0, Vec2::ZERO, EmSize(0.0), RemSize(0.0)),
            )
        };
        let (a, b) = (Val::px(10.0), Val::px(20.0));
        assert_eq!(
            resolve(-Val::from(CalcOp::Min(smallvec![a.clone(), b.clone()]))),
            Ok(-10.0)
        );
        assert_eq!(
            resolve(-Val::from(CalcOp::Max(smallvec![a.clone(), b.clone()]))),
            Ok(-20.0)
        );
        assert_eq!(
            resolve(-Val::from(CalcOp::Clamp(
                a.clone(),
                Val::px(15.0),
                b.clone()
            ))),
            Ok(-15.0)
        );
        assert_eq!(
            resolve(-Val::from(CalcOp::Clamp(b, Val::px(15.0), a))),
            Ok(-20.0)
        );
    }

    #[test]
    fn repeated_scaling_does_not_grow_the_tree() {
        let mut value = Val::px(1.0) + Val::px(2.0);
        for _ in 0..100_000 {
            value *= 2.0;
            value /= 2.0;
        }
        assert_eq!(value, Val::px(1.0) + Val::px(2.0));
    }

    #[test]
    fn clone_and_reflection_preserve_expressions() {
        use bevy_reflect::{FromReflect, PartialReflect, TypeRegistry};

        let value = Val::percent(100.0) - Val::px(24.0);
        let mut cloned = value.clone();
        assert_eq!(value, cloned);
        cloned *= 2.0;
        assert_ne!(value, cloned);
        assert_eq!(value, Val::percent(100.0) - Val::px(24.0));
        let mut registry = TypeRegistry::default();
        registry.register::<Val>();
        let dynamic = value.to_dynamic().unwrap();
        assert_eq!(Val::from_reflect(dynamic.as_ref()), Some(value));
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn expressions_round_trip_through_serde() {
        let values = [
            Val::percent(50.0) - Val::px(10.0),
            Val::px(10.0).min(Val::percent(50.0)),
            Val::percent(80.0).clamp(Val::px(100.0), Val::px(300.0)),
            CalcOp::Min(smallvec![Val::em(1.0) + Val::percent(10.0), Val::rem(1.0)]).into(),
        ];
        for value in values {
            let ron = ron::to_string(&value).unwrap();
            assert_eq!(ron::from_str::<Val>(&ron).unwrap(), value, "{ron}");
        }
    }

    #[test]
    fn auto_is_zero_in_expressions() {
        assert_eq!(Val::AUTO + Val::AUTO, Val::ZERO);
        assert_eq!(Val::AUTO - Val::AUTO, Val::ZERO);
        assert_eq!(Val::px(20.0) + Val::AUTO, Val::px(20.0));
        assert_eq!(Val::percent(10.0) + Val::AUTO, Val::percent(10.0));
        assert_eq!(Val::AUTO.min(Val::px(10.0)), Val::ZERO);
        assert_eq!(Val::AUTO.max(Val::px(10.0)), Val::px(10.0));
        assert_eq!(Val::px(5.0).clamp(Val::AUTO, Val::px(10.0)), Val::px(5.0));
        assert_eq!(Val::AUTO.clamp(Val::px(5.0), Val::px(10.0)), Val::px(5.0));
        assert_eq!(
            (Val::percent(10.0) + Val::AUTO)
                .resolve(100.0, &ValContext::default())
                .unwrap(),
            10.0
        );
    }

    #[test]
    fn terms_merge_across_nested_expressions() {
        let max = Val::px(10.0).max(Val::vh(20.0));
        let sum = Val::vw(10.0) + max.clone() + Val::vw(20.0);
        assert_eq!(sum, max.clone() + Val::vw(30.0));
        assert!(
            matches!(sum.unpack(), ValRef::Calc(calc) if matches!(&calc.0, CalcOp::Sum(terms) if terms.len() == 2)),
            "got {sum:?}"
        );
        // Nested sums are flattened and same-unit terms are folded away.
        let sum = (Val::vw(10.0) + max.clone()) + (max.clone() - Val::vw(10.0));
        assert_eq!(sum, CalcOp::Sum(smallvec![max.clone(), max.clone()]).into());
        // Minimums behave the same way.
        let min = Val::px(1.0).min(max.clone()).min(Val::px(2.0));
        assert_eq!(min, CalcOp::Min(smallvec![Val::px(1.0), max]).into());
    }

    #[test]
    fn font_size_rejects_expressions() {
        assert_eq!(FontSize::try_from(Val::rem(2.0)), Ok(FontSize::Rem(2.0)));
        assert_eq!(
            FontSize::try_from(Val::percent(100.0) - Val::px(24.0)),
            Err(ValArithmeticError::NonEvaluable)
        );
    }

    #[test]
    fn inverted_clamp_negates_its_resolved_value() {
        let value = Val::from(CalcOp::Clamp(
            Val::px(100.0),
            Val::percent(50.0),
            Val::px(20.0),
        ));
        let resolve = |value: Val| {
            value
                .resolve(
                    100.0,
                    &ValContext::new(
                        1.0,
                        Default::default(),
                        Default::default(),
                        Default::default(),
                    ),
                )
                .unwrap()
        };
        assert_eq!(resolve(value.clone()), 100.0);
        assert_eq!(resolve(-value), -100.0);
    }

    #[test]
    fn extrapolated_clamp_negates_its_resolved_value() {
        let a = Val::percent(50.0).clamp(Val::px(0.0), Val::px(100.0));
        let b = Val::percent(50.0).clamp(Val::px(100.0), Val::px(100.0));
        let value = StableInterpolate::interpolate_stable(&a, &b, 2.0);
        let resolve = |value: Val| {
            value
                .resolve(
                    100.0,
                    &ValContext::new(
                        1.0,
                        Default::default(),
                        Default::default(),
                        Default::default(),
                    ),
                )
                .unwrap()
        };
        // `-a + 2b`: `a` resolves to 50 and `b` to 100.
        assert_eq!(resolve(value.clone()), 150.0);
        assert_eq!(resolve(-value), -150.0);
    }
}

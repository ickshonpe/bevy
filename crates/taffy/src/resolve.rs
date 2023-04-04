//! Helper trait to calculate dimensions during layout resolution

use crate::prelude::{Dimension, LengthPercentage, LengthPercentageAuto, Rect, Size};
use crate::style_helpers::TaffyZero;

/// Trait to encapsulate behaviour where we need to resolve from a
/// potentially context-dependent size or dimension into
/// a context-independent size or dimension.
///
/// Will return a `None` if it unable to resolve.
pub(crate) trait MaybeResolve<In, Out> {
    /// Resolve a dimension that might be dependent on a context, with `None` as fallback value
    fn maybe_resolve(self, context: In) -> Out;
}

/// Trait to encapsulate behaviour where we need to resolve from a
/// potentially context-dependent size or dimension into
/// a context-independent size or dimension.
///
/// Will return a default value if it unable to resolve.
pub(crate) trait ResolveOrZero<TContext, TOutput: TaffyZero> {
    /// Resolve a dimension that might be dependent on a context, with a default fallback value
    fn resolve_or_zero(self, context: TContext) -> TOutput;
}

impl MaybeResolve<Option<f32>, Option<f32>> for LengthPercentage {
    /// Converts the given [`LengthPercentage`] into a concrete value of points
    /// Can return `None`
    fn maybe_resolve(self, context: Option<f32>) -> Option<f32> {
        match self {
            LengthPercentage::Points(points) => Some(points),
            LengthPercentage::Percent(percent) => context.map(|dim| dim * percent),
        }
    }
}

impl MaybeResolve<Option<f32>, Option<f32>> for LengthPercentageAuto {
    /// Converts the given [`LengthPercentageAuto`] into a concrete value of points
    /// Can return `None`
    fn maybe_resolve(self, context: Option<f32>) -> Option<f32> {
        match self {
            LengthPercentageAuto::Points(points) => Some(points),
            LengthPercentageAuto::Percent(percent) => context.map(|dim| dim * percent),
            LengthPercentageAuto::Auto => None,
        }
    }
}

impl MaybeResolve<Option<f32>, Option<f32>> for Dimension {
    /// Converts the given [`Dimension`] into a concrete value of points
    ///
    /// Can return `None`
    fn maybe_resolve(self, context: Option<f32>) -> Option<f32> {
        match self {
            Dimension::Points(points) => Some(points),
            Dimension::Percent(percent) => context.map(|dim| dim * percent),
            Dimension::Auto => None,
        }
    }
}

// Generic implementation of MaybeResolve for f32 context where MaybeResolve is implemented
// for Option<f32> context
impl<T: MaybeResolve<Option<f32>, Option<f32>>> MaybeResolve<f32, Option<f32>> for T {
    /// Converts the given MaybeResolve value into a concrete value of points
    /// Can return `None`
    fn maybe_resolve(self, context: f32) -> Option<f32> {
        self.maybe_resolve(Some(context))
    }
}

// Generic MaybeResolve for Size
impl<In, Out, T: MaybeResolve<In, Out>> MaybeResolve<Size<In>, Size<Out>> for Size<T> {
    /// Converts any `parent`-relative values for size into an absolute size
    fn maybe_resolve(self, context: Size<In>) -> Size<Out> {
        Size { width: self.width.maybe_resolve(context.width), height: self.height.maybe_resolve(context.height) }
    }
}

impl ResolveOrZero<Option<f32>, f32> for LengthPercentage {
    /// Will return a default value of result is evaluated to `None`
    fn resolve_or_zero(self, context: Option<f32>) -> f32 {
        self.maybe_resolve(context).unwrap_or(0.0)
    }
}

impl ResolveOrZero<Option<f32>, f32> for LengthPercentageAuto {
    /// Will return a default value of result is evaluated to `None`
    fn resolve_or_zero(self, context: Option<f32>) -> f32 {
        self.maybe_resolve(context).unwrap_or(0.0)
    }
}

impl ResolveOrZero<Option<f32>, f32> for Dimension {
    /// Will return a default value of result is evaluated to `None`
    fn resolve_or_zero(self, context: Option<f32>) -> f32 {
        self.maybe_resolve(context).unwrap_or(0.0)
    }
}

// Generic ResolveOrZero for Size
impl<In, Out: TaffyZero, T: ResolveOrZero<In, Out>> ResolveOrZero<Size<In>, Size<Out>> for Size<T> {
    /// Converts any `parent`-relative values for size into an absolute size
    fn resolve_or_zero(self, context: Size<In>) -> Size<Out> {
        Size { width: self.width.resolve_or_zero(context.width), height: self.height.resolve_or_zero(context.height) }
    }
}

// Generic ResolveOrZero for resolving Rect against Size
impl<In: Copy, Out: TaffyZero, T: ResolveOrZero<In, Out>> ResolveOrZero<Size<In>, Rect<Out>> for Rect<T> {
    /// Converts any `parent`-relative values for Rect into an absolute Rect
    fn resolve_or_zero(self, context: Size<In>) -> Rect<Out> {
        Rect {
            left: self.left.resolve_or_zero(context.width),
            right: self.right.resolve_or_zero(context.width),
            top: self.top.resolve_or_zero(context.height),
            bottom: self.bottom.resolve_or_zero(context.height),
        }
    }
}

// Generic ResolveOrZero for resolving Rect against Option
impl<Out: TaffyZero, T: ResolveOrZero<Option<f32>, Out>> ResolveOrZero<Option<f32>, Rect<Out>> for Rect<T> {
    /// Converts any `parent`-relative values for Rect into an absolute Rect
    fn resolve_or_zero(self, context: Option<f32>) -> Rect<Out> {
        Rect {
            left: self.left.resolve_or_zero(context),
            right: self.right.resolve_or_zero(context),
            top: self.top.resolve_or_zero(context),
            bottom: self.bottom.resolve_or_zero(context),
        }
    }
}

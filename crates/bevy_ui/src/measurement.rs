use bevy_ecs::prelude::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_math::Vec2;
use bevy_reflect::Reflect;
use std::{fmt::Formatter, sync::Arc};
pub use taffy::style::AvailableSpace;

impl std::fmt::Debug for ContentSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentSize").finish()
    }
}

/// A `Measure` is used to compute the size of a ui node
/// when the size of that node is based on its content.
pub trait Measure: Send + Sync + 'static {
    /// Calculate the size of the node given the constraints.
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2;
}

/// A `FixedMeasure` is a `Measure` that ignores all constraints and
/// always returns the same size.
#[derive(Default, Clone)]
pub struct FixedMeasure {
    pub size: Vec2,
}

impl Measure for FixedMeasure {
    fn measure(
        &self,
        _: Option<f32>,
        _: Option<f32>,
        _: AvailableSpace,
        _: AvailableSpace,
    ) -> Vec2 {
        self.size
    }
}

/// A node with a `ContentSize` component is a node where its size
/// is based on its content.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ContentSize {
    /// The `Measure` used to compute the intrinsic size
    #[reflect(ignore)]
    pub measure: Option<Arc<Box<dyn Measure>>>,
}

impl ContentSize {
    /// Set a `Measure` for this function
    pub fn set(&mut self, measure: impl Measure) {
        self.measure = Some(Arc::new(Box::new(measure)));
    }

    /// Call the `Measure` manually, if present
    pub fn measure(
        &mut self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Option<Vec2> {
        self.measure.as_ref().map(|inner_measure| {
            inner_measure.measure(width, height, available_width, available_height)
        })
    }
}

impl Default for ContentSize {
    fn default() -> Self {
        Self { measure: None }
    }
}

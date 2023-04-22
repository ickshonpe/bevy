use bevy_math::Vec2;

pub trait ConvertFrom<T> {
    fn convert_from(value: T) -> Self;
}

pub trait ConvertInto<T> {
    fn convert_into(self: Self) -> T;
}

impl<T, U> ConvertInto<U> for T
where
    U: ConvertFrom<T>,
{
    fn convert_into(self: Self) -> U {
        U::convert_from(self)
    }
}

pub trait MapFrom<T> {
    type Units;
    fn map_from(value: T, f: impl FnMut(Self::Units) -> Self::Units) -> Self;
}

pub trait MapInto<T, U> {
    fn map_into(self: Self, f: impl FnMut(U) -> U) -> T;
}

impl<T, U, V> MapInto<U, V> for T
where
    U: MapFrom<T, Units = V>,
{
    fn map_into(self: Self, f: impl FnMut(V) -> V) -> U {
        U::map_from(self, f)
    }
}

impl MapFrom<Vec2> for taffy::prelude::Size<f32> {
    type Units = f32;
    #[inline]
    fn map_from(value: Vec2, mut f: impl FnMut(f32) -> f32) -> Self {
        taffy::prelude::Size {
            width: f(value.x),
            height: f(value.y),
        }
    }
}

impl MapFrom<taffy::prelude::Size<f32>> for Vec2 {
    type Units = f32;
    #[inline]
    fn map_from(value: taffy::prelude::Size<f32>, mut f: impl FnMut(f32) -> f32) -> Self {
        Vec2::new(f(value.width), f(value.height))
    }
}

impl ConvertFrom<taffy::prelude::Size<f32>> for Vec2 {
    #[inline]
    fn convert_from(value: taffy::prelude::Size<f32>) -> Vec2 {
        Vec2::new(value.width, value.height)
    }
}

impl ConvertFrom<Vec2> for taffy::prelude::Size<f32> {
    #[inline]
    fn convert_from(value: Vec2) -> taffy::prelude::Size<f32> {
        taffy::prelude::Size {
            width: value.x,
            height: value.y,
        }
    }
}

impl ConvertFrom<taffy::geometry::Point<f32>> for Vec2 {
    fn convert_from(value: taffy::geometry::Point<f32>) -> Self {
        Self::new(value.x, value.y)
    }
}

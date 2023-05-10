use crate::prelude::UiCameraConfig;
use bevy_ecs::prelude::Entity;
use bevy_ecs::query::QueryIter;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::SystemParam;
use bevy_input::prelude::Touches;
use bevy_math::Vec2;
use bevy_render::prelude::Camera;
use bevy_window::PrimaryWindow;
use bevy_window::Window;

#[derive(SystemParam)]
pub struct Windows<'w, 's> {
    primary_window_query: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
    window_query: Query<'w, 's, &'static Window>,
    camera_query: Query<'w, 's, (&'static Camera, Option<&'static UiCameraConfig>)>,
    touches_input: Res<'w, Touches>,
}

impl Windows<'_, '_> {
    fn get_primary_window(&self) -> (Entity, &Window) {
        match self.primary_window_query.get_single() {
            Ok(primary_window_info) => primary_window_info,
            Err(bevy_ecs::query::QuerySingleError::NoEntities(_)) => panic!("No primary window."),
            Err(bevy_ecs::query::QuerySingleError::MultipleEntities(_)) => {
                panic!("Multiple primary windows.")
            }
        }
    }

    pub fn primary_window_index(&self) -> Entity {
        self.get_primary_window().0
    }

    pub fn primary_window(&self) -> &Window {
        self.get_primary_window().1
    }

    /// The scale factor of the primary window
    pub fn primary_scale_factor(&self) -> f64 {
        self.primary_window().scale_factor()
    }

    /// The resolution of the primary window in physical pixels
    pub fn primary_physical_resolution(&self) -> Vec2 {
        Vec2::new(
            self.primary_window().physical_width() as f32,
            self.primary_window().physical_height() as f32,
        )
    }

    /// The resolution of the primary window in logical pixels
    pub fn primary_resolution(&self) -> Vec2 {
        Vec2::new(
            self.primary_window().width(),
            self.primary_window().height(),
        )
    }

    pub fn count_windows(&self) -> usize {
        self.into_iter().len()
    }
}

impl<'w, 's> IntoIterator for &'w Windows<'_, 's> {
    type Item = &'w Window;
    type IntoIter = QueryIter<'w, 's, &'static Window, ()>;

    fn into_iter(self) -> Self::IntoIter {
        self.window_query.iter()
    }
}

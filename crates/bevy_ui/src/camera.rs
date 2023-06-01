use bevy_core_pipeline::tonemapping::DebandDither;
use bevy_core_pipeline::tonemapping::Tonemapping;
use bevy_ecs::prelude::Bundle;
use bevy_ecs::prelude::Component;
use bevy_render::camera::CameraProjection;
use bevy_render::camera::CameraRenderGraph;
use bevy_render::prelude::Camera;
use bevy_render::prelude::OrthographicProjection;
use bevy_render::primitives::Frustum;
use bevy_render::view::VisibleEntities;
use bevy_transform::prelude::GlobalTransform;
use bevy_transform::prelude::Transform;

pub const NAME: &str = "ui_camera";

#[derive(Component)]
pub struct UiCamera;

#[derive(Bundle)]
pub struct UiCameraBundle {
    pub camera: Camera,
    pub camera_render_graph: CameraRenderGraph,
    pub projection: OrthographicProjection,
    pub visible_entities: VisibleEntities,
    pub frustum: Frustum,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub tonemapping: Tonemapping,
    pub deband_dither: DebandDither,
    pub ui_camera: UiCamera,
}

impl Default for UiCameraBundle {
    fn default() -> Self {
        let far = 1000.;
        let transform = Transform::from_xyz(0.0, 0.0, far - 0.1);
        let projection = OrthographicProjection {
            far: far - 0.1,
            ..Default::default()
        };
        let view_projection =
            projection.get_projection_matrix() * transform.compute_matrix().inverse();
        let frustum = Frustum::from_view_projection_custom_far(
            &view_projection,
            &transform.translation,
            &transform.back(),
            projection.far(),
        );
        Self {
            camera_render_graph: CameraRenderGraph::new(NAME),
            projection,
            visible_entities: VisibleEntities::default(),
            frustum,
            camera: Camera::default(),
            transform,
            global_transform: Default::default(),
            tonemapping: Tonemapping::None,
            deband_dither: DebandDither::Disabled,
            ui_camera: UiCamera,
        }
    }
}

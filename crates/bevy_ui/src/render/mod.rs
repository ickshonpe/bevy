mod instances;
mod pipeline;
mod render_pass;

use bevy_core_pipeline::{core_2d::Camera2d, core_3d::Camera3d};
use bevy_render::{ExtractSchedule, Render};
use bevy_window::{PrimaryWindow, Window};
use instances::*;
pub use pipeline::*;
pub use render_pass::*;

use crate::{
    prelude::UiCameraConfig, BackgroundColor, BorderColor, CalculatedClip, Node, UiImage, UiScale,
    UiStack, UiTextureAtlasImage,
};
use crate::{resolve_color_stops, Ellipse, Outline, UiColor, OutlineStyle};

use bevy_app::prelude::*;
use bevy_asset::{load_internal_asset, AssetEvent, Assets, Handle, HandleUntyped};
use bevy_ecs::prelude::*;
use bevy_math::{vec2, Mat4, Rect, UVec4, Vec2, Vec4};
use bevy_reflect::TypeUuid;
use bevy_render::texture::DEFAULT_IMAGE_HANDLE;
use bevy_render::{
    camera::Camera,
    color::Color,
    render_asset::RenderAssets,
    render_graph::{RenderGraph, RunGraphOnViewNode},
    render_phase::{sort_phase_system, AddRenderCommand, DrawFunctions, RenderPhase},
    render_resource::*,
    renderer::{RenderDevice, RenderQueue},
    texture::Image,
    view::{ComputedVisibility, ExtractedView, ViewUniforms},
    Extract, RenderApp, RenderSet,
};
use bevy_sprite::SpriteAssetEvents;
use bevy_sprite::TextureAtlas;
#[cfg(feature = "bevy_text")]
use bevy_text::{PositionedGlyph, Text, TextLayoutInfo};
use bevy_transform::components::GlobalTransform;
use bevy_utils::HashMap;
use bytemuck::{Pod, Zeroable};
use std::ops::Range;

pub mod node {
    pub const UI_PASS_DRIVER: &str = "ui_pass_driver";
}

pub mod draw_ui_graph {
    pub const NAME: &str = "draw_ui";
    pub mod node {
        pub const UI_PASS: &str = "ui_pass";
    }
}

pub const UI_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 13012847047162779583);

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum RenderUiSystem {
    ExtractNode,
    ExtractBorder,
    ExtractOutline,
    ExtractAtlasNode,
}

pub fn build_ui_render(app: &mut App) {
    load_internal_asset!(app, UI_SHADER_HANDLE, "ui.wgsl", Shader::from_wgsl);

    let render_app = match app.get_sub_app_mut(RenderApp) {
        Ok(render_app) => render_app,
        Err(_) => return,
    };

    render_app
        .init_resource::<SpecializedRenderPipelines<UiPipeline>>()
        .init_resource::<UiImageBindGroups>()
        .init_resource::<UiMeta>()
        .init_resource::<ExtractedUiNodes>()
        .init_resource::<DrawFunctions<TransparentUi>>()
        .add_render_command::<TransparentUi, DrawUi>()
        .add_systems(
            ExtractSchedule,
            (
                extract_default_ui_camera_view::<Camera2d>,
                extract_default_ui_camera_view::<Camera3d>,
                extract_uinodes.in_set(RenderUiSystem::ExtractNode),
                extract_atlas_uinodes
                    .in_set(RenderUiSystem::ExtractAtlasNode)
                    .after(RenderUiSystem::ExtractNode),
                extract_borders
                    .in_set(RenderUiSystem::ExtractBorder)
                    .after(RenderUiSystem::ExtractAtlasNode),
                extract_outlines.after(RenderUiSystem::ExtractBorder),
                #[cfg(feature = "bevy_text")]
                extract_text_uinodes
                    .after(RenderUiSystem::ExtractAtlasNode)
                    .after(RenderUiSystem::ExtractBorder)
                    .after(RenderUiSystem::ExtractOutline),
            ),
        )
        .add_systems(
            Render,
            (
                prepare_uinodes.in_set(RenderSet::Prepare),
                queue_uinodes.in_set(RenderSet::Queue),
                sort_phase_system::<TransparentUi>.in_set(RenderSet::PhaseSort),
            ),
        );

    // Render graph
    let ui_graph_2d = get_ui_graph(render_app);
    let ui_graph_3d = get_ui_graph(render_app);
    let mut graph = render_app.world.resource_mut::<RenderGraph>();

    if let Some(graph_2d) = graph.get_sub_graph_mut(bevy_core_pipeline::core_2d::graph::NAME) {
        graph_2d.add_sub_graph(draw_ui_graph::NAME, ui_graph_2d);
        graph_2d.add_node(
            draw_ui_graph::node::UI_PASS,
            RunGraphOnViewNode::new(draw_ui_graph::NAME),
        );
        graph_2d.add_node_edge(
            bevy_core_pipeline::core_2d::graph::node::MAIN_PASS,
            draw_ui_graph::node::UI_PASS,
        );
        graph_2d.add_node_edge(
            bevy_core_pipeline::core_2d::graph::node::END_MAIN_PASS_POST_PROCESSING,
            draw_ui_graph::node::UI_PASS,
        );
        graph_2d.add_node_edge(
            draw_ui_graph::node::UI_PASS,
            bevy_core_pipeline::core_2d::graph::node::UPSCALING,
        );
    }

    if let Some(graph_3d) = graph.get_sub_graph_mut(bevy_core_pipeline::core_3d::graph::NAME) {
        graph_3d.add_sub_graph(draw_ui_graph::NAME, ui_graph_3d);
        graph_3d.add_node(
            draw_ui_graph::node::UI_PASS,
            RunGraphOnViewNode::new(draw_ui_graph::NAME),
        );
        graph_3d.add_node_edge(
            bevy_core_pipeline::core_3d::graph::node::END_MAIN_PASS,
            draw_ui_graph::node::UI_PASS,
        );
        graph_3d.add_node_edge(
            bevy_core_pipeline::core_3d::graph::node::END_MAIN_PASS_POST_PROCESSING,
            draw_ui_graph::node::UI_PASS,
        );
        graph_3d.add_node_edge(
            draw_ui_graph::node::UI_PASS,
            bevy_core_pipeline::core_3d::graph::node::UPSCALING,
        );
    }
}

fn get_ui_graph(render_app: &mut App) -> RenderGraph {
    let ui_pass_node = UiPassNode::new(&mut render_app.world);
    let mut ui_graph = RenderGraph::default();
    ui_graph.add_node(draw_ui_graph::node::UI_PASS, ui_pass_node);
    ui_graph
}

pub fn extract_atlas_uinodes(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    images: Extract<Res<Assets<Image>>>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<
            (
                &Node,
                &GlobalTransform,
                &BackgroundColor,
                &ComputedVisibility,
                Option<&CalculatedClip>,
                &Handle<TextureAtlas>,
                &UiTextureAtlasImage,
            ),
            Without<UiImage>,
        >,
    >,
) {
    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((
            uinode,
            _transform,
            color,
            visibility,
            clip,
            texture_atlas_handle,
            atlas_image,
        )) = uinode_query.get(*entity)
        {
            // Skip invisible and completely transparent nodes
            if !visibility.is_visible() {
                continue;
            }

            let (mut atlas_rect, atlas_size, image) =
                if let Some(texture_atlas) = texture_atlases.get(texture_atlas_handle) {
                    let atlas_rect = *texture_atlas
                        .textures
                        .get(atlas_image.index)
                        .unwrap_or_else(|| {
                            panic!(
                                "Atlas index {:?} does not exist for texture atlas handle {:?}.",
                                atlas_image.index,
                                texture_atlas_handle.id(),
                            )
                        });
                    (
                        atlas_rect,
                        texture_atlas.size,
                        texture_atlas.texture.clone(),
                    )
                } else {
                    // Atlas not present in assets resource (should this warn the user?)
                    continue;
                };

            // Skip loading images
            if !images.contains(&image) {
                continue;
            }

            atlas_rect.min /= atlas_size;
            atlas_rect.max /= atlas_size;

            let color = match &color.0 {
                UiColor::Color(color) => *color,
                _ => Color::NONE,
            };

            extracted_uinodes.push_node(
                stack_index,
                uinode.position.into(),
                uinode.size().into(),
                Some(image),
                atlas_rect,
                color,
                uinode.border_radius,
                uinode.border,
                clip.map(|clip| clip.clip),
                
            );
        }
    }
}

pub fn extract_uinodes(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    images: Extract<Res<Assets<Image>>>,
    ui_stack: Extract<Res<UiStack>>,
    ui_scale: Extract<Res<UiScale>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
    uinode_query: Extract<
        Query<
            (
                &Node,
                &BackgroundColor,
                Option<&UiImage>,
                &ComputedVisibility,
                Option<&CalculatedClip>,
            ),
            Without<UiTextureAtlasImage>,
        >,
    >,
) {
    let viewport_size = windows
        .get_single()
        .map(|window| vec2(window.resolution.width(), window.resolution.height()))
        .unwrap_or(Vec2::ZERO)
        / ui_scale.scale as f32;

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, color, maybe_image, visibility, clip)) = uinode_query.get(*entity) {
            if !visibility.is_visible() {
                continue;
            }

            if color.is_visible() {
                let (image, _flip_x, _flip_y) = if let Some(image) = maybe_image {
                    // Skip loading images
                    if !images.contains(&image.texture) {
                        continue;
                    }
                    (Some(image.texture.clone_weak()), image.flip_x, image.flip_y)
                } else {
                    (None, false, false)
                };

                match &color.0 {
                    UiColor::Color(color) => {
                        extracted_uinodes.push_node(
                            stack_index,
                            uinode.position,
                            uinode.size(),
                            image,
                            Rect::new(0.0, 0.0, 1.0, 1.0),
                            *color,
                            uinode.border_radius,
                            uinode.border,
                            clip.map(|clip| clip.clip),
                        );
                    }
                    UiColor::LinearGradient(l) => {
                        let (start_point, length) = l.resolve_geometry(uinode.rect());
                        let stops = resolve_color_stops(&l.stops, length, viewport_size);
                        extracted_uinodes.push_node_with_linear_gradient(
                            stack_index,
                            uinode.position,
                            uinode.size(),
                            image,
                            Rect::new(0.0, 0.0, 1.0, 1.0),
                            uinode.border_radius,
                            start_point,
                            l.angle,
                            &stops,
                            clip.map(|clip| clip.clip),
                        );
                    }
                    UiColor::RadialGradient(r) => {
                        let ellipse = r.resolve_geometry(uinode.rect(), viewport_size);
                        let stops = resolve_color_stops(&r.stops, ellipse.extents.x, viewport_size);
                        extracted_uinodes.push_node_with_radial_gradient(
                            stack_index,
                            uinode.position,
                            uinode.size(),
                            image,
                            Rect::new(0.0, 0.0, 1.0, 1.0),
                            uinode.border_radius,
                            ellipse,
                            &stops,
                            clip.map(|clip| clip.clip),
                        );
                    }
                }
            }
        }
    }
}

pub fn extract_borders(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    ui_stack: Extract<Res<UiStack>>,
    ui_scale: Extract<Res<UiScale>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
    uinode_query: Extract<
        Query<(
            &Node,
            &BorderColor,
            &ComputedVisibility,
            Option<&CalculatedClip>,
        )>,
    >,
) {
    let viewport_size = windows
        .get_single()
        .map(|window| vec2(window.resolution.width(), window.resolution.height()))
        .unwrap_or(Vec2::ZERO)
        / ui_scale.scale as f32;

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, border_color, visibility, clip)) = uinode_query.get(*entity) {
            if !visibility.is_visible() {
                continue;
            }
    
            let size = uinode.size();
            let position = uinode.position();
            let border = uinode.border;

            if border_color.is_visible() {
                match &border_color.0 {
                    UiColor::Color(color) => {
                        extracted_uinodes.push_border(
                            stack_index,
                            position,
                            size,
                            *color,
                            border,
                            uinode.border_radius,
                            clip.map(|clip| clip.clip),
                        );
                    }
                    UiColor::LinearGradient(l) => {
                        let (start_point, length) = l.resolve_geometry(uinode.rect());
                        let stops = resolve_color_stops(&l.stops, length, viewport_size);
                        extracted_uinodes.push_border_with_linear_gradient(
                            stack_index,
                            position,
                            size,
                            border,
                            uinode.border_radius,
                            start_point,
                            l.angle,
                            &stops,
                            clip.map(|clip| clip.clip),
                        );
                    }
                    UiColor::RadialGradient(r) => {
                        let ellipse = r.resolve_geometry(uinode.rect(), viewport_size);
                        let stops = resolve_color_stops(&r.stops, ellipse.extents.x, viewport_size);
                        extracted_uinodes.push_border_with_radial_gradient(
                            stack_index,
                            position,
                            size,
                            border,
                            uinode.border_radius,
                            ellipse,
                            &stops,
                            clip.map(|clip| clip.clip),
                        );
                    }
                }
            }
        }
    }
}

pub fn extract_outlines(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<(
            &Node,
            &Outline,
            Option<&OutlineStyle>,
            &ComputedVisibility,
            Option<&CalculatedClip>,
        )>,
    >,
) {
    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, outline, maybe_outline_style, visibility, clip)) = uinode_query.get(*entity) {
            if !visibility.is_visible() {
                continue;
            }

            match maybe_outline_style.unwrap_or(&OutlineStyle::Solid) {
                OutlineStyle::Solid => {
                    extracted_uinodes.push_border(
                        stack_index,
                        uinode.position() - Vec2::splat(uinode.outline_offset + uinode.outline_width),
                        uinode.size() + 2. * (uinode.outline_width + uinode.outline_offset),
                        outline.color,
                        [uinode.outline_width; 4],
                        uinode.border_radius,
                        clip.map(|clip| clip.clip),
                    );
                },
                OutlineStyle::Dashed(gap) => {
                    extracted_uinodes.push_dashed_border(
                        stack_index, 
                        uinode.position() - Vec2::splat(uinode.outline_offset + uinode.outline_width),
                        uinode.size() + 2. * (uinode.outline_width + uinode.outline_offset),
                        outline.color,
                        uinode.outline_width,
                        *gap,
                        uinode.border_radius,
                        clip.map(|clip| clip.clip),
                    )
                },
            }
        }
    }
}

/// The UI camera is "moved back" by this many units (plus the [`UI_CAMERA_TRANSFORM_OFFSET`]) and also has a view
/// distance of this many units. This ensures that with a left-handed projection,
/// as ui elements are "stacked on top of each other", they are within the camera's view
/// and have room to grow.
// TODO: Consider computing this value at runtime based on the maximum z-value.
const UI_CAMERA_FAR: f32 = 1000.0;

// This value is subtracted from the far distance for the camera's z-position to ensure nodes at z == 0.0 are rendered
// TODO: Evaluate if we still need this.
const UI_CAMERA_TRANSFORM_OFFSET: f32 = -0.1;

#[derive(Component)]
pub struct DefaultCameraView(pub Entity);

pub fn extract_default_ui_camera_view<T: Component>(
    mut commands: Commands,
    ui_scale: Extract<Res<UiScale>>,
    query: Extract<Query<(Entity, &Camera, Option<&UiCameraConfig>), With<T>>>,
) {
    let scale = (ui_scale.scale as f32).recip();
    for (entity, camera, camera_ui) in &query {
        // ignore cameras with disabled ui
        if matches!(camera_ui, Some(&UiCameraConfig { show_ui: false, .. })) {
            continue;
        }
        if let (Some(logical_size), Some((physical_origin, _)), Some(physical_size)) = (
            camera.logical_viewport_size(),
            camera.physical_viewport_rect(),
            camera.physical_viewport_size(),
        ) {
            // use a projection matrix with the origin in the top left instead of the bottom left that comes with OrthographicProjection
            let projection_matrix = Mat4::orthographic_rh(
                0.0,
                logical_size.x * scale,
                logical_size.y * scale,
                0.0,
                0.0,
                UI_CAMERA_FAR,
            );
            let default_camera_view = commands
                .spawn(ExtractedView {
                    projection: projection_matrix,
                    transform: GlobalTransform::from_xyz(
                        0.0,
                        0.0,
                        UI_CAMERA_FAR + UI_CAMERA_TRANSFORM_OFFSET,
                    ),
                    view_projection: None,
                    hdr: camera.hdr,
                    viewport: UVec4::new(
                        physical_origin.x,
                        physical_origin.y,
                        physical_size.x,
                        physical_size.y,
                    ),
                    color_grading: Default::default(),
                })
                .id();
            commands.get_or_spawn(entity).insert((
                DefaultCameraView(default_camera_view),
                RenderPhase::<TransparentUi>::default(),
            ));
        }
    }
}

#[cfg(feature = "bevy_text")]
pub fn extract_text_uinodes(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
    ui_stack: Extract<Res<UiStack>>,
    ui_scale: Extract<Res<UiScale>>,
    uinode_query: Extract<
        Query<(
            &Node,
            &Text,
            &TextLayoutInfo,
            &ComputedVisibility,
            Option<&CalculatedClip>,
        )>,
    >,
) {
    // TODO: Support window-independent UI scale: https://github.com/bevyengine/bevy/issues/5621
    let (scale_factor, _viewport_size) = {
        let (scale_factor, viewport_size) = windows
            .get_single()
            .map(|window| {
                (
                    window.resolution.scale_factor(),
                    vec2(window.resolution.width(), window.resolution.height()),
                )
            })
            .unwrap_or((1., Vec2::ZERO));
        (
            scale_factor * ui_scale.scale,
            viewport_size * ui_scale.scale as f32,
        )
    };

    let inverse_scale_factor = (scale_factor as f32).recip();

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, text, text_layout_info, visibility, clip)) = uinode_query.get(*entity) {
            // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
            if !visibility.is_visible() || uinode.size().x == 0. || uinode.size().y == 0. {
                continue;
            }

            let node_position = uinode.position();

            let mut color = Color::WHITE;
            let mut current_section = usize::MAX;
            for PositionedGlyph {
                position: glyph_position,
                atlas_info,
                section_index,
                ..
            } in &text_layout_info.glyphs
            {
                if *section_index != current_section {
                    color = text.sections[*section_index].style.color.as_rgba_linear();
                    current_section = *section_index;
                }
                let atlas = texture_atlases.get(&atlas_info.texture_atlas).unwrap();

                let mut uv_rect = atlas.textures[atlas_info.glyph_index];
                let scaled_glyph_size = uv_rect.size() * inverse_scale_factor;
                let scaled_glyph_position = *glyph_position * inverse_scale_factor;
                uv_rect.min /= atlas.size;
                uv_rect.max /= atlas.size;

                let position = node_position + scaled_glyph_position - 0.5 * scaled_glyph_size;

                extracted_uinodes.push_glyph(
                    stack_index,
                    position,
                    scaled_glyph_size,
                    atlas.texture.clone(),
                    color,
                    clip.map(|clip| clip.clip),
                    uv_rect,
                );
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct ExtractedUiNodes {
    pub uinodes: Vec<ExtractedItem>,
}

pub struct ExtractedItem {
    pub stack_index: u32,
    pub image: Handle<Image>,
    pub instance: ExtractedInstance,
}

impl ExtractedItem {
    fn new(
        stack_index: usize,
        image: Handle<Image>,
        instance: impl Into<ExtractedInstance>,
    ) -> Self {
        Self {
            stack_index: stack_index as u32,
            image,
            instance: instance.into(),
        }
    }
}

pub(crate) fn rect_to_f32_4(r: Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.max.x, r.max.y]
}

impl ExtractedUiNodes {
    pub fn push_glyph(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        image: Handle<Image>,
        color: Color,
        clip: Option<Rect>,
        uv_rect: Rect,
    ) {
        let color = color.as_linear_rgba_f32();
        let uv_min = uv_rect.min.into();
        let uv_size = uv_rect.size().into();
        let i = TextInstance {
            location: position.into(),
            size: size.into(),
            uv_min,
            uv_size,
            color,
        };
        self.uinodes
            .push(ExtractedItem::new(stack_index, image, (i, clip)));
    }

    pub fn push_node(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        image: Option<Handle<Image>>,
        uv_rect: Rect,
        color: Color,
        radius: [f32; 4],
        border: [f32; 4],
        clip: Option<Rect>,
    ) {
        let color = color.as_linear_rgba_f32();
        let uv_min = uv_rect.min;
        let uv_size = uv_rect.size();

        let flags = if image.is_some() {
            TEXTURED_QUAD
        } else {
            UNTEXTURED_QUAD
        };
        let image = image.unwrap_or(DEFAULT_IMAGE_HANDLE.typed());

        let i = NodeInstance {
            location: position.into(),
            size: size.into(),
            uv: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
            color,
            radius,
            flags,
            border,
        };
        self.uinodes
            .push(ExtractedItem::new(stack_index, image, (i, clip)));
    }

    pub fn push_border(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        color: Color,
        inset: [f32; 4],
        radius: [f32; 4],
        clip: Option<Rect>,
    ) {
        let color = color.as_linear_rgba_f32();
        let flags = UNTEXTURED_QUAD | BORDERED;
        let i = NodeInstance {
            location: position.into(),
            size: size.into(),
            uv: [0., 0., 1., 1.],
            color,
            radius,
            flags,
            border: inset,
        };
        self.uinodes.push(ExtractedItem::new(
            stack_index,
            DEFAULT_IMAGE_HANDLE.typed(),
            (i, clip),
        ));
    }

    pub fn push_dashed_border(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        color: Color,
        line_thickness: f32,
        gap_length: f32,
        radius: [f32; 4],
        clip: Option<Rect>,
    ) {
        let color = color.as_linear_rgba_f32();
        let i = DashedBorderInstance {
            location: position.into(),
            size: size.into(),
            color,
            radius,
            line_thickness,
            gap_length,
        };
        self.uinodes.push(ExtractedItem::new(
            stack_index,
            DEFAULT_IMAGE_HANDLE.typed(),
            (i, clip),
        ));
    }

    pub fn push_border_with_linear_gradient(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        inset: [f32; 4],
        radius: [f32; 4],
        start_point: Vec2,
        angle: f32,
        stops: &[(Color, f32)],
        clip: Option<Rect>,
    ) {
        for i in 0..stops.len() - 1 {
            let start = &stops[i];
            let end = &stops[i + 1];

            let mut flags = UNTEXTURED_QUAD | BORDERED;
            if i == 0 {
                flags |= FILL_START;
            }

            if i + 2 == stops.len() {
                flags |= FILL_END;
            }

            let i = LinearGradientInstance {
                location: position.into(),
                size: size.into(),
                uv_border: inset,
                radius,
                flags,
                focal_point: start_point.into(),
                angle,
                start_color: start.0.as_linear_rgba_f32(),
                start_len: start.1,
                end_len: end.1,
                end_color: end.0.as_linear_rgba_f32(),
            };
            self.uinodes.push(ExtractedItem::new(
                stack_index,
                DEFAULT_IMAGE_HANDLE.typed(),
                (i, clip),
            ));
        }
    }

    pub fn push_border_with_radial_gradient(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        inset: [f32; 4],
        radius: [f32; 4],
        ellipse: Ellipse,
        stops: &[(Color, f32)],
        clip: Option<Rect>,
    ) {
        let start_point: Vec2 = (ellipse.center - position - 0.5 * size).into();
        let ratio = ellipse.extents.x / ellipse.extents.y;

        for i in 0..stops.len() - 1 {
            let start = &stops[i];
            let end = &stops[i + 1];

            let mut flags = UNTEXTURED_QUAD | BORDERED;
            if i == 0 {
                flags |= FILL_START;
            }

            if i + 2 == stops.len() {
                flags |= FILL_END;
            }

            let i = RadialGradientInstance {
                location: position.into(),
                size: size.into(),
                uv_border: inset,
                radius,
                flags,
                ratio,
                start_point: start_point.into(),
                start_color: start.0.as_linear_rgba_f32(),
                start_len: start.1,
                end_len: end.1,
                end_color: end.0.as_linear_rgba_f32(),
            };
            self.uinodes.push(ExtractedItem::new(
                stack_index,
                DEFAULT_IMAGE_HANDLE.typed(),
                (i, clip),
            ));
        }
    }

    pub fn push_node_with_linear_gradient(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        image: Option<Handle<Image>>,
        uv_rect: Rect,
        radius: [f32; 4],
        start_point: Vec2,
        angle: f32,
        stops: &[(Color, f32)],
        clip: Option<Rect>,
    ) {
        let uv_min = uv_rect.min;
        let uv_size = uv_rect.size();

        let tflag = if image.is_some() {
            TEXTURED_QUAD //| FILL_START | FILL_END
        } else {
            UNTEXTURED_QUAD //| FILL_START | FILL_END
        };

        let image = image.unwrap_or(DEFAULT_IMAGE_HANDLE.typed());

        for i in 0..stops.len() - 1 {
            let start = &stops[i];
            let end = &stops[i + 1];
            let mut flags = tflag;
            if i == 0 {
                flags |= FILL_START;
            }

            if i + 2 == stops.len() {
                flags |= FILL_END;
            }

            let i = LinearGradientInstance {
                location: position.into(),
                size: size.into(),
                uv_border: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
                radius,
                flags,
                focal_point: start_point.into(),
                angle,
                start_color: start.0.as_linear_rgba_f32(),
                start_len: start.1,
                end_len: end.1,
                end_color: end.0.as_linear_rgba_f32(),
            };
            self.uinodes
                .push(ExtractedItem::new(stack_index, image.clone(), (i, clip)));
        }
    }

    pub fn push_node_with_radial_gradient(
        &mut self,
        stack_index: usize,
        position: Vec2,
        size: Vec2,
        image: Option<Handle<Image>>,
        uv_rect: Rect,
        radius: [f32; 4],
        ellipse: Ellipse,
        stops: &[(Color, f32)],
        clip: Option<Rect>,
    ) {
        let tflag = if image.is_some() {
            TEXTURED_QUAD
        } else {
            UNTEXTURED_QUAD
        };

        let uv_min = uv_rect.min;
        let uv_size = uv_rect.size();

        let image = image.unwrap_or(DEFAULT_IMAGE_HANDLE.typed());
        let start_point = (ellipse.center - position - 0.5 * size).into();
        let ratio = ellipse.extents.x / ellipse.extents.y;
        for i in 0..stops.len() - 1 {
            let start = &stops[i];
            let end = &stops[i + 1];
            let mut flags = tflag;
            if i == 0 {
                flags |= FILL_START;
            }

            if i + 2 == stops.len() {
                flags |= FILL_END;
            }

            let i = RadialGradientInstance {
                location: position.into(),
                size: size.into(),
                uv_border: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
                radius,
                flags,
                start_point,
                ratio,
                start_color: start.0.as_linear_rgba_f32(),
                start_len: start.1,
                end_len: end.1,
                end_color: end.0.as_linear_rgba_f32(),
            };
            self.uinodes
                .push(ExtractedItem::new(stack_index, image.clone(), (i, clip)));
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, ShaderType, Default)]
struct UiClip {
    clip: Vec4,
}

#[derive(Resource)]
pub struct UiMeta {
    pub view_bind_group: Option<BindGroup>,
    pub index_buffer: BufferVec<u32>,
    pub instance_buffers: UiInstanceBuffers,
}

impl Default for UiMeta {
    fn default() -> Self {
        Self {
            view_bind_group: None,
            index_buffer: BufferVec::<u32>::new(BufferUsages::INDEX),
            instance_buffers: Default::default(),
        }
    }
}

impl UiMeta {
    fn clear_instance_buffers(&mut self) {
        self.instance_buffers.clear_all();
    }

    fn write_instance_buffers(&mut self, render_device: &RenderDevice, render_queue: &RenderQueue) {
        self.instance_buffers.write_all(render_device, render_queue);
    }

    fn push(&mut self, item: &ExtractedInstance) {
        item.push(&mut self.instance_buffers);
    }
}

#[derive(Component)]
pub struct UiBatch {
    pub batch_type: BatchType,
    pub range: Range<u32>,
    pub image: Handle<Image>,
    pub stack_index: u32,
}

const UNTEXTURED_QUAD: u32 = 0;
const TEXTURED_QUAD: u32 = 1;
const BORDERED: u32 = 32;
const FILL_START: u32 = 64;
const FILL_END: u32 = 128;

pub fn prepare_uinodes(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
) {
    ui_meta.clear_instance_buffers();

    // sort by ui stack index, starting from the deepest node
    extracted_uinodes
        .uinodes
        .sort_by_key(|node| node.stack_index);

    let mut instance_counters = InstanceCounters::default();
    let mut batches: Vec<UiBatch> = vec![];
    for node in &extracted_uinodes.uinodes {
        ui_meta.push(&node.instance);
        let index = instance_counters.increment(node.instance.get_type());
        let current_batch = batches.last_mut().filter(|batch| {
            batch.batch_type == node.instance.get_type() && batch.image.id() == node.image.id()
        });
        if let Some(batch) = current_batch {
            batch.range.end = index;
        } else {
            let new_batch = UiBatch {
                batch_type: node.instance.get_type(),
                image: node.image.clone(),
                stack_index: node.stack_index,
                range: index - 1..index,
            };
            batches.push(new_batch);
        }
    }
    commands.spawn_batch(batches);

    ui_meta.write_instance_buffers(&render_device, &render_queue);

    if ui_meta.index_buffer.len() != 6 {
        ui_meta.index_buffer.clear();

        // NOTE: This code is creating 6 indices pointing to 4 vertices.
        // The vertices form the corners of a quad based on their two least significant bits.
        // 10   11
        //
        // 00   01
        // The sprite shader can then use the two least significant bits as the vertex index.
        // The rest of the properties to transform the vertex positions and UVs (which are
        // implicit) are baked into the instance transform, and UV offset and scale.
        // See bevy_sprite/src/render/sprite.wgsl for the details.
        ui_meta.index_buffer.push(2);
        ui_meta.index_buffer.push(0);
        ui_meta.index_buffer.push(1);
        ui_meta.index_buffer.push(1);
        ui_meta.index_buffer.push(3);
        ui_meta.index_buffer.push(2);

        ui_meta
            .index_buffer
            .write_buffer(&render_device, &render_queue);
    }
    extracted_uinodes.uinodes.clear();
}

#[derive(Resource, Default)]
pub struct UiImageBindGroups {
    pub values: HashMap<Handle<Image>, BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub fn queue_uinodes(
    draw_functions: Res<DrawFunctions<TransparentUi>>,
    render_device: Res<RenderDevice>,
    mut ui_meta: ResMut<UiMeta>,
    view_uniforms: Res<ViewUniforms>,
    ui_pipeline: Res<UiPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<UiPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut image_bind_groups: ResMut<UiImageBindGroups>,
    gpu_images: Res<RenderAssets<Image>>,
    ui_batches: Query<(Entity, &UiBatch)>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<TransparentUi>)>,
    events: Res<SpriteAssetEvents>,
) {
    // If an image has changed, the GpuImage has (probably) changed
    for event in &events.images {
        match event {
            AssetEvent::Created { .. } => None,
            AssetEvent::Modified { handle } | AssetEvent::Removed { handle } => {
                image_bind_groups.values.remove(handle)
            }
        };
    }

    if let Some(view_binding) = view_uniforms.uniforms.binding() {
        ui_meta.view_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding,
            }],
            label: Some("ui_view_bind_group"),
            layout: &ui_pipeline.view_layout,
        }));

        let draw_ui_function = draw_functions.read().id::<DrawUi>();
        for (view, mut transparent_phase) in &mut views {
            let node_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: false,
                    specialization: UiPipelineSpecialization::Node,
                },
            );
            let clipped_node_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: true,
                    specialization: UiPipelineSpecialization::Node,
                },
            );
            let text_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: false,
                    specialization: UiPipelineSpecialization::Text,
                },
            );
            let clipped_text_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: true,
                    specialization: UiPipelineSpecialization::Text,
                },
            );
            let linear_gradient_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: false,
                    specialization: UiPipelineSpecialization::LinearGradient,
                },
            );
            let clipped_linear_gradient_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: true,
                    specialization: UiPipelineSpecialization::LinearGradient,
                },
            );

            let radial_gradient_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: false,
                    specialization: UiPipelineSpecialization::RadialGradient,
                },
            );
            let clipped_radial_gradient_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: true,
                    specialization: UiPipelineSpecialization::RadialGradient,
                },
            );
            let dashed_border_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: false,
                    specialization: UiPipelineSpecialization::DashedBorder,
                },
            );
            let clipped_dashed_border_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey {
                    hdr: view.hdr,
                    clip: true,
                    specialization: UiPipelineSpecialization::DashedBorder,
                },
            );

            for (entity, batch) in &ui_batches {
                image_bind_groups
                    .values
                    .entry(batch.image.clone_weak())
                    .or_insert_with(|| {
                        let gpu_image = gpu_images.get(&batch.image).unwrap();
                        render_device.create_bind_group(&BindGroupDescriptor {
                            entries: &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::TextureView(&gpu_image.texture_view),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::Sampler(&gpu_image.sampler),
                                },
                            ],
                            label: Some("ui_material_bind_group"),
                            layout: &ui_pipeline.image_layout,
                        })
                    });
                let pipeline = match batch.batch_type {
                    BatchType::Node => node_pipeline,
                    BatchType::Text => text_pipeline,
                    BatchType::LinearGradient => linear_gradient_pipeline,
                    BatchType::CNode => clipped_node_pipeline,
                    BatchType::CText => clipped_text_pipeline,
                    BatchType::CLinearGradient => clipped_linear_gradient_pipeline,
                    BatchType::RadialGradient => radial_gradient_pipeline,
                    BatchType::CRadialGradient => clipped_radial_gradient_pipeline,
                    BatchType::DashedBorder => dashed_border_pipeline,
                    BatchType::CDashedBorder => clipped_dashed_border_pipeline,
                    
                };
                transparent_phase.add(TransparentUi {
                    draw_function: draw_ui_function,
                    pipeline,
                    entity,
                    sort_key: batch.stack_index,
                });
            }
        }
    }
}

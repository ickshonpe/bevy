mod pipeline;
mod render_pass;

use bevy_core_pipeline::{core_2d::Camera2d, core_3d::Camera3d};
use bevy_hierarchy::Parent;
use bevy_render::{ExtractSchedule, Render};
use bevy_window::{PrimaryWindow, Window};
pub use pipeline::*;
pub use render_pass::*;

use crate::{
    prelude::UiCameraConfig, BackgroundColor, BorderColor, CalculatedClip, ContentSize, Node,
    Style, UiImage, UiScale, UiStack, UiTextureAtlasImage, Val,
};

use crate::UiContentTransform;
use bevy_app::prelude::*;
use bevy_asset::{load_internal_asset, AssetEvent, Assets, Handle, HandleUntyped};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Rect, URect, UVec4, Vec2, Vec2Swizzles, Vec3, Vec4Swizzles, Vec3Swizzles, vec2};
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
use bevy_utils::FloatOrd;
use bevy_utils::HashMap;
use bytemuck::{Pod, Zeroable};
use std::mem::swap;
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
                extract_uinode_borders.after(RenderUiSystem::ExtractAtlasNode),
                #[cfg(feature = "bevy_text")]
                extract_text_uinodes.after(RenderUiSystem::ExtractAtlasNode),
            ),
        )
        .add_systems(
            Render,
            (
                prepare_uinodes_4.in_set(RenderSet::Prepare),
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

pub struct ExtractedUiNode {
    pub stack_index: usize,
    pub transform: Mat4,
    pub color: Color,
    pub image: Handle<Image>,
    pub size: Vec2,
    pub uv_rect: Rect,
    pub clip: Option<Rect>,
    pub content_transform: UiContentTransform,
}

#[derive(Resource, Default)]
pub struct ExtractedUiNodes {
    pub uinodes: Vec<ExtractedUiNode>,
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
                Option<&UiContentTransform>,
            ),
            Without<UiImage>,
        >,
    >,
) {
    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((
            uinode,
            transform,
            color,
            visibility,
            clip,
            texture_atlas_handle,
            atlas_image,
            orientation,
        )) = uinode_query.get(*entity)
        {
            // Skip invisible and completely transparent nodes
            if !visibility.is_visible() || color.0.a() == 0.0 {
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

            // let scale = uinode.size() / atlas_rect.size();
            // atlas_rect.min *= scale;
            // atlas_rect.max *= scale;
            // atlas_size *= scale;
    
            //println!("atlas size = {}", atlas_size);
            atlas_rect.min /= atlas_size;
            atlas_rect.max /= atlas_size;
           // println!("atlas_rect = {:?}", atlas_rect);

            let mut transform = transform.compute_matrix();
            // if let Some(orientation) = orientation {
            //     transform *= Mat4::from(*orientation);

            //     if orientation.is_sideways() {
            //         let aspect = uinode.size().y / uinode.size().x;
            //         transform *= Mat4::from_scale(Vec3::new(aspect, aspect.recip(), 1.));
            //     }
            // }



            extracted_uinodes.uinodes.push(ExtractedUiNode {
                stack_index,
                transform,
                color: color.0,
                size: uinode.size(),
                clip: clip.map(|clip| clip.clip),
                image,
                uv_rect: atlas_rect,
                content_transform: orientation.copied().unwrap_or_default(),
            });
        }
    }
}

fn resolve_border_thickness(value: Val, parent_width: f32, viewport_size: Vec2) -> f32 {
    match value {
        Val::Auto => 0.,
        Val::Px(px) => px.max(0.),
        Val::Percent(percent) => (parent_width * percent / 100.).max(0.),
        Val::Vw(percent) => (viewport_size.x * percent / 100.).max(0.),
        Val::Vh(percent) => (viewport_size.y * percent / 100.).max(0.),
        Val::VMin(percent) => (viewport_size.min_element() * percent / 100.).max(0.),
        Val::VMax(percent) => (viewport_size.max_element() * percent / 100.).max(0.),
    }
}

pub fn extract_uinode_borders(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
    ui_scale: Extract<Res<UiScale>>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<
            (
                &Node,
                &GlobalTransform,
                &Style,
                &BorderColor,
                Option<&Parent>,
                &ComputedVisibility,
                Option<&CalculatedClip>,
            ),
            Without<ContentSize>,
        >,
    >,
    node_query: Extract<Query<&Node>>,
) {
    let image = bevy_render::texture::DEFAULT_IMAGE_HANDLE.typed();

    let ui_logical_viewport_size = windows
        .get_single()
        .map(|window| Vec2::new(window.resolution.width(), window.resolution.height()))
        .unwrap_or(Vec2::ZERO)
        // The logical window resolution returned by `Window` only takes into account the window scale factor and not `UiScale`,
        // so we have to divide by `UiScale` to get the size of the UI viewport.
        / ui_scale.scale as f32;

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((node, global_transform, style, border_color, parent, visibility, clip)) =
            uinode_query.get(*entity)
        {
            // Skip invisible borders
            if !visibility.is_visible()
                || border_color.0.a() == 0.0
                || node.size().x <= 0.
                || node.size().y <= 0.
            {
                continue;
            }

            // Both vertical and horizontal percentage border values are calculated based on the width of the parent node
            // <https://developer.mozilla.org/en-US/docs/Web/CSS/border-width>
            let parent_width = parent
                .and_then(|parent| node_query.get(parent.get()).ok())
                .map(|parent_node| parent_node.size().x)
                .unwrap_or(ui_logical_viewport_size.x);
            let left =
                resolve_border_thickness(style.border.left, parent_width, ui_logical_viewport_size);
            let right = resolve_border_thickness(
                style.border.right,
                parent_width,
                ui_logical_viewport_size,
            );
            let top =
                resolve_border_thickness(style.border.top, parent_width, ui_logical_viewport_size);
            let bottom = resolve_border_thickness(
                style.border.bottom,
                parent_width,
                ui_logical_viewport_size,
            );

            // Calculate the border rects, ensuring no overlap.
            // The border occupies the space between the node's bounding rect and the node's bounding rect inset in each direction by the node's corresponding border value.
            let max = 0.5 * node.size();
            let min = -max;
            let inner_min = min + Vec2::new(left, top);
            let inner_max = (max - Vec2::new(right, bottom)).max(inner_min);
            let border_rects = [
                // Left border
                Rect {
                    min,
                    max: Vec2::new(inner_min.x, max.y),
                },
                // Right border
                Rect {
                    min: Vec2::new(inner_max.x, min.y),
                    max,
                },
                // Top border
                Rect {
                    min: Vec2::new(inner_min.x, min.y),
                    max: Vec2::new(inner_max.x, inner_min.y),
                },
                // Bottom border
                Rect {
                    min: Vec2::new(inner_min.x, inner_max.y),
                    max: Vec2::new(inner_max.x, max.y),
                },
            ];

            let transform = global_transform.compute_matrix();

            for edge in border_rects {
                if edge.min.x < edge.max.x && edge.min.y < edge.max.y {
                    extracted_uinodes.uinodes.push(ExtractedUiNode {
                        stack_index,
                        // This translates the uinode's transform to the center of the current border rectangle
                        transform: transform * Mat4::from_translation(edge.center().extend(0.)),
                        color: border_color.0,
                        size: edge.size(),
                        image: image.clone_weak(),
                        uv_rect: Rect {
                            min: Vec2::ZERO,
                            max: Vec2::ONE,
                        },
                        clip: clip.map(|clip| clip.clip),
                        content_transform: Default::default(),
                    });
                }
            }
        }
    }
}

pub fn extract_uinodes(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    images: Extract<Res<Assets<Image>>>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<
            (
                &Node,
                &GlobalTransform,
                &BackgroundColor,
                Option<&UiImage>,
                &ComputedVisibility,
                Option<&CalculatedClip>,
                Option<&UiContentTransform>,
            ),
            Without<UiTextureAtlasImage>,
        >,
    >,
) {
    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, transform, color, maybe_image, visibility, clip, orientation)) =
            uinode_query.get(*entity)
        {
            // Skip invisible and completely transparent nodes
            if !visibility.is_visible() || color.0.a() == 0.0 {
                continue;
            }

            let mut transform = transform.compute_matrix();
            let size = uinode.calculated_size;

            let image = if let Some(image) = maybe_image {
                // Skip loading images
                if !images.contains(&image.texture) {
                    continue;
                }
                if let Some(orientation) = orientation {
                    transform *= Mat4::from(*orientation);

                    if orientation.is_sideways() {
                        let aspect = uinode.size().y / uinode.size().x;
                        transform *= Mat4::from_scale(Vec3::new(aspect, aspect.recip(), 1.));
                    }
                }
                image.texture.clone_weak()
            } else {
                DEFAULT_IMAGE_HANDLE.typed()
            };

            extracted_uinodes.uinodes.push(ExtractedUiNode {
                stack_index,
                transform,
                color: color.0,
                size,
                clip: clip.map(|clip| clip.clip),
                image,
                uv_rect: Rect {
                    min: Vec2::ZERO,
                    max: Vec2::ONE,
                },
                content_transform: orientation.copied().unwrap_or_default(),
            });
        };
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
        if let (
            Some(logical_size),
            Some(URect {
                min: physical_origin,
                ..
            }),
            Some(physical_size),
        ) = (
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
            &GlobalTransform,
            &Text,
            &TextLayoutInfo,
            &ComputedVisibility,
            Option<&CalculatedClip>,
            Option<&UiContentTransform>,
        )>,
    >,
) {
    // TODO: Support window-independent UI scale: https://github.com/bevyengine/bevy/issues/5621
    let scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.0)
        * ui_scale.scale;

    let inverse_scale_factor = (scale_factor as f32).recip();

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((
            uinode,
            global_transform,
            text,
            text_layout_info,
            visibility,
            clip,
            orientation,
        )) = uinode_query.get(*entity)
        {
            // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
            if !visibility.is_visible() || uinode.size().x == 0. || uinode.size().y == 0. {
                continue;
            }
            let mut transform = global_transform.compute_matrix();

            if let Some(orientation) = orientation {
                let mut size = uinode.size();
                if orientation.is_sideways() {
                    size = size.yx();
                }
                transform *=
                    Mat4::from(*orientation) * Mat4::from_translation(-0.5 * size.extend(0.));
            } else {
                transform *= Mat4::from_translation(-0.5 * uinode.size().extend(0.));
            }

            let mut color = Color::WHITE;
            let mut current_section = usize::MAX;
            for PositionedGlyph {
                position,
                size,
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
                uv_rect.min /= atlas.size;
                uv_rect.max /= atlas.size;
                extracted_uinodes.uinodes.push(ExtractedUiNode {
                    stack_index,
                    transform: transform
                        * Mat4::from_translation(position.extend(0.) * inverse_scale_factor),
                    color,
                    size: *size * inverse_scale_factor,
                    image: atlas.texture.clone_weak(),
                    uv_rect,
                    clip: clip.map(|clip| clip.clip),
                    content_transform: orientation.copied().unwrap_or_default(),
                });
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct UiVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub mode: u32,
}

#[derive(Resource)]
pub struct UiMeta {
    vertices: BufferVec<UiVertex>,
    view_bind_group: Option<BindGroup>,
}

impl Default for UiMeta {
    fn default() -> Self {
        Self {
            vertices: BufferVec::new(BufferUsages::VERTEX),
            view_bind_group: None,
        }
    }
}

const QUAD_VERTEX_POSITIONS: [Vec3; 4] = [
    Vec3::new(-0.5, -0.5, 0.0),
    Vec3::new(0.5, -0.5, 0.0),
    Vec3::new(0.5, 0.5, 0.0),
    Vec3::new(-0.5, 0.5, 0.0),
];

const QUAD_INDICES: [usize; 6] = [0, 2, 3, 0, 1, 2];

#[derive(Component)]
pub struct UiBatch {
    pub range: Range<u32>,
    pub image: Handle<Image>,
    pub z: f32,
}

const TEXTURED_QUAD: u32 = 0;
const UNTEXTURED_QUAD: u32 = 1;

pub fn prepare_uinodes(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
) {
    ui_meta.vertices.clear();

    // sort by ui stack index, starting from the deepest node
    extracted_uinodes
        .uinodes
        .sort_by_key(|node| node.stack_index);

    let mut start = 0;
    let mut end = 0;
    let mut current_batch_image = DEFAULT_IMAGE_HANDLE.typed();
    let mut last_z = 0.0;

    #[inline]
    fn is_textured(image: &Handle<Image>) -> bool {
        image.id() != DEFAULT_IMAGE_HANDLE.id()
    }

    for extracted_uinode in extracted_uinodes.uinodes.drain(..) {
        let mode = if is_textured(&extracted_uinode.image) {
            if current_batch_image.id() != extracted_uinode.image.id() {
                if is_textured(&current_batch_image) && start != end {
                    commands.spawn(UiBatch {
                        range: start..end,
                        image: current_batch_image,
                        z: last_z,
                    });
                    start = end;
                }
                current_batch_image = extracted_uinode.image.clone_weak();
            }
            TEXTURED_QUAD
        } else {
            // Untextured `UiBatch`es are never spawned within the loop.
            // If all the `extracted_uinodes` are untextured a single untextured UiBatch will be spawned after the loop terminates.
            UNTEXTURED_QUAD
        };

        //let uinode_rect = extracted_uinode.rect;

        let rect_size = extracted_uinode.size.extend(1.0);

        // Specify the corners of the node
        let positions = QUAD_VERTEX_POSITIONS
            .map(|pos| (extracted_uinode.transform * (pos * rect_size).extend(1.)).xyz());

        // Calculate the effect of clipping
        // Note: this won't work with rotation/scaling, but that's much more complex (may need more that 2 quads)
        let positions_diff = if let Some(clip) = extracted_uinode.clip {
            let resolve = |p, min, max| {
                if p < min {
                    min - p
                } else if max < p {
                    max - p
                } else {
                    0.
                }
            };
            let resolve_x = |x| resolve(x, clip.min.x, clip.max.x);
            let resolve_y = |y| resolve(y, clip.min.y, clip.max.y);
            let resolve_point = |p: Vec2| Vec2::new(resolve_x(p.x), resolve_y(p.y));
            [
                resolve_point(positions[0].truncate()),
                resolve_point(positions[1].truncate()),
                resolve_point(positions[2].truncate()),
                resolve_point(positions[3].truncate()),
            ]
        } else {
            [Vec2::ZERO; 4]
        };

        let positions_clipped = [
            positions[0] + positions_diff[0].extend(0.),
            positions[1] + positions_diff[1].extend(0.),
            positions[2] + positions_diff[2].extend(0.),
            positions[3] + positions_diff[3].extend(0.),
        ];

        // Don't try to cull nodes that have a rotation
        // In a rotation around the Z-axis, this value is 0.0 for an angle of 0.0 or π
        // In those two cases, the culling check can proceed normally as corners will be on
        // horizontal / vertical lines
        // For all other angles, bypass the culling check
        // This does not properly handles all rotations on all axis
        if extracted_uinode.transform.x_axis[1] == 0.0 {
            // Cull nodes that are completely clipped
            if positions_diff[0].x - positions_diff[1].x >= rect_size.x
                || positions_diff[1].y - positions_diff[2].y >= rect_size.y
            {
                continue;
            }
        }
        let mut positions_diff = positions_diff.map(|p| p / extracted_uinode.size);
        let uvs = if mode == UNTEXTURED_QUAD {
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]
        } else if extracted_uinode.clip.is_none() {
            extracted_uinode.uv_rect.vertices()
        } else {
            let uv_rect = extracted_uinode.uv_rect;
            let mut min = Vec2::MAX;
            let mut max = Vec2::MIN;
            for p in &mut positions_diff {
                if p.x < min.x {
                    min.x = p.x;
                }
                if p.y < min.y {
                    min.y = p.y;
                }
                if p.x > max.x {
                    max.x = p.x;
                }
                if p.y > max.y {
                    max.y = p.y;
                }
            }
            [
                Vec2::new(
                    uv_rect.min.x + positions_diff[0].x,
                    uv_rect.min.y + positions_diff[0].y,
                ),
                Vec2::new(
                    uv_rect.max.x + positions_diff[1].x,
                    uv_rect.min.y + positions_diff[1].y,
                ),
                Vec2::new(
                    uv_rect.max.x + positions_diff[2].x,
                    uv_rect.max.y + positions_diff[2].y,
                ),
                Vec2::new(
                    uv_rect.min.x + positions_diff[3].x,
                    uv_rect.max.y + positions_diff[3].y,
                ),
            ]
        };

        let color = extracted_uinode.color.as_linear_rgba_f32();
        for i in QUAD_INDICES {
            ui_meta.vertices.push(UiVertex {
                position: positions_clipped[i].into(),
                uv: uvs[i].into(),
                color,
                mode,
            });
        }

        last_z = extracted_uinode.transform.w_axis[2];
        end += QUAD_INDICES.len() as u32;
    }

    // if start != end, there is one last batch to process
    if start != end {
        commands.spawn(UiBatch {
            range: start..end,
            image: current_batch_image,
            z: last_z,
        });
    }

    ui_meta.vertices.write_buffer(&render_device, &render_queue);
}


pub fn prepare_uinodes_debug(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
) {
    //println!("prepare_uinodes");
    ui_meta.vertices.clear();

    // sort by ui stack index, starting from the deepest node
    extracted_uinodes
        .uinodes
        .sort_by_key(|node| node.stack_index);

    let mut start = 0;
    let mut end = 0;
    let mut current_batch_image = DEFAULT_IMAGE_HANDLE.typed();
    let mut last_z = 0.0;

    #[inline]
    fn is_textured(image: &Handle<Image>) -> bool {
        image.id() != DEFAULT_IMAGE_HANDLE.id()
    }

    for extracted_uinode in extracted_uinodes.uinodes.drain(..) {
        let mode = if is_textured(&extracted_uinode.image) {
            if current_batch_image.id() != extracted_uinode.image.id() {
                if is_textured(&current_batch_image) && start != end {
                    commands.spawn(UiBatch {
                        range: start..end,
                        image: current_batch_image,
                        z: last_z,
                    });
                    start = end;
                }
                current_batch_image = extracted_uinode.image.clone_weak();
            }
            TEXTURED_QUAD
        } else {
            // Untextured `UiBatch`es are never spawned within the loop.
            // If all the `extracted_uinodes` are untextured a single untextured UiBatch will be spawned after the loop terminates.
            UNTEXTURED_QUAD
        };

        let size = extracted_uinode.transform.transform_vector3(extracted_uinode.size.extend(0.)).xy().abs();
        let center = extracted_uinode.transform.transform_point3(Vec3::ZERO).xy().abs();
        let rect = Rect::from_center_size(center, size);
        let uv_rect = extracted_uinode.uv_rect;

        let (clipped_rect, mut clipped_uv_rect) = 
            if let Some(clip_rect) = extracted_uinode.clip {
                let target_rect = rect.intersect(clip_rect);
                let min = (target_rect.min - rect.min) / size;
                let max = (target_rect.max - rect.min) / size;
                let normed_target = Rect {
                    min, max
                };
                let uv_size = uv_rect.size();
                let clipped_uv_size = uv_size * normed_target.size();
                let clipped_uv_rect = if extracted_uinode.content_transform.is_flipped() {
                    
                    let clipped_uv_min = Vec2::new(
                         uv_rect.max.x - uv_size.x * min.x,
                         uv_rect.min.y + uv_size.y * min.y,
                    );
                    let clipped_uv_max = Vec2::new(
                        clipped_uv_min.x - clipped_uv_size.x,
                        clipped_uv_min.y + clipped_uv_size.y,
                    );
                    
                    Rect {
                        min: clipped_uv_min,
                        max: clipped_uv_max,
                    }
                } else {
                    let clipped_uv_min = uv_rect.min + uv_size * min;
                    let clipped_uv_max = clipped_uv_min + clipped_uv_size;
                    Rect {
                        min: clipped_uv_min,
                        max: clipped_uv_max,
                    }
                };
                (target_rect, clipped_uv_rect)
            } else {
                (rect, extracted_uinode.uv_rect)
            };
            
        
            let color = extracted_uinode.color.as_linear_rgba_f32();
            let positions = clipped_rect.vertices();
            if extracted_uinode.content_transform.is_flipped() {
                if  extracted_uinode.content_transform.is_sideways() {
                    let tx = clipped_uv_rect.min.y;
                    clipped_uv_rect.min.y = clipped_uv_rect.max.y;
                    clipped_uv_rect.max.y = tx;
                } else {
                    let tx = clipped_uv_rect.min.x;
                    clipped_uv_rect.min.x = clipped_uv_rect.max.x;
                    clipped_uv_rect.max.x = tx;
                }
            }
            let mut uvs = clipped_uv_rect.vertices();
            use UiContentTransform::*;
            match extracted_uinode.content_transform {
                North | FlippedNorth => {}
                East | FlippedEast => uvs.rotate_right(1),
                South | FlippedSouth => uvs.rotate_right(2),
                West | FlippedWest => uvs.rotate_right(3),
            }
            
          //  println!("\tpositions: {:?}", positions);
          //  println!("\tuvs: {:?}", uvs);
            for i in QUAD_INDICES {            
                ui_meta.vertices.push(UiVertex {
                    position: positions[i].extend(0.).into(),
                    uv: uvs[i].into(),
                    color,
                    mode,
                });
            }
           // println!("extracted");
           // println!();
    
    
            last_z = extracted_uinode.transform.w_axis[2];
            end += QUAD_INDICES.len() as u32;
        }
    
        // if start != end, there is one last batch to process
        if start != end {
            commands.spawn(UiBatch {
                range: start..end,
                image: current_batch_image,
                z: last_z,
            });
        }
    
        ui_meta.vertices.write_buffer(&render_device, &render_queue);
    
        
        // println!();
        // println!();
    }
/*

        // Specify the corners of the node
        let positions = QUAD_VERTEX_POSITIONS
            .map(|pos| (extracted_uinode.transform * (pos * rect_size).extend(1.)).xyz());

        // Calculate the effect of clipping
        // Note: this won't work with rotation/scaling, but that's much more complex (may need more that 2 quads)
        let positions_diff = if let Some(clip) = extracted_uinode.clip {
            let resolve = |p, min, max| {
                if p < min {
                    min - p
                } else if max < p {
                    max - p
                } else {
                    0.
                }
            };
            let resolve_x = |x| resolve(x, clip.min.x, clip.max.x);
            let resolve_y = |y| resolve(y, clip.min.y, clip.max.y);
            let resolve_point = |p: Vec2| Vec2::new(resolve_x(p.x), resolve_y(p.y));
            [
                resolve_point(positions[0].truncate()),
                resolve_point(positions[1].truncate()),
                resolve_point(positions[2].truncate()),
                resolve_point(positions[3].truncate()),
            ]
        } else {
            [Vec2::ZERO; 4]
        };

        println!("\tpositions diff: {:?}", positions_diff);

        let positions_clipped = [
            positions[0] + positions_diff[0].extend(0.),
            positions[1] + positions_diff[1].extend(0.),
            positions[2] + positions_diff[2].extend(0.),
            positions[3] + positions_diff[3].extend(0.),
        ];

        println!("\tpositions clipped: {:?}", positions_clipped);

        let rect = Rect::from_center_size(extracted_uinode.transform.to_scale_rotation_translation().2.xy(), rect_size.truncate());
        let target = 
            if let Some(clip) = extracted_uinode.clip {
                rect.intersect(clip)
            } else {
                rect
            };
        
        let norm_rect = Rect {
            min: 0.5 + (target.min - target.center()) / rect.size(),
            max: 0.5 + (target.max - target.center()) / rect.size(),
        };
        println!("\ttarget_rect: {:?}", target);
        println!("\tnorm_rect: {:?}", norm_rect);

        // Don't try to cull nodes that have a rotation
        // In a rotation around the Z-axis, this value is 0.0 for an angle of 0.0 or π
        // In those two cases, the culling check can proceed normally as corners will be on
        // horizontal / vertical lines
        // For all other angles, bypass the culling check
        // This does not properly handles all rotations on all axis
        if extracted_uinode.transform.x_axis[1] == 0.0 {
            // Cull nodes that are completely clipped
            if positions_diff[0].x - positions_diff[1].x >= rect_size.x
                || positions_diff[1].y - positions_diff[2].y >= rect_size.y
            {
                continue;
            }
        }
        let mut positions_diff = positions_diff.map(|p| p / extracted_uinode.size);
        let uvs = if mode == UNTEXTURED_QUAD {
            println!("\tUV unit");
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]
        } else if extracted_uinode.clip.is_none() {
            println!("\tuvs from rect: {:?}", extracted_uinode.uv_rect);
            extracted_uinode.uv_rect.vertices()
        } else {
            let uv_rect = extracted_uinode.uv_rect;
            let uv_size = uv_rect.size();
            let min = uv_rect.min + uv_size * norm_rect.min;
            let max = min + uv_size * norm_rect.size();
            Rect { min, max }.vertices()
        };

        let color = extracted_uinode.color.as_linear_rgba_f32();
        println!("\tpositions: {:?}", positions_clipped);
        println!("\tuvs: {:?}", uvs);
        for i in QUAD_INDICES {            
            ui_meta.vertices.push(UiVertex {
                position: positions_clipped[i].into(),
                uv: uvs[i].into(),
                color,
                mode,
            });
        }
        println!("extracted");
        println!();


        last_z = extracted_uinode.transform.w_axis[2];
        end += QUAD_INDICES.len() as u32;
    }

    // if start != end, there is one last batch to process
    if start != end {
        commands.spawn(UiBatch {
            range: start..end,
            image: current_batch_image,
            z: last_z,
        });
    }

    ui_meta.vertices.write_buffer(&render_device, &render_queue);

    
    println!();
    println!();
}
*/


pub fn prepare_uinodes_3(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
) {
    //println!("prepare_uinodes");
    ui_meta.vertices.clear();

    // sort by ui stack index, starting from the deepest node
    extracted_uinodes
        .uinodes
        .sort_by_key(|node| node.stack_index);

    let mut start = 0;
    let mut end = 0;
    let mut current_batch_image = DEFAULT_IMAGE_HANDLE.typed();
    let mut last_z = 0.0;

    #[inline]
    fn is_textured(image: &Handle<Image>) -> bool {
        image.id() != DEFAULT_IMAGE_HANDLE.id()
    }

    for extracted_uinode in extracted_uinodes.uinodes.drain(..) {
        let mode = if is_textured(&extracted_uinode.image) {
            if current_batch_image.id() != extracted_uinode.image.id() {
                if is_textured(&current_batch_image) && start != end {
                    commands.spawn(UiBatch {
                        range: start..end,
                        image: current_batch_image,
                        z: last_z,
                    });
                    start = end;
                }
                current_batch_image = extracted_uinode.image.clone_weak();
            }
            TEXTURED_QUAD
        } else {
            // Untextured `UiBatch`es are never spawned within the loop.
            // If all the `extracted_uinodes` are untextured a single untextured UiBatch will be spawned after the loop terminates.
            UNTEXTURED_QUAD
        };
        

        let size = 
            if extracted_uinode.content_transform.is_sideways() {
                extracted_uinode.size.yx()
            } else {
                extracted_uinode.size
            };
        let mut uv_rect = extracted_uinode.uv_rect;
        if extracted_uinode.content_transform.is_flipped() {
            swap(&mut uv_rect.min.x, &mut uv_rect.max.x);    
        }
        let mut uvs = extracted_uinode.uv_rect.vertices();

        use UiContentTransform::*;
        match extracted_uinode.content_transform {
            North | FlippedNorth => {}
            East | FlippedEast => uvs.rotate_right(1),
            South | FlippedSouth => uvs.rotate_right(2),
            West | FlippedWest => uvs.rotate_right(3),
        }

        let size = extracted_uinode.transform.transform_vector3(size.extend(0.)).xy().abs();
        
        let center = extracted_uinode.transform.transform_point3(Vec3::ZERO).xy().abs();
        let rect = Rect::from_center_size(center, size);
       

        let (clipped_rect, mut clipped_uv_rect) = 
            if let Some(clip_rect) = extracted_uinode.clip {
                let target_rect = rect.intersect(clip_rect);
                let d_min = (target_rect.min - rect.min) / size;
                let d_max = (rect.max - target_rect.max) / size;
                let diffs = [d_min, Vec2::new(d_max.x, d_min.y), d_max, Vec2::new(d_min.x, d_max.y)];
                let uvs = [
                    uvs[0]
                ];

                let min = (target_rect.min - rect.min) / size;
                let max = (target_rect.max - rect.min) / size;
                let normed_target = Rect {
                    min, max
                };
                let uv_size = uv_rect.size();
                let clipped_uv_size = uv_size * normed_target.size();
                let clipped_uv_rect = if extracted_uinode.content_transform.is_flipped() {
                    
                    let clipped_uv_min = Vec2::new(
                         uv_rect.max.x - clipped_uv_size.x, //uv_size.x * max.x,
                         uv_rect.min.y + uv_size.y * min.y,
                    );
                    let clipped_uv_max = Vec2::new(
                        clipped_uv_min.x + clipped_uv_size.x,
                        clipped_uv_min.y + clipped_uv_size.y,
                    );
                    
                    Rect {
                        min: clipped_uv_min,
                        max: clipped_uv_max,
                    }
                } else {
                    let clipped_uv_min = uv_rect.min + uv_size * min;
                    let clipped_uv_max = clipped_uv_min + clipped_uv_size;
                    Rect {
                        min: clipped_uv_min,
                        max: clipped_uv_max,
                    }
                };
                (target_rect, clipped_uv_rect)
            } else {
                (rect, extracted_uinode.uv_rect)
            };
            
        
            let color = extracted_uinode.color.as_linear_rgba_f32();
            let positions = clipped_rect.vertices();
            if extracted_uinode.content_transform.is_flipped() {
                if  extracted_uinode.content_transform.is_sideways() {
                    let tx = clipped_uv_rect.min.y;
                    clipped_uv_rect.min.y = clipped_uv_rect.max.y;
                    clipped_uv_rect.max.y = tx;
                } else {
                    let tx = clipped_uv_rect.min.x;
                    clipped_uv_rect.min.x = clipped_uv_rect.max.x;
                    clipped_uv_rect.max.x = tx;
                }
            }
            let mut uvs = clipped_uv_rect.vertices();
           
            
            for i in QUAD_INDICES {            
                ui_meta.vertices.push(UiVertex {
                    position: positions[i].extend(0.).into(),
                    uv: uvs[i].into(),
                    color,
                    mode,
                });
            }
    
    
            last_z = extracted_uinode.transform.w_axis[2];
            end += QUAD_INDICES.len() as u32;
        }
    
        if start != end {
            commands.spawn(UiBatch {
                range: start..end,
                image: current_batch_image,
                z: last_z,
            });
        }
    
        ui_meta.vertices.write_buffer(&render_device, &render_queue);

    }


    pub fn prepare_uinodes_4(
        mut commands: Commands,
        render_device: Res<RenderDevice>,
        render_queue: Res<RenderQueue>,
        mut ui_meta: ResMut<UiMeta>,
        mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    ) {
        //println!("prepare_uinodes");
        ui_meta.vertices.clear();
    
        // sort by ui stack index, starting from the deepest node
        extracted_uinodes
            .uinodes
            .sort_by_key(|node| node.stack_index);
    
        let mut start = 0;
        let mut end = 0;
        let mut current_batch_image = DEFAULT_IMAGE_HANDLE.typed();
        let mut last_z = 0.0;
    
        #[inline]
        fn is_textured(image: &Handle<Image>) -> bool {
            image.id() != DEFAULT_IMAGE_HANDLE.id()
        }
    
        for extracted_uinode in extracted_uinodes.uinodes.drain(..) {
            let mode = if is_textured(&extracted_uinode.image) {
                if current_batch_image.id() != extracted_uinode.image.id() {
                    if is_textured(&current_batch_image) && start != end {
                        commands.spawn(UiBatch {
                            range: start..end,
                            image: current_batch_image,
                            z: last_z,
                        });
                        start = end;
                    }
                    current_batch_image = extracted_uinode.image.clone_weak();
                }
                TEXTURED_QUAD
            } else {
                UNTEXTURED_QUAD
            };
            
    
            let size = extracted_uinode.size;
    
            let center = extracted_uinode.transform.transform_point3(Vec3::ZERO).xy();
          
          
            
            let rect = Rect::from_center_size(center, size);

            let [positions, uvs] =
                calculate_vertices_combined(rect, extracted_uinode.clip, extracted_uinode.uv_rect, extracted_uinode.content_transform.rotations(), extracted_uinode.content_transform.is_flipped());
            
                
            
                let color = extracted_uinode.color.as_linear_rgba_f32();
    
                
                for i in QUAD_INDICES {            
                    ui_meta.vertices.push(UiVertex {
                        position: positions[i].extend(0.).into(),
                        uv: uvs[i].into(),
                        color,
                        mode,
                    });
                }
        
        
                last_z = extracted_uinode.transform.w_axis[2];
                end += QUAD_INDICES.len() as u32;
            }
        
            if start != end {
                commands.spawn(UiBatch {
                    range: start..end,
                    image: current_batch_image,
                    z: last_z,
                });
            }
        
            ui_meta.vertices.write_buffer(&render_device, &render_queue);
    
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
            let pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey { hdr: view.hdr },
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
                transparent_phase.add(TransparentUi {
                    draw_function: draw_ui_function,
                    pipeline,
                    entity,
                    sort_key: FloatOrd(batch.z),
                });
            }
        }
    }
}




fn calculate_vertices(
    node_rect: Rect,
    clip_rect: Option<Rect>,
    uv_rect: Rect
) -> [[Vec2; 4]; 2] {
    let target = 
        clip_rect.map(|clip| node_rect.intersect(clip))
        .unwrap_or(node_rect);
    
    let r_0 = node_rect.min;
    let r_s = node_rect.size();
    let t_0 = target.min;
    let t_s = target.size();

    let e_0 = uv_rect.min;
    let e_s = uv_rect.size();//.yx();

    let d_0 = e_s * (t_0 - r_0) / r_s;
        
    let f_0 = e_0 + d_0;
    let f_s = (t_s / r_s) * e_s;
    let f_2 = f_0 + f_s;

    println!("r_0: {r_0}");
    println!("r_s: {r_s}");
    println!("t_0: {t_0}");
    println!("t_s: {t_s}");
    println!("e_0: {e_0}");
    println!("e_s: {e_s}");
    println!("e_0: {e_0}");
    println!("f_0: {f_0}");
    println!("f_2: {f_2}");
    
    let uvs =     [
        f_0,
        vec2(f_2.x, f_0.y),
        f_2,
        vec2(f_0.x, f_2.y),
    ];
    println!("uvs: {uvs:?}");
    println!("ps: {:?}", target.vertices());

    [
        target.vertices(),
        uvs
    ]
}

fn calculate_vertices_flipped(
    node_rect: Rect,
    clip_rect: Option<Rect>,
    uv_rect: Rect
) -> [[Vec2; 4]; 2] 
{
    let target = 
    clip_rect.map(|clip| node_rect.intersect(clip))
    .unwrap_or(node_rect);

    let r_0 = node_rect.min;
    let r_s = node_rect.size();
    let t_0 = target.min;
    let t_s = target.size();

    let e_0 = vec2(uv_rect.max.x, uv_rect.min.y);
    let e_s = uv_rect.size();

    let d_0 = e_s * (t_0 - r_0) / r_s;
    let f_0 = e_0 - d_0;
    let f_s =  (t_s / r_s) * e_s;
    let f_2 = f_0 + vec2(-f_s.x, f_s.y);

    let uvs =     [
        f_0,
        vec2(f_2.x, f_0.y),
        f_2,
        vec2(f_0.x, f_2.y),
    ];

    [
        target.vertices(),
        uvs
    ]
}



fn calculate_vertices_rotated(
    node_rect: Rect,
    clip_rect: Option<Rect>,
    uv_rect: Rect,
    n: u8,
) -> [[Vec2; 4]; 2] {
    let [positions, mut uvs] = calculate_vertices(node_rect, clip_rect, uv_rect);

    uvs.rotate_right(n as usize);
    [positions, uvs]
}

fn calculate_vertices_combined(
    mut node_rect: Rect,
    mut clip_rect: Option<Rect>,
    mut uv_rect: Rect,
    n: u8,
    flip: bool
) -> [[Vec2; 4]; 2] {
    if n == 1 || n == 3 {
        // uv_rect.min = uv_rect.min.yx();
        // uv_rect.max = uv_rect.max.yx();
        // if let Some(mut clip) = clip_rect {
        //     // clip.min = clip.min.yx();
        //     // clip.max = clip.max.yx();
        //     // clip_rect = Some(clip);
        // }
    }

    let [positions, mut uvs] = 
        if flip {
            calculate_vertices_flipped(node_rect, clip_rect, uv_rect)
        } else {
            calculate_vertices(node_rect, clip_rect, uv_rect)
        };
    

    uvs.rotate_right(n as usize);
    [positions, uvs]
}

#[cfg(test)]
mod tests {
    use bevy_math::Rect;
    use bevy_math::Vec2;
    use bevy_math::vec2;

    use crate::render::calculate_vertices_combined;
    use crate::render::calculate_vertices_flipped;

    use super::calculate_vertices;

    const EPSILON: f32 = 0.0001;
   
    macro_rules! assert_approx_eq_arr {
        ($a:expr, $b:expr) => {
            assert!(
                $a.len() == $b.len(),
                "assertion failed: `(left.len() != right.len())` (left: `{:?}`, right: `{:?}`)",
                $a,
                $b
            );
            for (index, (left, right)) in $a.iter().zip($b.iter()).enumerate() {
                assert!(
                    left.abs_diff_eq(*right, EPSILON),
                    "assertion failed at {index}: `(left != right)` (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
                    left,
                    right,
                    EPSILON
                );
            }
        };
    }
    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr) => {
            
            assert!(
                $a.abs_diff_eq($b, EPSILON),
                "assertion failed: `(left != right)` (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
                $a,
                $b,
                EPSILON
            )
        };
       
        
    }
    

    #[test]
    fn node() {
        let node_rect = Rect { min: Vec2::ZERO, max: vec2(200., 100.) };
        let clip_rect = Rect { min: vec2(50., 25.), max: vec2(150., 50.) };
        let uv_rect = Rect { min: vec2(0.5, 0.25), max: vec2(1.0, 0.5) };
        let [ps, uvs] = calculate_vertices(node_rect, Some(clip_rect), uv_rect);
        assert_approx_eq!(ps[0], clip_rect.min);
        assert_approx_eq!(ps[1], vec2(clip_rect.max.x, clip_rect.min.y));
        assert_approx_eq!(ps[2], clip_rect.max);
        assert_approx_eq!(ps[3], vec2(clip_rect.min.x, clip_rect.max.y));
        assert_approx_eq_arr!(uvs, [
            vec2(0.625, 0.3125),
            vec2(0.625 + 0.25, 0.3125),
            vec2(0.625 + 0.25, 0.3125 + 1. / 16.),
            vec2(0.625, 0.3125 + 1. / 16.),
        ]);
    }

    fn x(v: Vec2) -> Vec2 {
        v.x * Vec2::X
    }

    fn y(v: Vec2) -> Vec2 {
        v.y * Vec2::Y
    }

    #[test]
    fn node1() {
        let node_rect = Rect { min: Vec2::ZERO, max: vec2(100., 100.) };
        let clip_rect = Rect { min: Vec2::ZERO, max: vec2(150., 250.) };
        let uv_rect = Rect { min: Vec2::ZERO, max: Vec2::ONE };
        let [ps, uvs] = calculate_vertices(node_rect, Some(clip_rect), uv_rect);
        assert_approx_eq!(ps[0], Vec2::ZERO);
        assert_approx_eq!(ps[1], x(node_rect.max));
        assert_approx_eq!(ps[2], node_rect.max);
        assert_approx_eq!(ps[3], y(node_rect.max));
        assert_approx_eq!(uvs[0], Vec2::ZERO);
        assert_approx_eq!(uvs[1], Vec2::new(1., 0.));
        assert_approx_eq!(uvs[2], Vec2::ONE);
        assert_approx_eq!(uvs[3], Vec2::new(0., 1.));
    }

    
    #[test]
    fn node2() {
        let node_rect = Rect { min: Vec2::ZERO, max: vec2(100., 100.) };
        let clip_rect = Rect { min: Vec2::ZERO, max: vec2(50., 100.) };
        let uv_rect = Rect { min: Vec2::ZERO, max: Vec2::ONE };
        let [ps, uvs] = calculate_vertices(node_rect, Some(clip_rect), uv_rect);
        assert_approx_eq!(ps[0], Vec2::ZERO);
        assert_approx_eq!(ps[1], vec2(50., 0.));
        assert_approx_eq!(ps[2], vec2(50., 100.));
        assert_approx_eq!(ps[3], vec2(0., 100.));
        assert_approx_eq!(uvs[0], Vec2::ZERO);
        assert_approx_eq!(uvs[1], Vec2::new(0.5, 0.));
        assert_approx_eq!(uvs[2], vec2(0.5, 1.0));
        assert_approx_eq!(uvs[3], Vec2::new(0., 1.));
    }

    #[test]
    fn node3() {
        let node_rect = Rect { min: Vec2::ZERO, max: vec2(100., 100.) };
        let clip_rect = Rect { min: Vec2::ZERO, max: vec2(100., 50.) };
        let uv_rect = Rect { min: Vec2::ZERO, max: Vec2::ONE };
        let [ps, uvs] = calculate_vertices(node_rect, Some(clip_rect), uv_rect);
        assert_approx_eq!(ps[0], Vec2::ZERO);
        assert_approx_eq!(ps[1], vec2(100., 0.));
        assert_approx_eq!(ps[2], vec2(100., 50.));
        assert_approx_eq!(ps[3], vec2(0., 50.));
        assert_approx_eq!(uvs[0], Vec2::ZERO);
        assert_approx_eq!(uvs[1], Vec2::new(1.0, 0.));
        assert_approx_eq!(uvs[2], vec2(1.0, 0.5));
        assert_approx_eq!(uvs[3], Vec2::new(0., 0.5));
    }

    
    #[test]
    fn node4() {
        let node_rect = Rect { min: Vec2::ZERO, max: vec2(400., 100.) };
        let clip_rect = Rect { min: vec2(100., 0.), max: vec2(200., 100.) };
        let uv_rect = Rect { min: Vec2::ZERO, max: Vec2::ONE };
        let [ps, uvs] = calculate_vertices(node_rect, Some(clip_rect), uv_rect);
        assert_approx_eq!(ps[0], vec2(100., 0.));
        assert_approx_eq!(ps[1], vec2(200., 0.));
        assert_approx_eq!(ps[2], vec2(200., 100.));
        assert_approx_eq!(ps[3], vec2(100., 100.));
        assert_approx_eq!(uvs[0], vec2(0.25, 0.));
        assert_approx_eq!(uvs[1], vec2(0.5, 0.));
        assert_approx_eq!(uvs[2], vec2(0.5, 1.));
        assert_approx_eq!(uvs[3], Vec2::new(0.25, 1.));
    }

    #[test]
    fn node5() {
        let node_rect = Rect { min: Vec2::ZERO, max: vec2(200., 100.) };
        let clip_rect = Rect { min: vec2(50., 25.), max: vec2(150., 50.) };
        let uv_rect = Rect { min: Vec2::ZERO, max: Vec2::ONE };
        let [ps, uvs] = calculate_vertices(node_rect, Some(clip_rect), uv_rect);
        assert_approx_eq!(ps[0], clip_rect.min);
        assert_approx_eq!(ps[1], vec2(clip_rect.max.x, clip_rect.min.y));
        assert_approx_eq!(ps[2], clip_rect.max);
        assert_approx_eq!(ps[3], vec2(clip_rect.min.x, clip_rect.max.y));
        assert_approx_eq!(uvs[0], vec2(0.25, 0.25));
        assert_approx_eq!(uvs[1], vec2(0.75, 0.25));
        assert_approx_eq!(uvs[2], vec2(0.75, 0.5));
        assert_approx_eq!(uvs[3], vec2(0.25, 0.5));
    }

    #[test]
    fn nodey_flip() {
        let node_rect = Rect::new(0., 0., 100., 100.);
        let clip_rect = Rect::new(50., 0., 100., 100.);
        let uv = Rect::new(0., 0., 1., 1.);
        let [ps, uvs] = calculate_vertices_flipped(node_rect, Some(clip_rect), uv);
        assert_approx_eq_arr!(ps, [clip_rect.min, vec2(clip_rect.max.x, clip_rect.min.y), clip_rect.max, vec2(clip_rect.min.x, clip_rect.max.y),]);
        assert_approx_eq_arr!(uvs, [vec2(0.5, 0.), vec2(0., 0.), vec2(0., 1.), vec2(0.5, 1.)]);    
    }

    #[test]
    fn nodey_rot() {
        let node_rect = Rect::new(0., 0., 100., 100.);
        let clip_rect = Rect::new(0., 0., 50., 100.);
        let uv = Rect::new(0., 0., 1., 1.);
        let [ps, uvs] = calculate_vertices_combined(node_rect, Some(clip_rect), uv, 1, false);
        assert_approx_eq_arr!(ps, [clip_rect.min, vec2(clip_rect.max.x, clip_rect.min.y), clip_rect.max, vec2(clip_rect.min.x, clip_rect.max.y),]);
        assert_approx_eq_arr!(uvs, [vec2(1., 0.5), vec2(1., 1.), vec2(0., 1.), vec2(0., 0.5)]);    
    }

    #[test]
    fn atlas_east_clipped() {
        let node_rect = Rect::new(0.,0., 128., 128.);
        let clip_rect = Rect::new(0., 0., 96., 48.);
        let uv = Rect::new(0., 0., 128. / 256., 64. / 256.);
        let [ps, vs] = calculate_vertices_combined(node_rect, Some(clip_rect), uv, 1, false);
        println!("{:?}", ps);
        println!("{:?}", vs);
    }
}
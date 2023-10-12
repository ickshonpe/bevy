mod pipeline;
mod render_pass;

use bevy_core_pipeline::{core_2d::Camera2d, core_3d::Camera3d};
use bevy_render::{ExtractSchedule, Render};
use bevy_window::{PrimaryWindow, Window};
pub use pipeline::*;
pub use render_pass::*;

use crate::Outline;
use crate::{
    prelude::UiCameraConfig, BackgroundColor, BorderColor, CalculatedClip, ContentSize, Node,
    Style, UiImage, UiScale, UiStack, UiTextureAtlasImage, Val,
};

use bevy_app::prelude::*;
use bevy_asset::{load_internal_asset, AssetEvent, Assets, Handle, HandleUntyped};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Rect, UVec4, Vec2, Vec3, Vec3Swizzles};
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
                #[cfg(feature = "bevy_text")]
                extract_text_uinodes.after(RenderUiSystem::ExtractAtlasNode),
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

pub struct ExtractedUiNode {
    pub stack_index: usize,
    pub color: Color,
    pub position: Vec2,
    pub size: Vec2,
    pub uv_rect: Rect,
    pub image: Handle<Image>,
    pub clip: Option<Rect>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub border: [f32; 4],
    pub radius: [f32; 4],
    pub border_color: Color,
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
            ),
            Without<UiImage>,
        >,
    >,
) {
    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, transform, color, visibility, clip, texture_atlas_handle, atlas_image)) =
            uinode_query.get(*entity)
        {
            // Skip invisible and completely transparent nodes
            if !visibility.is_visible() || color.0.a() == 0.0 {
                continue;
            }

            let (mut atlas_rect, mut atlas_size, image) =
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

            let target = Rect::from_center_size(transform.translation().xy(), uinode.size());
            extracted_uinodes.uinodes.push(ExtractedUiNode {
                stack_index,
                color: color.0,
                position: uinode.position(),
                size: uinode.size(),
                clip: clip.map(|clip| clip.clip),
                image,
                uv_rect: atlas_rect,
                flip_x: atlas_image.flip_x,
                flip_y: atlas_image.flip_y,
                border: [0.; 4],
                radius: [0.; 4],
                border_color: Color::NONE,
            });
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
                &BackgroundColor,
                Option<&BorderColor>,
                Option<&Outline>,
                Option<&UiImage>,
                &ComputedVisibility,
                Option<&CalculatedClip>,
            ),
            Without<UiTextureAtlasImage>,
        >,
    >,
) {
    extracted_uinodes.uinodes.clear();

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((
            uinode,
            color,
            maybe_border_color,
            maybe_outline,
            maybe_image,
            visibility,
            clip,
        )) = uinode_query.get(*entity)
        {
            let border_color = maybe_border_color.map(|border_color| border_color.0).unwrap_or(Color::NONE);

            // Skip invisible and completely transparent nodes
            if visibility.is_visible() || !(color.0.a() == 0.0 && border_color.a() == 0.0) {
                let (image, flip_x, flip_y) = if let Some(image) = maybe_image {
                    // Skip loading images
                    if !images.contains(&image.texture) {
                        continue;
                    }
                    (image.texture.clone_weak(), image.flip_x, image.flip_y)
                } else {
                    (DEFAULT_IMAGE_HANDLE.typed(), false, false)
                };

                extracted_uinodes.uinodes.push(ExtractedUiNode {
                    stack_index,
                    color: color.0,
                    position: uinode.position(),
                    size: uinode.size(),
                    uv_rect: Rect::new(0., 0., 1., 1.),
                    clip: clip.map(|clip| clip.clip),
                    image: image.clone(),
                    flip_x,
                    flip_y,
                    border: uinode.border,
                    radius: uinode.border_radius,
                    border_color,
                });
            }

            if let Some(outline) = maybe_outline {
                extracted_uinodes.uinodes.push(ExtractedUiNode {
                    stack_index,
                    color: Color::NONE,
                    position: uinode.position() - Vec2::splat(uinode.outline_offset + uinode.outline_width),
                    size: uinode.size() + 2. * (uinode.outline_width + uinode.outline_offset),
                    uv_rect: Rect::new(0., 0., 1., 1.),
                    clip: clip.map(|clip| clip.clip),
                    image: DEFAULT_IMAGE_HANDLE.typed().clone(),
                    flip_x,
                    flip_y,
                    border: [uinode.outline_width; 4],
                    radius: uinode.border_radius,
                    border_color: outline.color,
                });
            }
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
    let scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.0)
        * ui_scale.scale;

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

                extracted_uinodes.uinodes.push(ExtractedUiNode {
                    stack_index,
                    position,
                    size: scaled_glyph_size,
                    color,
                    image: atlas.texture.clone_weak(),
                    uv_rect,
                    clip: clip.map(|clip| clip.clip),
                    flip_x: false,
                    flip_y: false,
                    border: [0.; 4],
                    radius: [0.; 4],
                    border_color: Color::NONE,
                });
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct UiInstance {
    pub i_location: [f32; 2],
    pub i_size: [f32; 2],
    pub i_uv_min: [f32; 2],
    pub i_uv_size: [f32; 2],
    pub i_color: [f32; 4],
    pub i_radius: [f32; 4],
    pub i_border: [f32; 4],
    pub i_flags: u32,
    pub i_border_color: [f32; 4],
}

impl UiInstance {
    #[inline]
    fn from(
        location: Vec2,
        size: Vec2,
        uv_rect: Rect,
        color: Color,
        mode: u32,
        radius: [f32; 4],
        border: [f32; 4],
        border_color: Color,
    ) -> Self {
        Self {
            i_location: location.into(),
            i_size: size.into(),
            i_uv_min: uv_rect.min.into(),
            i_uv_size: uv_rect.size().into(),
            i_color: color.as_linear_rgba_f32(),
            i_radius: radius,
            i_border: border,
            i_flags: mode,
            i_border_color: border_color.as_linear_rgba_f32(),
        }
    }
}

#[derive(Resource)]
pub struct UiMeta {
    view_bind_group: Option<BindGroup>,
    index_buffer: BufferVec<u32>,
    instance_buffer: BufferVec<UiInstance>,
}

impl Default for UiMeta {
    fn default() -> Self {
        Self {
            view_bind_group: None,
            index_buffer: BufferVec::<u32>::new(BufferUsages::INDEX),
            instance_buffer: BufferVec::<UiInstance>::new(BufferUsages::VERTEX),
        }
    }
}

#[derive(Component)]
pub struct UiBatch {
    pub range: Range<u32>,
    pub image: Handle<Image>,
    pub z: f32,
}

const UNTEXTURED_QUAD: u32 = 0;
const TEXTURED_QUAD: u32 = 1;

pub fn prepare_uinodes(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
) {
    ui_meta.instance_buffer.clear();

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

    for extracted_uinode in &extracted_uinodes.uinodes {
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

        ui_meta.instance_buffer.push(UiInstance::from(
            extracted_uinode.position,
            extracted_uinode.size,
            extracted_uinode.uv_rect,
            extracted_uinode.color,
            mode,
            extracted_uinode.radius,
            extracted_uinode.border,
            extracted_uinode.border_color,
        ));

        last_z = extracted_uinode.stack_index as f32;
        end += 1;
    }

    // if start != end, there is one last batch to process
    if start != end {
        commands.spawn(UiBatch {
            range: start..end,
            image: current_batch_image,
            z: last_z,
        });
    }
    ui_meta
        .instance_buffer
        .write_buffer(&render_device, &render_queue);

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

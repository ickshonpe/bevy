mod pipeline;
mod render_pass;

use bevy_core_pipeline::{core_2d::Camera2d, core_3d::Camera3d};
use bevy_ecs::storage::SparseSet;
use bevy_hierarchy::Parent;
use bevy_render::view::ViewVisibility;
use bevy_render::{ExtractSchedule, Render};
use bevy_window::{PrimaryWindow, Window};
pub use pipeline::*;
pub use render_pass::*;

use crate::{
    prelude::UiCameraConfig, BackgroundColor, BorderColor, CalculatedClip, ComputedLayout,
    ContentSize, Style, UiImage, UiScale, UiStack, UiTextureAtlasImage, Val,
};

use bevy_app::prelude::*;
use bevy_asset::{load_internal_asset, AssetEvent, AssetId, Assets, Handle};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Rect, URect, UVec4, Vec2};
use bevy_render::{
    camera::Camera,
    color::Color,
    render_asset::RenderAssets,
    render_graph::{RenderGraph, RunGraphOnViewNode},
    render_phase::{sort_phase_system, AddRenderCommand, DrawFunctions, RenderPhase},
    render_resource::*,
    renderer::{RenderDevice, RenderQueue},
    texture::Image,
    view::{ExtractedView, ViewUniforms},
    Extract, RenderApp, RenderSet,
};
use bevy_sprite::{SpriteAssetEvents, TextureAtlas};
#[cfg(feature = "bevy_text")]
use bevy_text::{PositionedGlyph, Text, TextLayoutInfo};
use bevy_transform::components::GlobalTransform;
use bevy_utils::{FloatOrd, HashMap};
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

pub const UI_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(13012847047162779583);

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
                // extract_uinode_borders.after(RenderUiSystem::ExtractAtlasNode),
                #[cfg(feature = "bevy_text")]
                extract_text_uinodes.after(RenderUiSystem::ExtractAtlasNode),
            ),
        )
        .add_systems(
            Render,
            (
                queue_uinodes.in_set(RenderSet::Queue),
                sort_phase_system::<TransparentUi>.in_set(RenderSet::PhaseSort),
                prepare_uinodes.in_set(RenderSet::PrepareBindGroups),
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
    pub position: Vec2,
    pub size: Vec2,
    pub color: Color,
    pub uv_rect: Option<Rect>,
    pub image: AssetId<Image>,
    pub clip: Option<Rect>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub border_width: [f32; 4],
    pub border_radius: [f32; 4],
    pub border_color: Color,
}

#[derive(Resource, Default)]
pub struct ExtractedUiNodes {
    pub uinodes: SparseSet<Entity, ExtractedUiNode>,
}

pub fn extract_atlas_uinodes(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    images: Extract<Res<Assets<Image>>>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<
            (
                Entity,
                &ComputedLayout,
                &BackgroundColor,
                &ViewVisibility,
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
            entity,
            uinode,
            color,
            view_visibility,
            clip,
            texture_atlas_handle,
            atlas_image,
        )) = uinode_query.get(*entity)
        {
            // Skip invisible and completely transparent nodes
            if !view_visibility.get() || color.0.a() == 0.0 {
                continue;
            }

            let (mut atlas_rect, image) =
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
                    (atlas_rect, texture_atlas.texture.clone())
                } else {
                    // Atlas not present in assets resource (should this warn the user?)
                    continue;
                };

            // Skip loading images
            if !images.contains(&image) {
                continue;
            }

            atlas_rect.min /= atlas_rect.size();
            atlas_rect.max /= atlas_rect.size();

            extracted_uinodes.uinodes.insert(
                entity,
                ExtractedUiNode {
                    stack_index,
                    position: uinode.position,
                    size: uinode.size,
                    color: color.0,
                    uv_rect: Some(atlas_rect),
                    clip: clip.map(|clip| clip.clip),
                    image: image.id(),
                    flip_x: atlas_image.flip_x,
                    flip_y: atlas_image.flip_y,
                    border_width: [0.; 4],
                    border_radius: [0.; 4],
                    border_color: Color::NONE,
                },
            );
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
                Entity,
                &ComputedLayout,
                &BackgroundColor,
                Option<&UiImage>,
                &ViewVisibility,
                Option<&CalculatedClip>,
                Option<&BorderColor>,
            ),
            Without<UiTextureAtlasImage>,
        >,
    >,
) {
    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((entity, uinode, color, maybe_image, view_visibility, clip, maybe_border_color)) =
            uinode_query.get(*entity)
        {
            // Skip invisible and completely transparent nodes
            if !view_visibility.get() || color.0.a() == 0.0 {
                continue;
            }

            let (image, flip_x, flip_y) = if let Some(image) = maybe_image {
                // Skip loading images
                if !images.contains(&image.texture) {
                    continue;
                }
                (image.texture.id(), image.flip_x, image.flip_y)
            } else {
                (AssetId::default(), false, false)
            };

            extracted_uinodes.uinodes.insert(
                entity,
                ExtractedUiNode {
                    stack_index,
                    position: uinode.position(),
                    size: uinode.size(),
                    color: color.0,
                    uv_rect: None,
                    clip: clip.map(|clip| clip.clip),
                    image,
                    flip_x,
                    flip_y,
                    border_width: uinode.border_thickness,
                    border_radius: uinode.border_radius,
                    border_color: maybe_border_color.map(|b| b.0).unwrap_or(Color::NONE),
                },
            );
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
    let scale = (ui_scale.0 as f32).recip();
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
    mut commands: Commands,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
    ui_stack: Extract<Res<UiStack>>,
    ui_scale: Extract<Res<UiScale>>,
    uinode_query: Extract<
        Query<(
            &ComputedLayout,
            &Text,
            &TextLayoutInfo,
            &ViewVisibility,
            Option<&CalculatedClip>,
        )>,
    >,
) {
    // TODO: Support window-independent UI scale: https://github.com/bevyengine/bevy/issues/5621

    let scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.0)
        * ui_scale.0;

    let inverse_scale_factor = (scale_factor as f32).recip();

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((uinode, text, text_layout_info, view_visibility, clip)) =
            uinode_query.get(*entity)
        {
            // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
            if !view_visibility.get() || uinode.size().x == 0. || uinode.size().y == 0. {
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
                let size = uv_rect.size() * inverse_scale_factor;
                uv_rect.min /= atlas.size;
                uv_rect.max /= atlas.size;

                let position = node_position + *glyph_position * inverse_scale_factor - 0.5 * size;

                extracted_uinodes.uinodes.insert(
                    commands.spawn_empty().id(),
                    ExtractedUiNode {
                        stack_index,
                        position,
                        size,
                        color,
                        uv_rect: Some(uv_rect),
                        image: atlas.texture.id(),
                        clip: clip.map(|clip| clip.clip),
                        flip_x: false,
                        flip_y: false,
                        border_width: [0.; 4],
                        border_radius: [0.; 4],
                        border_color: Color::NONE,
                    },
                );
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct UiInstance {
    // Affine 4x3 transposed to 3x4
    pub i_location: [f32; 2],
    pub i_size: [f32; 2],
    pub i_z: f32,
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
        z: f32,
        uv_rect: Rect,
        color: &Color,
        mode: u32,
        radius: [f32; 4],
        border: [f32; 4],
        border_color: &Color,
    ) -> Self {
        Self {
            i_location: location.into(),
            i_size: size.into(),
            i_z: z,
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
    pub image: AssetId<Image>,
}

const TEXTURED_QUAD: u32 = 0;
const UNTEXTURED_QUAD: u32 = 1;

#[allow(clippy::too_many_arguments)]
pub fn queue_uinodes(
    extracted_uinodes: Res<ExtractedUiNodes>,
    ui_pipeline: Res<UiPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<UiPipeline>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<TransparentUi>)>,
    pipeline_cache: Res<PipelineCache>,
    draw_functions: Res<DrawFunctions<TransparentUi>>,
) {
    let draw_function = draw_functions.read().id::<DrawUi>();
    for (view, mut transparent_phase) in &mut views {
        let pipeline = pipelines.specialize(
            &pipeline_cache,
            &ui_pipeline,
            UiPipelineKey { hdr: view.hdr },
        );
        transparent_phase
            .items
            .reserve(extracted_uinodes.uinodes.len());
        for (entity, extracted_uinode) in extracted_uinodes.uinodes.iter() {
            transparent_phase.add(TransparentUi {
                draw_function,
                pipeline,
                entity: *entity,
                sort_key: FloatOrd(extracted_uinode.stack_index as f32),
                // batch_size will be calculated in prepare_uinodes
                batch_size: 0,
            });
        }
    }
}

#[derive(Resource, Default)]
pub struct UiImageBindGroups {
    pub values: HashMap<AssetId<Image>, BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_uinodes(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    view_uniforms: Res<ViewUniforms>,
    ui_pipeline: Res<UiPipeline>,
    mut image_bind_groups: ResMut<UiImageBindGroups>,
    gpu_images: Res<RenderAssets<Image>>,
    mut phases: Query<&mut RenderPhase<TransparentUi>>,
    events: Res<SpriteAssetEvents>,
    mut previous_len: Local<usize>,
) {
    // If an image has changed, the GpuImage has (probably) changed
    for event in &events.images {
        match event {
            AssetEvent::Added { .. } |
            // Images don't have dependencies
            AssetEvent::LoadedWithDependencies { .. } => {}
            AssetEvent::Modified { id } | AssetEvent::Removed { id } => {
                image_bind_groups.values.remove(id);
            }
        };
    }

    #[inline]
    fn is_textured(image: AssetId<Image>) -> bool {
        image != AssetId::default()
    }

    if let Some(view_binding) = view_uniforms.uniforms.binding() {
        let mut batches: Vec<(Entity, UiBatch)> = Vec::with_capacity(*previous_len);

        ui_meta.instance_buffer.clear();
        ui_meta.view_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding,
            }],
            label: Some("ui_view_bind_group"),
            layout: &ui_pipeline.view_layout,
        }));
        for mut ui_phase in &mut phases {
            for item_index in 0..ui_phase.items.len() {
                let item = &mut ui_phase.items[item_index];
                if let Some(extracted_uinode) = extracted_uinodes.uinodes.get(item.entity) {
                    if let Some(gpu_image) = gpu_images.get(extracted_uinode.image) {
                        image_bind_groups
                            .values
                            .entry(extracted_uinode.image)
                            .or_insert_with(|| {
                                render_device.create_bind_group(&BindGroupDescriptor {
                                    entries: &[
                                        BindGroupEntry {
                                            binding: 0,
                                            resource: BindingResource::TextureView(
                                                &gpu_image.texture_view,
                                            ),
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
                    } else {
                        continue;
                    }

                    let mode = if is_textured(extracted_uinode.image) {
                        TEXTURED_QUAD
                    } else {
                        UNTEXTURED_QUAD
                    };

                    batches.push((
                        item.entity,
                        UiBatch {
                            image: extracted_uinode.image,
                            range: item_index as u32..item_index as u32 + 1,
                        },
                    ));

                    ui_meta.instance_buffer.push(UiInstance::from(
                        extracted_uinode.position,
                        extracted_uinode.size,
                        extracted_uinode.stack_index as f32 * 0.001,
                        extracted_uinode.uv_rect.unwrap_or(Rect {
                            min: Vec2::ZERO,
                            max: Vec2::ONE,
                        }),
                        &extracted_uinode.color,
                        mode,
                        extracted_uinode.border_radius,
                        extracted_uinode.border_width,
                        &extracted_uinode.border_color,
                    ));
                    ui_phase.items[item_index].batch_size = 1;
                }
            }
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

        *previous_len = batches.len();

        // for mut ui_phase in &mut phases {
        //     println!("PHASE ITEM COUNT: {}", ui_phase.items.len());
        // }
        // println!("EXTRACTED COUNT: {}", extracted_uinodes.uinodes.len());
        // println!("BATCH COUNT: {}", batches.len());
        commands.insert_or_spawn_batch(batches);
    }
    extracted_uinodes.uinodes.clear();
}

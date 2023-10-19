mod pipeline;
mod render_pass;
//mod gradients;

use bevy_core_pipeline::{core_2d::Camera2d, core_3d::Camera3d};
use bevy_render::{ExtractSchedule, Render};
use bevy_window::{PrimaryWindow, Window};
pub use pipeline::*;
pub use render_pass::*;

use crate::{
    prelude::UiCameraConfig, BackgroundColor, BorderColor, CalculatedClip, ContentSize, Node,
    Style, UiImage, UiScale, UiStack, UiTextureAtlasImage, Val,
};
use crate::{LinearGradient, Outline, UiColor};

use bevy_app::prelude::*;
use bevy_asset::{load_internal_asset, AssetEvent, Assets, Handle, HandleUntyped};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Rect, UVec4, Vec2, Vec3, Vec3Swizzles, Vec4};
use bevy_reflect::TypeUuid;
use bevy_render::texture::{DEFAULT_IMAGE_HANDLE, TEXTURE_ASSET_INDEX};
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
use std::num::NonZeroU64;
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
            if !visibility.is_visible() || color.is_visible() {
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

            let gradient = match &color.0 {
                UiColor::Color(color) => (*color).into(),
                UiColor::LinearGradient(g) => g.clone(),
                UiColor::RadialGradient(_r) => Color::NONE.into(),
            };

            extracted_uinodes.uinodes.push(ExtractedItem {
                stack_index,
                clip: clip.map(|clip| clip.clip),
                image,
                instance: ExtractedInstance::Node(NodeInstance {
                    location: uinode.position.into(),
                    size: uinode.size().into(),
                    uv_border: [
                            atlas_rect.min.x,
                            atlas_rect.min.y,
                            atlas_rect.size().x,
                            atlas_rect.size().y,
                    ],
                    color: gradient.stops[0].color.as_linear_rgba_f32(),
                    radius: [0.; 4],
                    flags: 1,
                }),
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
            if !visibility.is_visible() {
                continue;
            }
            
            let color = match &color.0 {
                UiColor::Color(color) => (*color).into(),
                _ => None,
            };
            
            let border_color = match maybe_border_color {
                Some(BorderColor(UiColor::Color(color))) => (*color).into(),
                _ => None,
            };
            
            
            
            let (image, flip_x, flip_y) = if let Some(image) = maybe_image {
                // Skip loading images
                if !images.contains(&image.texture) {
                    continue;
                }
                (image.texture.clone_weak(), image.flip_x, image.flip_y)
            } else {
                (DEFAULT_IMAGE_HANDLE.typed(), false, false)
            };

            if let Some(color) = color {
                extracted_uinodes.uinodes.push(ExtractedItem {
                    stack_index,
                    clip: clip.map(|clip| clip.clip),
                    image,
                    instance: ExtractedInstance::Node(NodeInstance {
                        location: uinode.position.into(),
                        size: uinode.size().into(),
                        uv_border: [0., 0., 1., 1.],
                        color: color.as_linear_rgba_f32(),
                        radius: uinode.border_radius,
                        flags: TEXTURED_QUAD,
                    }),
                });
            }
            

            if let Some(border_color) = border_color {
                let i = NodeInstance {
                    location:  uinode.position.into(),
                    size: uinode.size().into(),
                    uv_border: [20.; 4],
                    color: border_color.as_linear_rgba_f32(),
                    radius: [0.; 4],
                    flags: BORDERED,
                };

                extracted_uinodes.uinodes.push(ExtractedItem {
                    stack_index,
                    clip: clip.map(|clip| clip.clip),
                    image: DEFAULT_IMAGE_HANDLE.typed().clone(),
                    instance: ExtractedInstance::Node(i)
                });

            }

            if let Some(outline) = maybe_outline {
                extracted_uinodes.uinodes.push(ExtractedItem {
                    stack_index,
                    clip: clip.map(|clip| clip.clip),
                    image: DEFAULT_IMAGE_HANDLE.typed().clone(),
                    instance: ExtractedInstance::Node(NodeInstance {
                        location:  (uinode.position() - Vec2::splat(uinode.outline_offset + uinode.outline_width)).into(),
                        size: (uinode.size() + 2. * (uinode.outline_width + uinode.outline_offset)).into(),
                        uv_border: [uinode.outline_width; 4],
                        color: outline.color.as_linear_rgba_f32(),
                        radius: uinode.border_radius.map(|r| r + uinode.outline_width + uinode.outline_offset),
                        flags: BORDERED,
                    })
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

                extracted_uinodes.uinodes.push(ExtractedItem {
                    stack_index,
                    clip: None,
                    image: atlas.texture.clone(),
                    instance: ExtractedInstance::Text(TextInstance {
                        location: position.into(),
                        size: scaled_glyph_size.into(),
                        uv_min: uv_rect.min.into(),
                        uv_size: uv_rect.size().into(),
                        color: color.as_linear_rgba_f32(),
                    }),
                });
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct ExtractedUiNodes {
    pub uinodes: Vec<ExtractedItem>,
}

pub struct ExtractedItem {
    pub stack_index: usize,
    pub clip: Option<Rect>,
    pub image: Handle<Image>,
    pub instance: ExtractedInstance,
}

pub enum BatchType {
    Node,
    Text,
}

pub enum ExtractedInstance {
    Node(NodeInstance),
    Text(TextInstance),
}

impl ExtractedInstance {
    pub fn get_type(&self) -> BatchType {
        match self {
            ExtractedInstance::Node(_) => BatchType::Node,
            ExtractedInstance::Text(_) => BatchType::Text,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct NodeInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_border: [f32; 4],
    pub color: [f32; 4],
    pub radius: [f32; 4],
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct TextInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_size: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ClipUniform {
    pub clip: [f32;4],
}

#[derive(Resource)]
pub struct UiMeta {
    clip_bind_group: Option<BindGroup>,
    view_bind_group: Option<BindGroup>,
    index_buffer: BufferVec<u32>,
    instance_buffer: BufferVec<NodeInstance>,
    text_instance_buffer: BufferVec<TextInstance>,
    clipped_instance_buffer: BufferVec<NodeInstance>,
    clipped_text_instance_buffer: BufferVec<TextInstance>,
    clip_buffer: BufferVec<[f32;4]>,
}

impl Default for UiMeta {
    fn default() -> Self {
        Self {
            clip_bind_group: None,
            view_bind_group: None,
            index_buffer: BufferVec::<u32>::new(BufferUsages::INDEX),
            instance_buffer: BufferVec::<NodeInstance>::new(BufferUsages::VERTEX),
            text_instance_buffer: BufferVec::<TextInstance>::new(BufferUsages::VERTEX),
            clipped_instance_buffer: BufferVec::<NodeInstance>::new(BufferUsages::VERTEX),
            clipped_text_instance_buffer: BufferVec::<TextInstance>::new(BufferUsages::VERTEX),
            clip_buffer: BufferVec::<[f32; 4]>::new(BufferUsages::UNIFORM),
        }
    }
}

#[derive(Component)]
pub struct UiBatch {
    pub batch_type: BatchType,
    pub range: Range<u32>,
    pub image: Handle<Image>,
    pub z: f32,
}

const UNTEXTURED_QUAD: u32 = 0;
const TEXTURED_QUAD: u32 = 1; 
const BORDERED: u32 = 32;

pub fn prepare_uinodes(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut ui_meta: ResMut<UiMeta>,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
) {
    ui_meta.instance_buffer.clear();
    ui_meta.clip_buffer.clear();
    ui_meta.text_instance_buffer.clear();

    // sort by ui stack index, starting from the deepest node
    extracted_uinodes
        .uinodes
        .sort_by_key(|node| node.stack_index);

    let mut text_index: u32 = 0;
    let mut node_index = 0;


    

    for node in &extracted_uinodes.uinodes {
        let index = match node.instance {
            ExtractedInstance::Node(node) => {
                ui_meta.instance_buffer.push(node);
                node_index += 1;
                node_index - 1
            },
            ExtractedInstance::Text(text) => {
                ui_meta.text_instance_buffer.push(text);
                text_index += 1;
                text_index - 1
            },
        };


        let ui_batch = UiBatch {
            batch_type: node.instance.get_type(),
            range: index..index + 1,
            image: node.image.clone(),
            z: node.stack_index as f32,
        };

        commands.spawn(ui_batch);
    }

    ui_meta
        .instance_buffer
        .write_buffer(&render_device, &render_queue);

    ui_meta
        .text_instance_buffer
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

    ui_meta.clip_buffer.push([10., 30.0, 400., 700.]);
    
    ui_meta.clip_buffer.write_buffer(&render_device, &render_queue);

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

        if let Some(clip) = ui_meta.clip_buffer.buffer() {
            ui_meta.clip_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor {
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding { 
                        buffer: clip, 
                        offset: 0, 
                        size: NonZeroU64::new(16),
                    }),
                }],
                label: Some("ui_clip_bind_group"),
                layout: &ui_pipeline.clip_layout,
            }));
        } 
    
        let draw_ui_function = draw_functions.read().id::<DrawUi>();
        for (view, mut transparent_phase) in &mut views {
            let node_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey { hdr: view.hdr, clip: false, text: false, node: true  },
            );
            let clipped_node_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey { hdr: view.hdr, clip: true, text: false, node: true },
            );
            let text_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey { hdr: view.hdr, clip: false, text: true, node: false },
            );
            let clipped_text_pipeline = pipelines.specialize(
                &pipeline_cache,
                &ui_pipeline,
                UiPipelineKey { hdr: view.hdr, clip: true, text: true, node: false },
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
                };
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

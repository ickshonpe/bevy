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

fn is_color_none(ui_color: &UiColor) -> bool {
    if let UiColor::Color(color) = ui_color {
        color.a() == 0.0
    } else {
        false
    }
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
                clip.map(|clip| clip.clip),
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

            if color.is_visible() {
                let (image, flip_x, flip_y) = if let Some(image) = maybe_image {
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
                            clip.map(|clip| clip.clip),
                        );        
                    },
                    UiColor::LinearGradient(l) => {
                        extracted_uinodes.push_node_with_linear_gradient(
                            stack_index,
                            uinode.position,
                            uinode.size(),
                            image,
                            Rect::new(0.0, 0.0, 1.0, 1.0),
                            uinode.border_radius,   
                            l.clone(),
                            clip.map(|clip| clip.clip),
                        );        
                    },
                    UiColor::RadialGradient(_) => {
                    },
                }
            }
                

            if let Some(border_color) = maybe_border_color {
                if border_color.is_visible() {
                    match border_color.0 {
                        UiColor::Color(color) => {
                            extracted_uinodes.push_border(
                                stack_index,
                                uinode.position,
                                uinode.size(),
                                color,
                                uinode.border,
                                uinode.border_radius,
                                clip.map(|clip| clip.clip),
                            );                    
                        },
                        UiColor::LinearGradient(_) => {
                        },
                        UiColor::RadialGradient(_) => {
                        },
                    }
                }
            }

            if let Some(outline) = maybe_outline {
                extracted_uinodes.push_border(
                    stack_index,
                    uinode.position() - Vec2::splat(uinode.outline_offset + uinode.outline_width),
                    uinode.size() + 2. * (uinode.outline_width + uinode.outline_offset),
                    outline.color,
                    [uinode.outline_width; 4],
                    uinode.border_radius,
                    clip.map(|clip| clip.clip),
                );
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
    pub stack_index: usize,
    pub image: Handle<Image>,
    pub instance: ExtractedInstance,
}

fn rect_to_arr(r: Rect) -> [f32; 4] {
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
        if let Some(clip) = clip {
            let i = CTextInstance {
                location: position.into(),
                size: size.into(),
                uv_min,
                uv_size,
                color,
                clip: rect_to_arr(clip),
            };
            self.uinodes.push(ExtractedItem {
                stack_index,
                image,
                instance: ExtractedInstance::CText(i),
            });
        } else {
            let i = TextInstance {
                location: position.into(),
                size: size.into(),
                uv_min,
                uv_size,
                color,
            };
            self.uinodes.push(ExtractedItem {
                stack_index,
                image,
                instance: ExtractedInstance::Text(i),
            });
        }
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
        if let Some(clip) = clip {
            let i = CNodeInstance {
                location: position.into(),
                size: size.into(),
                uv_border: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
                color,
                radius,
                flags,
                clip: rect_to_arr(clip),
            };
            self.uinodes.push(ExtractedItem {
                stack_index,
                image,
                instance: ExtractedInstance::CNode(i),
            });
        } else {
            let i = NodeInstance {
                location: position.into(),
                size: size.into(),
                uv_border: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
                color,
                radius,
                flags,
            };
            self.uinodes.push(ExtractedItem {
                stack_index,
                image,
                instance: ExtractedInstance::Node(i),
            });
        }
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
        if let Some(clip) = clip {
            let i = CNodeInstance {
                location: position.into(),
                size: size.into(),
                uv_border: inset,
                color,
                radius,
                flags,
                clip: rect_to_arr(clip),
            };
            self.uinodes.push(ExtractedItem {
                stack_index,
                image: DEFAULT_IMAGE_HANDLE.typed(),
                instance: ExtractedInstance::CNode(i),
            });
        } else {
            let i = NodeInstance {
                location: position.into(),
                size: size.into(),
                uv_border: inset,
                color,
                radius,
                flags,
            };
            self.uinodes.push(ExtractedItem {
                stack_index,
                image: DEFAULT_IMAGE_HANDLE.typed(),
                instance: ExtractedInstance::Node(i),
            });
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
        gradient: LinearGradient,
        clip: Option<Rect>,

    ) {
        let uv_min = uv_rect.min;
        let uv_size = uv_rect.size();

       
        let focal_point = (gradient.point - Vec2::splat(0.5)) * size;

        let tflag = if image.is_some() {
            TEXTURED_QUAD //| FILL_START | FILL_END
        } else {
            UNTEXTURED_QUAD //| FILL_START | FILL_END
        };
        let len = size.y;
        let s = len / (gradient.stops.len() - 1) as f32;
        let mut a = 0.;
        let mut b = s;

        let image = image.unwrap_or(DEFAULT_IMAGE_HANDLE.typed());

        for i in 0..gradient.stops.len() - 1 {
            let start = gradient.stops[i];
            let end = gradient.stops[i + 1];
            let mut flags = tflag;
            if i == 0 {
                flags |= FILL_START;
            }

            if i + 2 == gradient.stops.len() {
                flags |= FILL_END;
            }
            
            if let Some(clip) = clip {
                let i = CLinearGradientInstance {
                    location: position.into(),
                    size: size.into(),
                    uv_border: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
                    radius,
                    flags,
                    focal_point: focal_point.into(),
                    angle: gradient.angle,
                    start_color: start.color.as_linear_rgba_f32(),
                    start_len: a,
                    end_len: b,
                    end_color: end.color.as_linear_rgba_f32(),
                    clip: rect_to_arr(clip),
                };
                self.uinodes.push(ExtractedItem {
                    stack_index,
                    image: image.clone(),
                    instance: ExtractedInstance::CLinearGradient(i),
                });
            } else {
                let i = LinearGradientInstance {
                    location: position.into(),
                    size: size.into(),
                    uv_border: [uv_min.x, uv_min.y, uv_size.x, uv_size.y],
                    radius,
                    flags,
                    focal_point: focal_point.into(),
                    angle: gradient.angle,
                    start_color: start.color.as_linear_rgba_f32(),
                    start_len: a,
                    end_len: b,
                    end_color: end.color.as_linear_rgba_f32(),
                };
                self.uinodes.push(ExtractedItem {
                    stack_index,
                    image: image.clone(),
                    instance: ExtractedInstance::LinearGradient(i),
                });
            }

            a += s;
            b += s;
        }
    }
}

pub enum BatchType {
    Node,
    Text,
    CNode,
    CText,
    LinearGradient,
    CLinearGradient,
}

pub enum ExtractedInstance {
    Node(NodeInstance),
    Text(TextInstance),
    LinearGradient(LinearGradientInstance),
    CNode(CNodeInstance),
    CText(CTextInstance),
    CLinearGradient(CLinearGradientInstance),
}

impl ExtractedInstance {
    pub fn get_type(&self) -> BatchType {
        match self {
            ExtractedInstance::Node(_) => BatchType::Node,
            ExtractedInstance::Text(_) => BatchType::Text,            
            ExtractedInstance::CNode(_) => BatchType::CNode,
            ExtractedInstance::CText(_) => BatchType::CText,
            ExtractedInstance::LinearGradient(_) => BatchType::LinearGradient,
            ExtractedInstance::CLinearGradient(_) => BatchType::CLinearGradient,
            
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
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct LinearGradientInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_border: [f32; 4],
    pub radius: [f32; 4],
    pub flags: u32,
    pub focal_point: [f32; 2],
    pub angle: f32,
    // @location(7) start_color: vec4<f32>,
    pub start_color: [f32; 4],
    // @location(8) start_len: f32,
    pub start_len: f32,
    // @location(9) end_len: f32,
    pub end_len: f32,
    // @location(10) end_color: vec4<f32>,
    pub end_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct RadialGradientInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_border: [f32; 4],
    pub radius: [f32; 4],
    pub flags: u32,
    pub focal_point: [f32; 2],
    pub ratio: f32,
    // @location(7) start_color: vec4<f32>,
    pub start_color: [f32; 4],
    // @location(8) start_len: f32,
    pub start_len: f32,
    // @location(9) end_len: f32,
    pub end_len: f32,
    // @location(10) end_color: vec4<f32>,
    pub end_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct CNodeInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_border: [f32; 4],
    pub color: [f32; 4],
    pub radius: [f32; 4],
    pub flags: u32,
    pub clip: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct CTextInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_size: [f32; 2],
    pub color: [f32; 4],
    pub clip: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct CLinearGradientInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_border: [f32; 4],
    pub radius: [f32; 4],
    pub flags: u32,
    pub focal_point: [f32; 2],
    pub angle: f32,
    // @location(7) start_color: vec4<f32>,
    pub start_color: [f32; 4],
    // @location(8) start_len: f32,
    pub start_len: f32,
    // @location(9) end_len: f32,
    pub end_len: f32,
    // @location(10) end_color: vec4<f32>,
    pub end_color: [f32; 4],
    pub clip: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct CRadialGradientInstance {
    pub location: [f32; 2],
    pub size: [f32; 2],
    pub uv_border: [f32; 4],
    pub radius: [f32; 4],
    pub flags: u32,
    pub focal_point: [f32; 2],
    pub ratio: f32,
    // @location(7) start_color: vec4<f32>,
    pub start_color: [f32; 4],
    // @location(8) start_len: f32,
    pub start_len: f32,
    // @location(9) end_len: f32,
    pub end_len: f32,
    // @location(10) end_color: vec4<f32>,
    pub end_color: [f32; 4],
    pub clip: [f32; 4],
}

pub struct UiInstanceBuffers<N, T, L>
where
    N: Pod + Zeroable,
    T: Pod + Zeroable,
    L: Pod + Zeroable,
{
    node: BufferVec<N>,
    text: BufferVec<T>,
    linear_gradient: BufferVec<L>,
}

impl<N, T, L> Default for UiInstanceBuffers<N, T, L>
where
    N: Pod + Zeroable,
    T: Pod + Zeroable,
    L: Pod + Zeroable,
{
    fn default() -> Self {
        Self {
            node: BufferVec::<N>::new(BufferUsages::VERTEX),
            text: BufferVec::<T>::new(BufferUsages::VERTEX),
            linear_gradient: BufferVec::<L>::new(BufferUsages::VERTEX),
        }
    }
}

impl<N, T, L> UiInstanceBuffers<N, T, L>
where
    N: Pod + Zeroable,
    T: Pod + Zeroable,
    L: Pod + Zeroable,
{
    pub fn clear(&mut self) {
        self.node.clear();
        self.text.clear();
    }

    pub fn write_buffers(&mut self, render_device: &RenderDevice, render_queue: &RenderQueue) {
        self.node.write_buffer(&render_device, &render_queue);
        self.text.write_buffer(&render_device, &render_queue);
        self.linear_gradient.write_buffer(&render_device, &render_queue);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, ShaderType, Default)]
struct UiClip {
    clip: Vec4,
}

#[derive(Resource)]
pub struct UiMeta {
    view_bind_group: Option<BindGroup>,
    index_buffer: BufferVec<u32>,
    unclipped_instance_buffers: UiInstanceBuffers<NodeInstance, TextInstance, LinearGradientInstance>,
    clipped_instance_buffers: UiInstanceBuffers<CNodeInstance, CTextInstance, CLinearGradientInstance>,
    clip_buffer: BufferVec<[f32; 4]>,
}

impl Default for UiMeta {
    fn default() -> Self {
        Self {
            view_bind_group: None,
            index_buffer: BufferVec::<u32>::new(BufferUsages::INDEX),
            unclipped_instance_buffers: Default::default(),
            clipped_instance_buffers: Default::default(),
            clip_buffer: BufferVec::<[f32; 4]>::new(BufferUsages::UNIFORM),
        }
    }
}

impl UiMeta {
    fn clear_instance_buffers(&mut self) {
        self.unclipped_instance_buffers.clear();
        self.clipped_instance_buffers.clear();
    }

    fn write_instance_buffers(&mut self, render_device: &RenderDevice, render_queue: &RenderQueue) {
        self.unclipped_instance_buffers
            .write_buffers(render_device, render_queue);
        self.clipped_instance_buffers
            .write_buffers(render_device, render_queue);
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
    ui_meta.clip_buffer.clear();

    // sort by ui stack index, starting from the deepest node
    extracted_uinodes
        .uinodes
        .sort_by_key(|node| node.stack_index);

    let mut text_index: u32 = 0;
    let mut node_index = 0;
    let mut ctext_index: u32 = 0;
    let mut cnode_index = 0;
    let mut lg_index = 0;
    let mut clg_index = 0;

    for node in &extracted_uinodes.uinodes {
        let index = match node.instance {
            ExtractedInstance::Node(node) => {
                ui_meta.unclipped_instance_buffers.node.push(node);
                node_index += 1;
                node_index
            }
            ExtractedInstance::Text(text) => {
                ui_meta.unclipped_instance_buffers.text.push(text);
                text_index += 1;
                text_index
            }
            ExtractedInstance::LinearGradient(l) => {
                ui_meta.unclipped_instance_buffers.linear_gradient.push(l);
                lg_index += 1;
                lg_index
            },
            ExtractedInstance::CNode(c) => {
                ui_meta.clipped_instance_buffers.node.push(c);
                cnode_index += 1;
                cnode_index
            }
            ExtractedInstance::CText(c) => {
                ui_meta.clipped_instance_buffers.text.push(c);
                ctext_index += 1;
                ctext_index
            }
            ExtractedInstance::CLinearGradient(l) => {
                ui_meta.clipped_instance_buffers.linear_gradient.push(l);
                clg_index += 1;
                clg_index
            }
        };

        let ui_batch = UiBatch {
            batch_type: node.instance.get_type(),
            range: index - 1..index,
            image: node.image.clone(),
            z: node.stack_index as f32,
        };

        commands.spawn(ui_batch);
    }

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

    ui_meta
        .clip_buffer
        .write_buffer(&render_device, &render_queue);
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

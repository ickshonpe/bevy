use std::num::NonZeroU64;

use bevy_ecs::prelude::*;
use bevy_render::{
    render_resource::*,
    renderer::RenderDevice,
    texture::BevyDefault,
    view::{ViewTarget, ViewUniform},
};

#[derive(Resource)]
pub struct UiPipeline {
    pub view_layout: BindGroupLayout,
    pub image_layout: BindGroupLayout,
    pub clip_layout: BindGroupLayout,
}

impl FromWorld for UiPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let view_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(ViewUniform::min_size()),
                },
                count: None,
            }],
            label: Some("ui_view_layout"),
        });

        let image_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("ui_image_layout"),
        });

        let clip_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
            label: Some("ui_clip_layout"),
        });

        UiPipeline {
            view_layout,
            image_layout,
            clip_layout,
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct UiPipelineKey {
    pub hdr: bool,
    pub clip: bool,
    pub text: bool,
    // pub radial: bool,
    // pub linear: bool,
    // pub border: bool,
    // pub radius: bool,
    pub node: bool,
}

impl SpecializedRenderPipeline for UiPipeline {
    type Key = UiPipelineKey;
    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = Vec::new();
        let mut formats = vec![];
        if key.clip {
            shader_defs.push("CLIP".into());
        }

        if key.text {
            shader_defs.push("SPECIAL".into());
            shader_defs.push("TEXT".into());
            formats.extend([   
                // @location(0) i_location: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(1) i_size: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(2) i_uv_min: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(3) i_uv_size: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(4) i_color: vec4<f32>,
                VertexFormat::Float32x4,
            ]);
        } else if key.node {
            shader_defs.push("SPECIAL".into());
            shader_defs.push("NODE".into());
            formats.extend([
                    // @location(0) i_location: vec2<f32>,
                    VertexFormat::Float32x2,
                    // @location(1) i_size: vec2<f32>,
                    VertexFormat::Float32x2,
                    // @location(2) i_uv_border: vec4<f32>,
                    VertexFormat::Float32x4,
                    // @location(3) i_color: vec4<f32>,
                    VertexFormat::Float32x4,
                    // @location(4) i_radius: vec4<f32>,
                    VertexFormat::Float32x4,
                    // @location(5) i_flags: u32,
                    VertexFormat::Uint32,
                ]); 
        }
        
        //    // @location(0) i_location: vec2<f32>,
        //    VertexFormat::Float32x2,
        //    // @location(1) i_size: vec2<f32>,
        //    VertexFormat::Float32x2,
        //    // @location(2) i_uv_min: vec2<f32>,
        //    VertexFormat::Float32x2,
        //    // @location(3) i_uv_size: vec2<f32>,
        //    VertexFormat::Float32x2,
        //    // @location(4) i_color: vec4<f32>,
        //    VertexFormat::Float32x4,
        //    // @location(5) i_radius: vec4<f32>,
        //    VertexFormat::Float32x4,
        //    // @location(6) i_border: vec4<f32>,
        //    VertexFormat::Float32x4,
        //    // @location(7) i_flags: u32,
        //    VertexFormat::Uint32,
        //    // @location(8) i_border_color: vec4<f32>,
        //    VertexFormat::Float32x4,
        //    // @location(9) i_g_color: vec4<f32>,
        //    VertexFormat::Float32x4,
        //    // @location(10) i_gb_color: vec4<f32>,
        //    VertexFormat::Float32x4,
        //    // @location(11) i_g_angle: f32,
        //    VertexFormat::Float32,

        let instance_rate_vertex_buffer_layout = VertexBufferLayout::from_vertex_formats(VertexStepMode::Instance, formats);

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: super::UI_SHADER_HANDLE.typed::<Shader>(),
                entry_point: "vertex".into(),
                shader_defs: shader_defs.clone(),
                buffers: vec![instance_rate_vertex_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: super::UI_SHADER_HANDLE.typed::<Shader>(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: if key.hdr {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            layout: vec![
                self.view_layout.clone(), 
                self.image_layout.clone(),
                self.clip_layout.clone(),
            ],
            push_constant_ranges: Vec::new(),
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("ui_pipeline".into()),
        }
    }
}

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

        UiPipeline {
            view_layout,
            image_layout,
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct UiPipelineKey {
    pub hdr: bool,
}

impl SpecializedRenderPipeline for UiPipeline {
    type Key = UiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let instance_rate_vertex_buffer_layout = VertexBufferLayout {
            array_stride: 56,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                // @location(0) i_location: vec2<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // @location(1) i_size: vec2<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                // @location(2) i_z: f32,
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 16,
                    shader_location: 2,
                },
                // @location(3) i_uv_min: vec2<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: 20,
                    shader_location: 3,
                },
                // @location(4) i_uv_size: vec2<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: 28,
                    shader_location: 4,
                },
                // @location(5) i_color: vec4<f32>,
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 36,
                    shader_location: 5,
                },
                // @location(6) i_mode: u32,
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: 52,
                    shader_location: 6,
                }
            ],
        };

        let shader_defs = Vec::new();

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: super::UI_SHADER_HANDLE,
                entry_point: "vertex".into(),
                shader_defs: shader_defs.clone(),
                buffers: vec![instance_rate_vertex_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: super::UI_SHADER_HANDLE,
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
            layout: vec![self.view_layout.clone(), self.image_layout.clone()],
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
            push_constant_ranges: Vec::new(),
        }
    }
}

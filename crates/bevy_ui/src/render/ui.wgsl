#import bevy_render::maths affine_to_square
#import bevy_render::view  View

const TEXTURED_QUAD: u32 = 0u;


@group(0) @binding(0)
var<uniform> view: View;

struct VertexInput {
    @builtin(vertex_index) index: u32,
    // NOTE: Instance-rate vertex buffer members prefixed with i_
    // NOTE: i_model_transpose_colN are the 3 columns of a 3x4 matrix that is the transpose of the
    // affine 4x3 model matrix.
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_z: f32,
    @location(3) i_color: vec4<f32>,
    //@location(4) i_uv_offset_scale: vec4<f32>,
 //  @location(5) i_mode: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) mode: u32,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = in.i_size * norm_location;
    out.clip_position = view.view_proj * vec4(in.i_location + relative_location, in.i_z, 1.0);
    out.uv = norm_location;
    out.color = in.i_color;
    out.mode = TEXTURED_QUAD;
    return out;
}


@group(1) @binding(0)
var sprite_texture: texture_2d<f32>;
@group(1) @binding(1)
var sprite_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // textureSample can only be called in unform control flow, not inside an if branch.
    var color = textureSample(sprite_texture, sprite_sampler, in.uv);
    if in.mode == TEXTURED_QUAD {
        color = in.color * color;
    } else {
        color = in.color;
    }
    return color;
}

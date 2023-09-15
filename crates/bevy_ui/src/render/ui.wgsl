#import bevy_render::maths affine_to_square
#import bevy_render::view  View

const TEXTURED_QUAD: u32 = 0u;


@group(0) @binding(0)
var<uniform> view: View;

struct VertexInput {
    @builtin(vertex_index) index: u32,
    // NOTE: Instance-rate vertex buffer members prefixed with i_
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_z: f32,
    @location(3) i_uv_min: vec2<f32>,
    @location(4) i_uv_size: vec2<f32>,
    @location(5) i_color: vec4<f32>,
    @location(6) i_radius: vec4<f32>,
    @location(7) i_border: vec4<f32>,
    @location(8) i_flags: u32,
    @location(9) i_border_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) mode: u32,
    @location(3) @interpolate(flat) radius: f32,
    @location(4) @interpolate(flat) border: vec2<f32>,
    @location(5) point: vec2<f32>,
    @location(6) @interpolate(flat) border_color: vec4<f32>,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    switch in.index {
        case 0u: {
            // top left
            out.radius = in.i_radius.x;
            out.border.x = in.i_border.x;
            out.border.y = in.i_border.y;
        }
        case 1u: {
            // top right
            out.radius = in.i_radius.y;
            out.border.x = in.i_border.z;
            out.border.y = in.i_border.y;
        }
        case 2u: {
            // bottom left
            out.radius = in.i_radius.z;
            out.border.x = in.i_border.x;
            out.border.y = in.i_border.w;
        }
        default: {
            // bottom right
            out.radius = in.i_radius.w;
            out.border.x = in.i_border.z;
            out.border.y = in.i_border.w;
        }
    }
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = in.i_size * norm_location;
    out.clip_position = view.view_proj * vec4(in.i_location + relative_location, in.i_z, 1.0);
    out.uv = in.i_uv_min + in.i_uv_size * norm_location;
    out.color = in.i_color;
    out.mode = in.i_flags;
    out.point = in.i_size * (norm_location - 0.5);
    out.border = half_size - out.border;
    out.border_color = in.i_border_color;
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
    let point = abs(in.point);
    if in.mode == TEXTURED_QUAD {
        color = in.color * color;
    } else {
        color = in.color;
    }
    
    if in.border.x < point.x || in.border.y < point.y {
        color = in.border_color;
    }

    return color;
}

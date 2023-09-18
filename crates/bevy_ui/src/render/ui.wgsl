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
    @location(2) @interpolate(flat) flags: u32,
    @location(3) @interpolate(flat) radius: vec4<f32>,
    @location(4) @interpolate(flat) border: vec4<f32>,
    @location(5) point: vec2<f32>,
    @location(6) @interpolate(flat) border_color: vec4<f32>,
    @location(7) @interpolate(flat) size: vec2<f32>,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
   
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = in.i_size * norm_location;
    out.clip_position = view.view_proj * vec4(in.i_location + relative_location, in.i_z, 1.0);
    out.uv = in.i_uv_min + in.i_uv_size * norm_location;
    out.color = in.i_color;
    out.flags = in.i_flags;
    out.border = in.i_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = in.i_size * (norm_location - 0.5);
    out.border_color = in.i_border_color;
    return out;
}


@group(1) @binding(0)
var sprite_texture: texture_2d<f32>;
@group(1) @binding(1)
var sprite_sampler: sampler;

// The returned value is the shortest distance from the given point to the boundary of the rounded box.
// Negative values indicate that the point is inside the rounded box, positive values that the point is outside, and zero is exactly on the boundary.
// arguments
// point -> The function will return the distance from this point to the closest point on the boundary.
// size -> The maximum width and height of the box.
// corner_radii -> The radius of each rounded corner. Ordered counter clockwise starting top left:
//                      x = top left, y = top right, z = bottom right, w = bottom left.
fn sd_rounded_box(point: vec2<f32>, size: vec2<f32>, corner_radii: vec4<f32>) -> f32 {
    // if 0.0 < y then select bottom left (w) and bottom right corner radius (z)
    // else select top left (x) and top right corner radius (y)
    let rs = select(corner_radii.xy, corner_radii.wz, 0.0 < point.y);
    // w and z are swapped so that both pairs are in left to right order, otherwise this second select statement would return the incorrect value for the bottom pair.
    let radius = select(rs.x, rs.y, 0.0 < point.x);
    // Vector from the corner closest to the point, to the point
    let corner_to_point = abs(point) - 0.5 * size;
    // Vector from the center of the radius circle to the point 
    let q = corner_to_point + radius;
    // length from center of the radius circle to the point, 0s a component if the point is not within the quadrant of the radius circle that is part of the curved corner.
    let l = length(max(q, vec2(0.0)));
    let m = min(max(q.x, q.y), 0.0);
    return l + m - radius;
}

fn sd_inset_rounded_box(point: vec2<f32>, size: vec2<f32>, radius: vec4<f32>, inset: vec4<f32>) -> f32 {
    let inner_size = size - inset.xy - inset.zw;
    let inner_center = inset.xy + 0.5 * inner_size - 0.5 *size;
    let inner_point = point - inner_center;

    var r = radius;

    // top left corner
    r.x = r.x - max(inset.x, inset.y);

    // top right corner
    r.y = r.y - max(inset.z, inset.y);

    // bottom right corner
    r.z = r.z - max(inset.z, inset.w); 

    // bottom left corner
    r.w = r.w - max(inset.x, inset.w);

    let half_size = inner_size * 0.5;
    let minimum = min(half_size.x, half_size.y);
    
    r = min(max(r, vec4(0.0)), vec4<f32>(minimum));

    return sd_rounded_box(inner_point, inner_size, r);
}

fn sd_inset_rounded_box_clamped_inner_radius(point: vec2<f32>, size: vec2<f32>, radius: vec4<f32>, inset: vec4<f32>) -> f32 {
    let inner_size = size - inset.xy - inset.zw;
    let inner_center = inset.xy + 0.5 * inner_size - 0.5 *size;
    let inner_point = point - inner_center;

    var r = radius;

    
    if 0. < min(inset.x, inset.y) || inset.x + inset.y <= 0. {
        // top left corner
        r.x = r.x - max(inset.x, inset.y);
    } else {
        r.x = 0.;
    }

    if 0. < min(inset.z, inset.y) || inset.z + inset.y <= 0.{
        // top right corner
        r.y = r.y - max(inset.z, inset.y);
    } else {
        r.y = 0.;
    }

    if 0. < min(inset.z, inset.w) || inset.z + inset.w <= 0. {
        // bottom right corner
        r.z = r.z - max(inset.z, inset.w); 
    } else {
        r.z = 0.;
    }

    if 0. < min(inset.x, inset.w) || inset.x + inset.w <= 0. {
        // bottom left corner
        r.w = r.w - max(inset.x, inset.w);
    } else {
        r.w = 0.;
    }

    let half_size = inner_size * 0.5;
    let minimum = min(half_size.x, half_size.y);
    
    r = min(max(r, vec4<f32>(0.0)), vec4<f32>(minimum));

    return sd_rounded_box(inner_point, inner_size, r);
}

const TEXTURED = 1u;
const BOX_SHADOW = 2u;
const DISABLE_AA = 4u;
const RIGHT_VERTEX = 8u;
const BOTTOM_VERTEX = 16u;
const BORDER: u32 = 32u;
fn enabled(flags: u32, mask: u32) -> bool {
    return (flags & mask) != 0u;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // textureSample can only be called in unform control flow, not inside an if branch.
    let color = in.color * textureSample(sprite_texture, sprite_sampler, in.uv);
    //let color = select(in.color, in.color * texture_color, enabled(in.flags, TEXTURED));
    let point = abs(in.point);

    // Signed distance from the exterior boundary.
    let external_distance = sd_rounded_box(in.point, in.size, in.radius);

    // Signed distance from the border's internal edge (the signed distance is negative if the point is inside the rect but not on the border).
    // If the border size is set to zero, this is the same as as the external distance.
    //let internal_distance = sd_inset_rounded_box(in.point, in.size, in.radius, in.border);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(in.point, in.size, in.radius, in.border);

    // Signed distance from the border (the intersection of the rect with its border)
    // Points inside the border have negative signed distance. Any point outside the border, whether outside the outside edge, or inside the inner edge have positive signed distance.
    let border_distance = max(external_distance, -internal_distance);

    if border_distance < 0. {
        return in.border_color;
    }

    if external_distance < 0. {
        return color;
    }

    return vec4<f32>(0.);
}

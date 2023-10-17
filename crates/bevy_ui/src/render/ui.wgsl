#import bevy_render::view  View

const TEXTURED_QUAD: u32 = 0u;

@group(0) @binding(0) var<uniform> view: View;

struct VertexInput {
    @builtin(vertex_index) index: u32,
    // NOTE: Instance-rate vertex buffer members prefixed with i_
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_uv_min: vec2<f32>,
    @location(3) i_uv_size: vec2<f32>,
    @location(4) i_color: vec4<f32>,
    @location(5) i_radius: vec4<f32>,
    @location(6) i_border: vec4<f32>,
    @location(7) i_flags: u32,
    @location(8) i_border_color: vec4<f32>,
    @location(9) i_clip: vec4<f32>,
    @location(10) i_g_color: vec4<f32>,
    @location(11) i_gb_color: vec4<f32>,
    @location(12) i_g_angle: f32,
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
    @location(8) @interpolate(flat) clip: vec4<f32>,
    @location(9) position: vec2<f32>,
    @location(10) end_color: vec4<f32>,
    @location(11) border_end_color: vec4<f32>,
    @location(12) @interpolate(flat) g_f: vec2<f32>,
    @location(13) @interpolate(flat) g_dir: vec2<f32>,
    @location(14) @interpolate(flat) g_len: f32,
};

const PI: f32 = 3.14159265358979323846;

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = in.i_size * norm_location;
    out.position = in.i_location + relative_location;
    out.clip_position = view.view_proj * vec4(in.i_location + relative_location, 0., 1.);
    out.uv = in.i_uv_min + in.i_uv_size * norm_location;
    out.color = in.i_color;
    out.flags = in.i_flags;
    out.border = in.i_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = in.i_size * (norm_location - 0.4999);
    out.border_color = in.i_border_color;
    out.clip = in.i_clip;
    out.end_color = in.i_g_color;
    out.border_end_color = in.i_gb_color;
    let i = find_quadrant(in.i_g_angle);
    out.g_dir = gradient_dir(in.i_g_angle);
    switch (i) {
        case 0: {
            out.g_f = vec2(-1., 1.) * half_size;
        }
        case 1: {
            out.g_f = vec2(-1., -1.) * half_size;
        }
        case 2: {
            out.g_f = vec2(1., -1.) * half_size;
        }            
        default: {
            out.g_f = vec2(1., 1.) * half_size;
        }           
    }
    out.g_len = 2. * sdf_line(out.g_f, out.g_dir, vec2(0., 0.));

    return out;

}

@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

struct Box {
    // center
    p: vec2<f32>,
    // half size
    s: vec2<f32>,
}

fn inset_box(box: Box, inset: vec4<f32>) -> Box {
    let p = box.p + 0.5 * (-inset.xy + inset.zw);
    let s = box.s - 0.5 * (inset.xy + inset.zw);
    return Box(p, s);
}

fn sd_box(box: Box) -> f32 {
    let d = abs(box.p) - box.s;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y) , 0.0);
}

struct Distance {
    edge: f32,
    border: f32
}

fn find_quadrant(angle: f32) -> i32 {
    let reduced = modulo(angle, 2.0 * PI) ;
    return i32(reduced * 2.0 / PI);
}

fn modulo(x: f32, m: f32) -> f32 {
    return x - m * floor(x / m);
}

// The returned value is the shortest distance from the given point to the boundary of the rounded box.
// Negative values indicate that the point is inside the rounded box, positive values that the point is outside, and zero is exactly on the boundary.
// arguments
// point -> The function will return the distance from this point to the closest point on the boundary.
// size -> The maximum width and height of the box.
// corner_radii -> The radius of each rounded corner. Ordered counter clockwise starting top left:
//                      x = top left, y = top right, z = bottom right, w = bottom left.
fn sd_rounded_box(b: Box, corner_radii: vec4<f32>) -> f32 {
    // if 0.0 < y then select bottom left (w) and bottom right corner radius (z)
    // else select top left (x) and top right corner radius (y)
    let rs = select(corner_radii.xy, corner_radii.wz, 0.0 < b.p.y);
    // w and z are swapped so that both pairs are in left to right order, otherwise this second select statement would return the incorrect value for the bottom pair.
    let radius = select(rs.x, rs.y, 0.0 < b.p.x);
    // Vector from the corner closest to the point, to the point
    let corner_to_point = abs(b.p) - b.s;
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

    return sd_rounded_box(Box(inner_point, half_size), r);
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

    return sd_rounded_box(Box(inner_point, 0.5 * inner_size), r);
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


fn compute_sd_boxes(in: VertexOutput) -> Distance {
    let box = Box(in.point, 0.5 * in.size);
    let inner_box = inset_box(box, in.border);

    let external_distance = sd_box(box);
    let internal_distance = sd_box(inner_box);
    let border_distance = max(external_distance, -internal_distance);
    return Distance(external_distance, border_distance);
}

fn select_inset(p: vec2<f32>, inset: vec4<f32>) -> vec2<f32> {
    if p.x < 0. {
        if p.y < 0. {
            return inset.xy;
        } else {
            return inset.xw;
        }
    } else {
        if p.y < 0. {
            return inset.zy;
        } else {
            return inset.zw;
        }
    }
}

fn compute_rounded_clamped(in: VertexOutput) -> Distance {
    let box = Box(in.point, 0.5 * in.size);
    let inner_box = inset_box(box, in.border);
    let external_distance = sd_rounded_box(box, in.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(in.point, in.size, in.radius, in.border);
    let border_distance = max(external_distance, -internal_distance);
    return Distance(external_distance, border_distance);
}

fn compute_rounded_clamped_2(in: VertexOutput) -> Distance {
    let box = Box(in.point, 0.5 * in.size);
    let inner_box = inset_box(box, in.border);
    let external_distance = sd_rounded_box(box, in.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(in.point, in.size, in.radius, in.border);
    let i = select_inset(in.point, in.border);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(external_distance, border_distance);
}

fn compute_rounded(in: VertexOutput) -> Distance {
    let box = Box(in.point, 0.5 * in.size);
    let inner_box = inset_box(box, in.border);
    let external_distance = sd_rounded_box(box, in.radius);
    let internal_distance = sd_inset_rounded_box(in.point, in.size, in.radius, in.border);
    let border_distance = max(external_distance, -internal_distance);
    return Distance(external_distance, border_distance);
}

fn compute_rounded_2(in: VertexOutput) -> Distance {
    let box = Box(in.point, 0.5 * in.size);
    let inner_box = inset_box(box, in.border);
    let external_distance = sd_rounded_box(box, in.radius);
    let internal_distance = sd_inset_rounded_box(in.point, in.size, in.radius, in.border);
    let i = select_inset(in.point, in.border);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(external_distance, border_distance);
}



fn g(d: f32) -> f32 {
    let d = abs(d);
    return exp(-0.028 * d);
}

fn draw_node_outlined(distance: Distance, in: VertexOutput) -> vec4<f32> {
    let color = in.color * textureSample(sprite_texture, sprite_sampler, in.uv);

    if distance.border <= 0. {
        return vec4(g(distance.border) * in.border_color.rgb, in.border_color.a);
       
    }

    if distance.edge <= 0. {
        return vec4(g(distance.edge) * in.color.rgb, in.color.a);
    }

    return vec4<f32>(0.);
}

fn draw_node_normalized(distance: Distance, in: VertexOutput) -> vec4<f32> {
    let color = in.color * textureSample(sprite_texture, sprite_sampler, in.uv);

    if distance.border <= 0. {
        let s = smooth_normalize(distance.border, -length(0.4 * in.size), 0.);
        return vec4(s * in.border_color.rgb, in.border_color.a);
    }

    if distance.edge <= 0. {
        let s = smooth_normalize(distance.border, 0.0,length(0.4 * in.size));
        return vec4(s * in.color.rgb, in.color.a);
    }

    return vec4<f32>(0.);
}

fn draw_node_mixed_border(distance: Distance, in: VertexOutput) -> vec4<f32> {
    let color = in.color * select(vec4<f32>(1.), textureSample(sprite_texture, sprite_sampler, in.uv), enabled(in.flags, TEXTURED));

    if distance.border <= 0. {    
        let rgb = mix(color.rgb, in.border_color.rgb, in.border.a);
        let a = color.a + in.border_color.a * (1.0 - color.a);
        return vec4<f32>(rgb, a);
    }

    if distance.edge <= 0. {
        return color;
    }

    return vec4<f32>(0.);
}

fn draw_node(distance: Distance, in: VertexOutput) -> vec4<f32> {
    let color = in.color * select(vec4<f32>(1.), textureSample(sprite_texture, sprite_sampler, in.uv), enabled(in.flags, TEXTURED));

    if in.position.x < in.clip.x || in.clip.z < in.position.x || in.position.y < in.clip.y || in.clip.w < in.position.y {
        return vec4<f32>(0.);
    }

    if distance.border <= 0. {    
        return in.border_color;
    }

    if distance.edge <= 0. {
        return color;
    }

    return vec4<f32>(0.);
}

fn basic_border(in: VertexOutput) -> vec4<f32> { 
    let half_size = 0.5 * in.size;
    let tl = -half_size + in.border.xy;
    let br = half_size - in.border.zw;
    if (tl.x < in.point.x) && (tl.y < in.point.y) && (in.point.x < br.x) && (in.point.y < br.y) {
        return in.color;
    }

    return in.border_color;
}

fn smooth_normalize(distance: f32, min_val: f32, max_val: f32) -> f32 {
    let t = clamp((distance - min_val) / (max_val - min_val), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn apply_gradient(tc: vec4<f32>, sc: vec4<f32>, ec: vec4<f32>, in: VertexOutput) -> vec4<f32> {
    let d = sdf_line(in.g_f, in.g_dir, in.point);
    let s = d / in.g_len;
    let c = mix(sc, ec, s);
    return  c * tc;
}

fn draw_node_with_gradient(distance: Distance, in: VertexOutput) -> vec4<f32> {
    let tc = select(vec4<f32>(1.), textureSample(sprite_texture, sprite_sampler, in.uv), enabled(in.flags, TEXTURED));
    if in.position.x < in.clip.x || in.clip.z < in.position.x || in.position.y < in.clip.y || in.clip.w < in.position.y {
        return apply_gradient(vec4<f32>(0.), vec4<f32>(0.), vec4<f32>(0.), in);
    }

    if distance.border <= 0. {    
        return apply_gradient(vec4<f32>(1.), in.border_color, in.border_end_color, in);
        
    }

    if distance.edge <= 0. {
        return apply_gradient(tc, in.color, in.end_color, in);
    }

    return vec4<f32>(0.);
}

// o point on line, dir normalised direction of the line, p distant point
fn sdf_line(o: vec2<f32>, dir: vec2<f32>, p: vec2<f32>) -> f32 {
    return distance(p, o + dir * dot(p-o, dir));
}

fn gradient_dir(angle: f32) -> vec2<f32> {
    let x = cos(angle);
    let y = sin(angle);
    return vec2<f32>(x, y);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    
    let d = compute_rounded_clamped_2(in);
    //return draw_node(d, in);
    return draw_node_with_gradient(d, in);
}
#import bevy_render::view  View

const PI: f32 = 3.14159265358979323846;


@group(0) @binding(0) var<uniform> view: View;

@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

fn clip(color: vec4<f32>, position: vec2<f32>, clip: vec4<f32>) -> vec4<f32> { 
    if position.x < clip.x || clip.z < position.x || position.y < clip.y || clip.w < position.y {
        return vec4(0.);
    }
    return color;
}

const TEXTURED = 1u;
const BOX_SHADOW = 2u;
const DISABLE_AA = 4u;
const BORDER: u32 = 32u;
const FILL_START: u32 = 64u;
const FILL_END: u32 = 128u;

fn is_enabled(flags: u32, mask: u32) -> bool {
    return (flags & mask) != 0u;
}

#ifdef TEXT 
struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_uv_min: vec2<f32>,
    @location(3) i_uv_size: vec2<f32>,
    @location(4) i_color: vec4<f32>,
    #ifdef CLIP 
        @location(5) i_clip: vec4<f32>,
    #endif
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    #ifdef CLIP 
        @location(3) clip: vec4<f32>,
    #endif
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = in.i_size * norm_location;
    out.position = in.i_location + relative_location;
    out.clip_position = view.view_proj * vec4(in.i_location + relative_location, 0., 1.);
    out.uv = in.i_uv_min + in.i_uv_size * norm_location;
    out.color = in.i_color;

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    return out;
}

@fragment 
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = in.color * textureSample(sprite_texture, sprite_sampler, in.uv);
    
    #ifdef CLIP 
        return clip(color, in.position, in.clip);
    #else 
        return color;
    #endif
}
#endif

#ifdef NODE

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_uv: vec4<f32>,
    @location(3) i_color: vec4<f32>,
    @location(4) i_radius: vec4<f32>,
    @location(5) i_flags: u32,
    @location(6) i_border: vec4<f32>,
    #ifdef CLIP 
        @location(7) i_clip: vec4<f32>,
    #endif
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) flags: u32,
    @location(3) @interpolate(flat) radius: vec4<f32>,
    @location(5) point: vec2<f32>,
    @location(7) @interpolate(flat) size: vec2<f32>,
    @location(8) position: vec2<f32>,
    @location(9) @interpolate(flat) border: vec4<f32>,
    #ifdef CLIP 
        @location(10) clip: vec4<f32>,
    #endif
};


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
    let uv_min = in.i_uv.xy;
    let uv_size = in.i_uv.zw;
    out.uv = uv_min + uv_size * norm_location;
    out.color = in.i_color;
    out.flags = in.i_flags;
    out.border = in.i_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = in.i_size * (norm_location - 0.4999);

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    var n: Node;
    n.size = in.size;
    n.radius = in.radius;
    n.inset = in.border;
    let distance = compute_geometry_separated(in.point, n);
    let fb = fwidth(distance.border);
    let fe = fwidth(distance.edge);
    var f: f32;
    var d: f32;
    if is_enabled(in.flags, BORDER) {
        d = distance.border;
        f = fb;
    } else {
        d = distance.edge;
        f = fe;
    }
    
    let color = select(in.color, in.color * sampled_color, is_enabled(in.flags, TEXTURED));
    let a = mix(0.0, color.a, 1.0 - smoothstep(0.0, f, d));
    let color_out = vec4(color.rgb, a);   
    
    #ifdef CLIP
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
}

#endif

#ifdef LINEAR_GRADIENT

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_uv_border: vec4<f32>,
    @location(3) i_radius: vec4<f32>,
    @location(4) i_flags: u32,
    // point on a line perpendicular to the gradient
    // coordinates should be relative to the center of the ui node
    @location(5) focal_point: vec2<f32>,
    // angle of the gradient
    @location(6) angle: f32,
    // color it starts at
    @location(7) start_color: vec4<f32>,
    // distance from focal point where the gradient starts
    @location(8) start_len: f32,
    // distance from the focal point when the gradient ends
    @location(9) end_len: f32,
    // color the gradient ends at
    @location(10) end_color: vec4<f32>,
    
    #ifdef CLIP 
        @location(11) i_clip: vec4<f32>,
    #endif
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) flags: u32,
    @location(2) @interpolate(flat) radius: vec4<f32>,
    @location(3) point: vec2<f32>,
    @location(4) @interpolate(flat) size: vec2<f32>,
    @location(5) position: vec2<f32>,
    @location(6) @interpolate(flat) border: vec4<f32>,
    @location(7) @interpolate(flat) focal_point: vec2<f32>,
    // unit vector in the direction of the gradient
    @location(8) @interpolate(flat) dir: vec2<f32>,
    @location(9) @interpolate(flat) start_color: vec4<f32>,
    @location(10) @interpolate(flat) start_len: f32,
    @location(11) @interpolate(flat) end_len: f32,
    @location(12) @interpolate(flat) end_color: vec4<f32>,
    
    #ifdef CLIP 
        @location(14) clip: vec4<f32>,
    #endif
};

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
    let uv_min = in.i_uv_border.xy;
    let uv_size = in.i_uv_border.zw;
    out.uv = uv_min + uv_size * norm_location;
    out.flags = in.i_flags;
    out.border = in.i_uv_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = in.i_size * (norm_location - 0.4999);
    out.focal_point = in.focal_point;
    out.dir = gradient_dir(in.angle);
    out.start_color = in.start_color;
    out.start_len = in.start_len;
    out.end_len = in.end_len;
    out.end_color = in.end_color;
    
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    var n: Node;
    n.size = in.size;
    n.radius = in.radius;
    n.inset = in.border;
    let distance = compute_geometry_separated(in.point, n);

    let gradient_distance = df_line(in.focal_point, in.dir, in.point);
    let t = gradient(gradient_distance, in.start_len, in.end_len);

    var gradient_color: vec4<f32>;

    if t <= 0.0 {
        if is_enabled(in.flags, FILL_START) {
            gradient_color = in.start_color;
        } else {
            gradient_color = vec4(0.0);   
        }
    } else if 1.0 < t {
        if is_enabled(in.flags, FILL_END) {
            gradient_color = in.end_color;
        } else {
            gradient_color = vec4(0.0);   
        }
    } else {
        gradient_color = mix(in.start_color, in.end_color, t);
    }

    let fb = fwidth(distance.border);
    let fe = fwidth(distance.edge);
    var f: f32;
    var d: f32;
    if is_enabled(in.flags, BORDER) {
        d = distance.border;
        f = fb;
    } else {
        d = distance.edge;
        f = fe;
    }
    
    let color = select(gradient_color, gradient_color * sampled_color, is_enabled(in.flags, TEXTURED));
    let alpha_out = mix(0.0, gradient_color.a, 1.0 - smoothstep(0.0, f, d));
    let color_out = vec4(gradient_color.rgb, alpha_out);   

    #ifdef CLIP
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
}
#endif

#ifdef RADIAL_GRADIENT


struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_uv_border: vec4<f32>,
    @location(3) i_radius: vec4<f32>,
    @location(4) i_flags: u32,
    // center of the radial gradient
    @location(5) g_center: vec2<f32>,
    @location(6) g_ratio: f32,
    @location(7) start_color: vec4<f32>,
    // distance from center where the gradient starts
    @location(8) start_len: f32,
    // distance from the center where the gradient ends
    @location(9) end_len: f32,
    // color the gradient ends at
    @location(10) end_color: vec4<f32>,
    
    #ifdef CLIP 
        @location(11) i_clip: vec4<f32>,
    #endif
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) flags: u32,
    @location(2) @interpolate(flat) radius: vec4<f32>,
    @location(3) point: vec2<f32>,
    @location(4) @interpolate(flat) size: vec2<f32>,
    @location(5) position: vec2<f32>,
    @location(6) @interpolate(flat) border: vec4<f32>,
    @location(7) @interpolate(flat) g_center: vec2<f32>,
    @location(8) @interpolate(flat) g_ratio: f32,
    @location(9) @interpolate(flat) start_color: vec4<f32>,
    @location(10) @interpolate(flat) start_len: f32,
    @location(11) @interpolate(flat) end_len: f32,
    @location(12) @interpolate(flat) end_color: vec4<f32>,
    
    #ifdef CLIP 
        @location(13) clip: vec4<f32>,
    #endif
};

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
    let uv_min = in.i_uv_border.xy;
    let uv_size = in.i_uv_border.zw;
    out.uv = uv_min + uv_size * norm_location;
    out.flags = in.i_flags;
    out.border = in.i_uv_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = in.i_size * (norm_location - 0.4999);
    out.g_center = in.g_center;
    out.start_color = in.start_color;
    out.start_len = in.start_len;
    out.end_len = in.end_len;
    out.end_color = in.end_color;
    out.g_ratio = in.g_ratio;
    
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    var n: Node;
    n.size = in.size;
    n.radius = in.radius;
    n.inset = in.border;
    let distance = compute_geometry_separated(in.point, n);

    let x = in.point.x;
    let y = in.point.y * in.g_ratio;
    let p = vec2<f32>(x, y);
    let gradient_distance = distance(in.g_center, p);
    let t = gradient(gradient_distance, in.start_len, in.end_len);

    var gradient_color: vec4<f32>;

    if t <= 0.0 {
        if is_enabled(in.flags, FILL_START) {
            gradient_color = in.start_color;
        } else {
            gradient_color = vec4(0.);
        }
    } else if 1.0 < t {
        if is_enabled(in.flags, FILL_END) {
            gradient_color = in.end_color;
        } else {
            gradient_color = vec4(0.);
        }
    } else {
        gradient_color = mix(in.start_color, in.end_color, t);
    }
        
    let fb = fwidth(distance.border);
    let fe = fwidth(distance.edge);
    var f: f32;
    var d: f32;
    if is_enabled(in.flags, BORDER) {
        d = distance.border;
        f = fb;
    } else {
        d = distance.edge;
        f = fe;
    }
    
    let color = select(gradient_color, gradient_color * sampled_color, is_enabled(in.flags, TEXTURED));
    let alpha_out = mix(0.0, gradient_color.a, 1.0 - smoothstep(0.0, f, d));
    let color_out = vec4(gradient_color.rgb, alpha_out);   

    #ifdef CLIP
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
}

#endif


#ifdef DASHED_BORDER

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_line_thickness: f32,
    @location(3) i_color: vec4<f32>,
    @location(4) i_radius: vec4<f32>,
    @location(5) i_dash_length: f32,
    #ifdef CLIP 
        @location(6) i_clip: vec4<f32>,
    #endif
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(flat) radius: vec4<f32>,
    @location(2) point: vec2<f32>,
    @location(3) @interpolate(flat) size: vec2<f32>,
    @location(4) position: vec2<f32>,
    @location(5) @interpolate(flat) line_thickness: f32,
    @location(6) @interpolate(flat) quadrant_lengths: vec4<f32>,
    @location(7) @interpolate(flat) dash_length: f32,
    #ifdef CLIP 
        @location(10) clip: vec4<f32>,
    #endif
};


@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = in.i_size * norm_location;

    out.clip_position = view.view_proj * vec4(in.i_location + relative_location, 0., 1.);
    out.color = in.i_color;
    out.radius = in.i_radius;
    out.point = in.i_size * (norm_location - 0.4999);
    out.size = in.i_size;
    out.position = in.i_location + relative_location;
    out.line_thickness = in.i_line_thickness;
    
    out.dash_length = in.i_dash_length;

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let box = Box(in.point, 0.5 * in.size);
    let d = sd_rounded_box(box, in.radius);
    if 0. < d {
        return vec4(0.);
    }

    if d < -in.line_thickness {
        return vec4(0.);
    }
    
    #ifdef CLIP
        return clip(in.color, in.position, in.clip);
    #else 
        return in.color;
    #endif
    
}

#endif

fn compute_geometry_separated(
    point: vec2<f32>, 
    n: Node,
) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(point, n);
    let i = select_inset(point, n.inset);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(internal_distance_2, border_distance);
}


fn compute_geometry(
    point: vec2<f32>, 
    n: Node,
) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(point, n);
    let i = select_inset(point, n.inset);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(external_distance, border_distance);
}

fn compute_geometry_adjusted(
    point: vec2<f32>, 
    n: Node,
) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(point, n);
    let i = select_inset(point, n.inset);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(external_distance, border_distance);
}

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

fn sd_inset_rounded_box(point: vec2<f32>, node: Node) -> f32 {
    let inset = node.inset;
    let size = node.size;
    let radius = node.radius;
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


fn sd_inset_rounded_box_clamped_inner_radius(point: vec2<f32>, n: Node) -> f32 {
    let inner_size = n.size - n.inset.xy - n.inset.zw;
    let inner_center = n.inset.xy + 0.5 * inner_size - 0.5 * n.size;
    let inner_point = point - inner_center;

    var r = n.radius;
    let inset = n.inset;
    
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



fn compute_sd_boxes(point: vec2<f32>, n: Node) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);

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

struct Node {
    size: vec2<f32>,
    radius: vec4<f32>,
    inset: vec4<f32>,
}

fn compute_rounded_clamped(point: vec2<f32>, n: Node) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(point, n);
    let border_distance = max(external_distance, -internal_distance);
    return Distance(external_distance, border_distance);
}

fn compute_rounded_clamped_2(point: vec2<f32>, n: Node) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box_clamped_inner_radius(point, n);
    let i = select_inset(point, n.inset);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(external_distance, border_distance);
}

fn compute_rounded(point: vec2<f32>, n: Node) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box(point, n);
    let border_distance = max(external_distance, -internal_distance);
    return Distance(external_distance, border_distance);
}

fn compute_rounded_2(point: vec2<f32>, n: Node) -> Distance {
    let box = Box(point, 0.5 * n.size);
    let inner_box = inset_box(box, n.inset);
    let external_distance = sd_rounded_box(box, n.radius);
    let internal_distance = sd_inset_rounded_box(point, n);
    let i = select_inset(point, n.inset);
    let internal_distance_2 = max(external_distance + min(i.x, i.y), internal_distance);
    let border_distance = max(external_distance, -internal_distance_2);
    return Distance(external_distance, border_distance);
}

fn g(d: f32) -> f32 {
    let d = abs(d);
    return exp(-0.028 * d);
}

fn smooth_normalize(distance: f32, min_val: f32, max_val: f32) -> f32 {
    let t = clamp((distance - min_val) / (max_val - min_val), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn apply_gradient(f: vec2<f32>, dir: vec2<f32>, point: vec2<f32>, len: f32, sampled_color: vec4<f32>, sc: vec4<f32>, ec: vec4<f32>) -> vec4<f32> {
    let d = df_line(f, dir, point);
    let s = d / len;
    let c = mix(sc, ec, s);
    return  c * sampled_color;
}

// return the distance of point `p` from the line defined by point `o` and direction `dir`
// returned value is always positive
fn df_line(o: vec2<f32>, dir: vec2<f32>, p: vec2<f32>) -> f32 {
    // project p onto the the o-dir line and then return the distance between p and the projection.
    return distance(p, o + dir * dot(p-o, dir));
}

fn gradient_dir(angle: f32) -> vec2<f32> {
    let x = cos(angle);
    let y = sin(angle);
    return vec2<f32>(x, y);
}

fn gradient(p: f32, start: f32, end:f32) -> f32 {
    let len = end - start;
    return (p - start) / len;
}

// calculate the length along the edge of this quadrant
// r should be clamped to min of half_size's components
fn calculate_quadrant_perimeter(
    half_size: vec2<f32>,
    r: f32,
) -> f32 {
    let h = half_size.x - r;
    let v = half_size.y - r;
    let arc_length = r * PI / 2.;
    return h + v + arc_length;
}

// returns the perimeter of a rounded UI node rect
fn calculate_perimeter(
    half_size: vec2<f32>,
    rs: vec4<f32>
) -> f32 {
    return
        calculate_quadrant_perimeter(half_size, rs[0])
        + calculate_quadrant_perimeter(half_size, rs[1])
        + calculate_quadrant_perimeter(half_size, rs[2])
        + calculate_quadrant_perimeter(half_size, rs[3]);
}


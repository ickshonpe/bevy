#import bevy_render::view::View

const PI: f32 = 3.14159265358979323846;


@group(0) @binding(0) var<uniform> view: View;

<<<<<<< HEAD
@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

fn clip(color: vec4<f32>, position: vec2<f32>, clip: vec4<f32>) -> vec4<f32> { 
    if position.x < clip.x || clip.z < position.x || position.y < clip.y || clip.w < position.y {
        return vec4(0.);
    }
    return color;
}

fn antialias(distance: f32, current_alpha: f32) -> f32 {
    // we want to antialias when the distance value is between -0.5 and 0.5
    return mix(0.0, current_alpha, 1.0 - smoothstep(0.0, 1.0, distance + 0.5));
}

const TEXTURED = 1u;
const BOX_SHADOW = 2u;
const DISABLE_AA = 4u;
const BORDER: u32 = 32u;
const FILL_START: u32 = 64u;
const FILL_END: u32 = 128u;

const PADDING: f32 = 5.;
const F: f32 = 1.;

fn is_border_enabled(flags: u32) -> bool {
    return (flags & BORDER) != 0u;
}

fn is_enabled(flags: u32, mask: u32) -> bool {
    return (flags & mask) != 0u;
}

// ***********************************************************************************

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


// ***********************************************************************************

#ifdef NODE

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_flags: u32,
    @location(3) i_border: vec4<f32>,
    @location(4) i_radius: vec4<f32>,
    @location(5) i_color: vec4<f32>,
    @location(6) i_uv: vec4<f32>,
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
    @location(4) point: vec2<f32>,
    @location(5) @interpolate(flat) size: vec2<f32>,
    @location(6) position: vec2<f32>,
    @location(7) @interpolate(flat) border: f32,
    #ifdef CLIP 
        @location(8) clip: vec4<f32>,
    #endif
};


@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    let padding = select(PADDING, 0., is_enabled(in.i_flags, TEXTURED));
    let location = in.i_location - padding;
    var out: VertexOutput;
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    //let relative_location = in.i_size * norm_location;
    let relative_location = (in.i_size + 2. * padding) * norm_location;
    out.position = location + relative_location;
    out.clip_position = view.view_proj * vec4(location + relative_location, 0., 1.);
    let uv_min = in.i_uv.xy;
    let uv_size = in.i_uv.zw;
    let uv_padding = uv_size * (vec2(padding, padding) / in.i_size);
    out.uv = uv_min - uv_padding + (2. * uv_padding + uv_size) * norm_location;
    out.color = in.i_color;
    out.flags = in.i_flags;
    out.border = in.i_border[0];
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = (2. * padding + in.i_size) * (norm_location - 0.4999);

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    let color = select(in.color, in.color * sampled_color, is_enabled(in.flags, TEXTURED));

    let d = compute_signed_distance_with_uniform_border(in.point, 0.5 * in.size, in.flags, in.border, in.radius);
    
    let a = antialias(d, color.a);
    let color_out = vec4(color.rgb, a);

    #ifdef CLIP 
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
    
}

#endif

// ***********************************************************************************

#ifdef LINEAR_GRADIENT

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_flags: u32,
    @location(3) i_border: vec4<f32>,
    @location(4) i_radius: vec4<f32>,
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
    @location(0) @interpolate(flat) flags: u32,
    @location(1) @interpolate(flat) radius: vec4<f32>,
    @location(2) point: vec2<f32>,
    @location(3) @interpolate(flat) size: vec2<f32>,
    @location(4) position: vec2<f32>,
    @location(5) @interpolate(flat) border: vec4<f32>,
    @location(6) @interpolate(flat) focal_point: vec2<f32>,
    // unit vector in the direction of the gradient
    @location(7) @interpolate(flat) dir: vec2<f32>,
    @location(8) @interpolate(flat) start_color: vec4<f32>,
    @location(9) @interpolate(flat) start_len: f32,
    @location(10) @interpolate(flat) end_len: f32,
    @location(11) @interpolate(flat) end_color: vec4<f32>,
    
    #ifdef CLIP 
        @location(12) clip: vec4<f32>,
    #endif
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let location = in.i_location - PADDING;
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = (2. * PADDING + in.i_size) * norm_location;
    out.position = location + relative_location;
    out.clip_position = view.view_proj * vec4(location + relative_location, 0., 1.);
    out.flags = in.i_flags;
    out.border = in.i_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = (2. * PADDING + in.i_size) * (norm_location - 0.4999);
    out.focal_point = in.focal_point;
    out.dir = gradient_dir(in.angle);
    out.start_color = in.start_color;
    out.start_len = in.start_len;
    out.end_len = in.end_len;
    out.end_color = in.end_color;

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = compute_signed_distance_with_uniform_border(in.point, 0.5 * in.size, in.flags, in.border[0], in.radius);
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

    let alpha_out = antialias(d, gradient_color.a);
    let color_out = vec4(gradient_color.rgb, alpha_out);   

    #ifdef CLIP
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
}
#endif

// ***********************************************************************************

#ifdef RADIAL_GRADIENT


struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_flags: u32,
    @location(3) i_border: vec4<f32>,
    @location(4) i_radius: vec4<f32>,
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
    let location = in.i_location - PADDING;
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = (2. * PADDING + in.i_size) * norm_location;
    out.position = location + relative_location;
    out.clip_position = view.view_proj * vec4(location + relative_location, 0., 1.);
    out.flags = in.i_flags;
    out.border = in.i_border;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = (2. * PADDING + in.i_size) * (norm_location - 0.4999);
    out.g_center = in.g_center;
    out.start_color = in.start_color;
    out.start_len = in.start_len;
    out.end_len = in.end_len;
    out.end_color = in.end_color;
    out.g_ratio = in.g_ratio;

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = compute_signed_distance_with_uniform_border(in.point, 0.5 * in.size, in.flags, in.border[0], in.radius);
    let r = length((in.g_center - in.point) * vec2<f32>(1., in.g_ratio));
    let t = gradient(r, in.start_len, in.end_len);

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
        
    let alpha_out = antialias(d, gradient_color.a);
    let color_out = vec4(gradient_color.rgb, alpha_out);   

    #ifdef CLIP
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
}

#endif

// ***********************************************************************************

#ifdef DASHED_BORDER

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_line_thickness: f32,
    @location(3) i_color: vec4<f32>,
    @location(4) i_radius: vec4<f32>,
    @location(5) i_dash_length: f32,
    @location(6) i_break_length: f32,
    #ifdef CLIP 
        @location(7) i_clip: vec4<f32>,
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
    @location(8) @interpolate(flat) break_length: f32,
    #ifdef CLIP 
        @location(10) clip: vec4<f32>,
    #endif
};


@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let location = in.i_location - PADDING;
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = (2. * PADDING + in.i_size) * norm_location;

    out.clip_position = view.view_proj * vec4(location + relative_location, 0., 1.);
    out.color = in.i_color;
    out.radius = in.i_radius;
    out.point = (2. * PADDING + in.i_size) * (norm_location - 0.4999);
    out.size = in.i_size;
    out.position = location + relative_location;
    out.line_thickness = in.i_line_thickness;

    let perimeter = compute_rounded_box_perimeter(0.5 * in.i_size, in.i_radius);
    let segment_length = in.i_dash_length + in.i_break_length;
    let num_segments = floor(perimeter / segment_length);
    let adjusted_segment_length = perimeter / num_segments;
    let adjusted_dash = adjusted_segment_length * in.i_dash_length / segment_length;
    let adjusted_break = adjusted_segment_length * in.i_break_length / segment_length;

    out.dash_length = adjusted_dash;
    out.break_length = adjusted_break;

    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let half_size = 0.5 * in.size;
    let d = compute_signed_distance_with_uniform_border(in.point, half_size, BORDER, in.line_thickness, in.radius);
    let i = quadrant_index(in.point);
    var a = antialias(d, in.color.a);
    var p = abs(in.point);
    var s = half_size;
    var t: f32 = 0.;
    for(var j = 0; j < i; j++) {
        t += calculate_quarter_perimeter(s, in.radius[j]);
    }
    if i == 0 || i == 2 {
        p = p.yx;
        s = s.yx;
    }
    t += rounded_border_quarter_distance(
        p.x,
        p.y,
        s.x,
        s.y,
        in.radius[i],
    );
    let m = modulo(t, in.dash_length + in.break_length);
    if in.break_length < m {
       a = 0.;
    }
    let color_out = vec4(in.color.rgb, a);

    #ifdef CLIP
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
    
}

#endif

// ***********************************************************************************

#ifdef SHADOW

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) i_location: vec2<f32>,
    @location(1) i_size: vec2<f32>,
    @location(2) i_radius: vec4<f32>,
    @location(3) i_color: vec4<f32>,
    @location(4) i_blur_radius: f32,
    #ifdef CLIP 
        @location(5) i_clip: vec4<f32>,
    #endif
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(flat) radius: vec4<f32>,
    @location(2) point: vec2<f32>,
    @location(3) @interpolate(flat) size: vec2<f32>,
    @location(4) position: vec2<f32>,
    @location(5) @interpolate(flat) blur_radius: f32,
    #ifdef CLIP 
        @location(6) clip: vec4<f32>,
    #endif
};


@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    let padding = in.i_blur_radius * 2.;
    let location = in.i_location - padding;
    var out: VertexOutput;
    let half_size = 0.5 * in.i_size;
    let norm_x = f32(in.index & 1u);
    let norm_y = f32((in.index & 2u) >> 1u);
    let norm_location = vec2(norm_x, norm_y);
    let relative_location = (in.i_size + 2. * padding) * norm_location;
    out.position = location + relative_location;
    out.clip_position = view.view_proj * vec4(location + relative_location, 0., 1.);
    out.color = in.i_color;
    out.radius = in.i_radius;
    out.size = in.i_size;
    out.point = (2. * padding + in.i_size) * (norm_location - 0.4999);
    out.blur_radius = in.i_blur_radius;
    #ifdef CLIP 
        out.clip = in.i_clip;
    #endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {    
    let color_out = calc_shadow(in.color, in.point, in.size, in.radius, in.blur_radius);
    #ifdef CLIP 
        return clip(color_out, in.position, in.clip);
    #else 
        return color_out;
    #endif
    
}

#endif

// ***********************************************************************************

fn calc_shadow(
    color: vec4<f32>,
    point: vec2<f32>,
    size: vec2<f32>,
    radius: vec4<f32>,
    blur: f32,
) -> vec4<f32> {
    let g = color.a * roundedBoxShadow(-0.5 * size, 0.5 * size, point, max(blur, 0.01), radius);
    let color_out = vec4(color.rgb, g);
    return color_out;
}


fn sd_box(point: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let d = abs(point) - half_size;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y) , 0.0);
}

fn quadrant_index(p: vec2<f32>) -> i32 {
    if p.x < 0. {
        // left
        if p.y < 0. {
            // top left
            return 0;
        } else {
            // bottom left
            return 3;
        }
    } else {
        // right
        if p.y < 0. {
            // top right
            return 1;
        } else {
            // bottom right
            return 2;
        }
    }
}

fn angle_quadrant(angle: f32) -> i32 {
    let reduced = modulo(angle, 2.0 * PI) ;
    return i32(reduced * 2.0 / PI);
}

fn modulo(x: f32, m: f32) -> f32 {
    return x - m * floor(x / m);
}

// The returned value is the shortest distance from the given point to the boundary of the rounded box.
// Negative values indicate that the point is inside the rounded box, positive values that the point is outside, and zero is exactly on the boundary.
// arguments
// p -> The function will return the distance from this point to the closest point on the boundary.
// s -> half size of the box.
// radii -> The radius of each rounded corner. Ordered counter clockwise starting top left:
//                      x = top left, y = top right, z = bottom right, w = bottom left.
fn sd_rounded_box(p: vec2<f32>, s: vec2<f32>, radii: vec4<f32>) -> f32 {
    // if 0.0 < y then select bottom left (w) and bottom right corner radius (z)
    // else select top left (x) and top right corner radius (y)
    let rs = select(radii.xy, radii.wz, 0.0 < p.y);
    // w and z are swapped so that both pairs are in left to right order, otherwise this second select statement would return the incorrect value for the bottom pair.
    let radius = select(rs.x, rs.y, 0.0 < p.x);
    // Vector from the corner closest to the point, to the point
    let corner_to_point = abs(p) - s;
    // Vector from the center of the radius circle to the point 
=======
struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,

    @location(2) @interpolate(flat) size: vec2<f32>,
    @location(3) @interpolate(flat) flags: u32,
    @location(4) @interpolate(flat) radius: vec4<f32>,    
    @location(5) @interpolate(flat) border: vec4<f32>,    

    // Position relative to the center of the rectangle.
    @location(6) point: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vertex(
    @location(0) vertex_position: vec3<f32>,
    @location(1) vertex_uv: vec2<f32>,
    @location(2) vertex_color: vec4<f32>,
    @location(3) flags: u32,

    // x: top left, y: top right, z: bottom right, w: bottom left.
    @location(4) radius: vec4<f32>,

    // x: left, y: top, z: right, w: bottom.
    @location(5) border: vec4<f32>,
    @location(6) size: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vertex_uv;
    out.position = view.clip_from_world * vec4(vertex_position, 1.0);
    out.color = vertex_color;
    out.flags = flags;
    out.radius = radius;
    out.size = size;
    out.border = border;
    var point = 0.49999 * size;
    if (flags & RIGHT_VERTEX) == 0u {
        point.x *= -1.;
    }
    if (flags & BOTTOM_VERTEX) == 0u {
        point.y *= -1.;
    }
    out.point = point;

    return out;
}

@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

// The returned value is the shortest distance from the given point to the boundary of the rounded 
// box.
// 
// Negative values indicate that the point is inside the rounded box, positive values that the point 
// is outside, and zero is exactly on the boundary.
//
// Arguments: 
//  - `point`        -> The function will return the distance from this point to the closest point on 
//                    the boundary.
//  - `size`         -> The maximum width and height of the box.
//  - `corner_radii` -> The radius of each rounded corner. Ordered counter clockwise starting 
//                    top left:
//                      x: top left, y: top right, z: bottom right, w: bottom left.
fn sd_rounded_box(point: vec2<f32>, size: vec2<f32>, corner_radii: vec4<f32>) -> f32 {
    // If 0.0 < y then select bottom left (w) and bottom right corner radius (z).
    // Else select top left (x) and top right corner radius (y).
    let rs = select(corner_radii.xy, corner_radii.wz, 0.0 < point.y);
    // w and z are swapped above so that both pairs are in left to right order, otherwise this second 
    // select statement would return the incorrect value for the bottom pair.
    let radius = select(rs.x, rs.y, 0.0 < point.x);
    // Vector from the corner closest to the point, to the point.
    let corner_to_point = abs(point) - 0.5 * size;
    // Vector from the center of the radius circle to the point.
>>>>>>> main
    let q = corner_to_point + radius;
    // length from center of the radius circle to the point, 0s a component if the point is not within the quadrant of the radius circle that is part of the curved corner.
    let l = length(max(q, vec2(0.0)));
    let m = min(max(q.x, q.y), 0.0);
    return l + m - radius;
}

// return the distance of point `p` from the line defined by point `o` and direction `dir`
// returned value is always positive
fn df_line(o: vec2<f32>, dir: vec2<f32>, p: vec2<f32>) -> f32 {
    // project p onto the the o-dir line and then return the distance between p and the projection.
    return distance(p, o + dir * dot(p-o, dir));
}

<<<<<<< HEAD
fn gradient_dir(angle: f32) -> vec2<f32> {
    let x = cos(angle);
    let y = sin(angle);
    return vec2<f32>(x, y);
}

fn gradient(p: f32, start: f32, end:f32) -> f32 {
    let len = end - start;
    return (p - start) / len;
}

fn sd_box_uniform_border(point: vec2<f32>, half_size: vec2<f32>, border: f32) -> f32 {
    let exterior = sd_box(point, half_size);
    let interior = exterior + border;
    return max(exterior, -interior);
}
=======
// get alpha for antialiasing for sdf
fn antialias(distance: f32) -> f32 {
    // Using the fwidth(distance) was causing artifacts, so just use the distance.
    return clamp(0.0, 1.0, 0.5 - distance);
}

fn draw(in: VertexOutput, texture_color: vec4<f32>) -> vec4<f32> {
    // Only use the color sampled from the texture if the `TEXTURED` flag is enabled. 
    // This allows us to draw both textured and untextured shapes together in the same batch.
    let color = select(in.color, in.color * texture_color, enabled(in.flags, TEXTURED));

    // Signed distances. The magnitude is the distance of the point from the edge of the shape.
    // * Negative values indicate that the point is inside the shape.
    // * Zero values indicate the point is on the edge of the shape.
    // * Positive values indicate the point is outside the shape.
>>>>>>> main

fn sd_rounded_box_uniform_border(point: vec2<f32>, half_size: vec2<f32>, corner_radii: vec4<f32>, border: f32) -> f32 {
    let exterior = sd_rounded_box(point, half_size, corner_radii);
    let interior = exterior + border;
    return max(exterior, -interior);
}

fn sd_rounded_box_interior(point: vec2<f32>, half_size: vec2<f32>, corner_radii: vec4<f32>, border: f32) -> f32 {
    let exterior = sd_rounded_box(point, half_size, corner_radii);
    let interior = exterior + border;
    return interior;    
}

fn compute_signed_distance_with_uniform_border(point: vec2<f32>, half_size: vec2<f32>, flags: u32, border: f32, radius: vec4<f32>) -> f32 {
    var d: f32;
    if is_border_enabled(flags) {        
        d = sd_rounded_box_uniform_border(point, half_size, radius, border);
    } else {
        d = sd_rounded_box_interior(point, half_size, radius, border);
    }
    return d;
}

<<<<<<< HEAD
fn rounded_border_quarter_distance(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
) -> f32 {
    // center of arc
    let qx = w - r;
    let qy = h - r;

    if qx < x && qy < y {
        // within arc area

        // create a normalized vector pointing from qx,qy towards x,y 
        // this vector is pointing towards the point on the arc we want to measure the distance to
        let n = normalize(vec2<f32>(x - qx, y - qy));
        let a = abs(atan2(n.x, n.y)) * r;
    
        return qx + a;
    }

    // distance from right
    let sx = w - x;
    
    // distance from top
    let sy = h - y;

    if sy <= sx {
        return x;
    }

    // must be closer to side edge
    // full arc length
    let l = r * PI / 2.;
    let ty = min(h, qy);
    let t =  max(ty - y, 0.);
    return qx + l + t;
}

// All input values should be positive
fn calculate_quarter_perimeter(s: vec2<f32>, r: f32) -> f32 {
    return s.x + s.y + (0.5 * PI - 2.) * r;
=======
    // At external edges with no border, `border_distance` is equal to zero. 
    // This select statement ensures we only perform anti-aliasing where a non-zero width border 
    // is present, otherwise an outline about the external boundary would be drawn even without 
    // a border.
    let t = select(1.0 - step(0.0, border_distance), antialias(border_distance), external_distance < internal_distance);

    // Blend mode ALPHA_BLENDING is used for UI elements, so we don't premultiply alpha here.
    return vec4(color.rgb, saturate(color.a * t));
}

fn draw_background(in: VertexOutput, texture_color: vec4<f32>) -> vec4<f32> {
    let color = select(in.color, in.color * texture_color, enabled(in.flags, TEXTURED));

    // When drawing the background only draw the internal area and not the border.
    let internal_distance = sd_inset_rounded_box(in.point, in.size, in.radius, in.border);
    let t = antialias(internal_distance);
    return vec4(color.rgb, saturate(color.a * t));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(sprite_texture, sprite_sampler, in.uv);

    if enabled(in.flags, BORDER) {
        return draw(in, texture_color);    
    } else {
        return draw_background(in, texture_color);
    }
>>>>>>> main
}

fn compute_rounded_box_perimeter(s: vec2<f32>, radius: vec4<f32>) -> f32 {
    var t: f32 = 0.;
    for(var j = 0; j < 4; j++) {
        t += calculate_quarter_perimeter(s, radius[j]);
    }
    return t;
}

fn gaussian(x: f32, sigma: f32) -> f32 {
  return exp(-(x * x) / (2. * sigma * sigma)) / (sqrt(2. * PI) * sigma);
}


const FRAC_2_SQRT_PI = 1.1283791;

fn erf(p: vec2<f32>) -> vec2<f32> {
  let s = sign(p);
  let a = abs(p);
  var result = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
  result = result * result;
  return s - s / (result * result);
}


fn selectCorner(x: f32, y: f32, c: vec4<f32>) -> f32 {
  return mix(mix(c.x, c.y, step(0., x)), mix(c.w, c.z, step(0., x)), step(0., y));
}

// Return the blurred mask along the x dimension.
fn roundedBoxShadowX(x: f32, y: f32, s: f32, corner: f32, halfSize: vec2<f32>) -> f32 {
  let d = min(halfSize.y - corner - abs(y), 0.);
  let c = halfSize.x - corner + sqrt(max(0., corner * corner - d * d));
  let integral = 0.5 + 0.5 * erf((x + vec2(-c, c)) * (sqrt(0.5) / s));
  return integral.y - integral.x;
}

// Return the mask for the shadow of a box from lower to upper.
fn roundedBoxShadow(
  lower: vec2<f32>,
  upper: vec2<f32>,
  point: vec2<f32>,
  sigma: f32,
  corners: vec4<f32>,
) -> f32 {
  // Center everything to make the math easier.
  let center = (lower + upper) * 0.5;
  let halfSize = (upper - lower) * 0.5;
  let p = point - center;

  // The signal is only non-zero in a limited range, so don't waste samples.
  let low = p.y - halfSize.y;
  let high = p.y + halfSize.y;
  let start = clamp(-3. * sigma, low, high);
  let end = clamp(3. * sigma, low, high);

  // Accumulate samples (we can get away with surprisingly few samples).
  let step = (end - start) / 4.0;
  var y = start + step * 0.5;
  var value: f32 = 0.0;

  for (var i = 0; i < 4; i++) {
    let corner = selectCorner(p.x, p.y, corners);
    value += roundedBoxShadowX(p.x, p.y - y, sigma, corner, halfSize) * gaussian(y, sigma) * step;
    y += step;
  }

  return value;
}
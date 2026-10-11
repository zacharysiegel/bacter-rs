// Prepended to cell.wgsl and shape.wgsl. The layout of CameraUniform matches camera.rs.

struct CameraUniform {
    world_to_clip: mat4x4<f32>,
    physical_pixels_per_world_pixel: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

// One pixel of antialiasing around a quad's shape and stroke.
const QUAD_MARGIN_PIXELS: f32 = 1.0;

fn get_clip_position(world_position: vec2<f32>) -> vec4<f32> {
    return camera.world_to_clip * vec4<f32>(world_position, 0.0, 1.0);
}

// Triangle strip order: (-1, -1), (1, -1), (-1, 1), (1, 1).
fn get_quad_corner(vertex_index: u32) -> vec2<f32> {
    return vec2<f32>(f32(vertex_index & 1u), f32(vertex_index >> 1u)) * 2.0 - 1.0;
}

// The proportion of a physical pixel inside a shape, from the signed distance of its centre to the shape's edge.
fn get_coverage(signed_distance_pixels: f32) -> f32 {
    return clamp(0.5 - signed_distance_pixels * camera.physical_pixels_per_world_pixel, 0.0, 1.0);
}

// A stroke is centred on the edge.
fn get_stroke_coverage(signed_distance_pixels: f32, stroke_width_pixels: f32) -> f32 {
    let half_width_pixels: f32 = stroke_width_pixels * 0.5;

    return get_coverage(signed_distance_pixels - half_width_pixels)
        - get_coverage(signed_distance_pixels + half_width_pixels);
}

fn get_rectangle_distance(local_position: vec2<f32>, half_extent: vec2<f32>) -> f32 {
    let outside: vec2<f32> = abs(local_position) - half_extent;

    return max(outside.x, outside.y);
}

// Exact for a circle; for an ellipse, the implicit function divided by its gradient's length.
fn get_ellipse_distance(local_position: vec2<f32>, radii: vec2<f32>) -> f32 {
    let normalized_length: f32 = length(local_position / radii);
    let gradient_length: f32 = length(local_position / (radii * radii));

    if (gradient_length == 0.0) {
        return -min(radii.x, radii.y);
    }

    return normalized_length * (normalized_length - 1.0) / gradient_length;
}

fn get_alpha(color: vec4<f32>) -> f32 {
    return color.a;
}

fn get_rgb(color: vec4<f32>) -> vec3<f32> {
    return color.rgb;
}

fn get_premultiplied(color: vec4<f32>, coverage: f32) -> vec4<f32> {
    let alpha: f32 = get_alpha(color) * coverage;

    return vec4<f32>(get_rgb(color) * alpha, alpha);
}

fn get_stroke_over_fill(
    fill_color: vec4<f32>,
    fill_coverage: f32,
    stroke_color: vec4<f32>,
    stroke_coverage: f32,
) -> vec4<f32> {
    let fill: vec4<f32> = get_premultiplied(fill_color, fill_coverage);
    let stroke: vec4<f32> = get_premultiplied(stroke_color, stroke_coverage);

    return stroke + fill * (1.0 - get_alpha(stroke));
}

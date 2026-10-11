// The instance layout matches ShapeInstance in shape_pipeline.rs.

const SHAPE_KIND_RECTANGLE: u32 = 0u;
const SHAPE_KIND_ELLIPSE: u32 = 1u;

struct ShapeInstanceInput {
    @location(0) center: vec2<f32>,
    @location(1) half_extent: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width_pixels: f32,
    @location(5) shape_kind: u32,
}

struct ShapeVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
    @location(1) half_extent: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width_pixels: f32,
    @location(5) @interpolate(flat) shape_kind: u32,
}

@vertex
fn shape_vertex_main(@builtin(vertex_index) vertex_index: u32, instance: ShapeInstanceInput) -> ShapeVertexOutput {
    let local_position: vec2<f32> = get_quad_corner(vertex_index)
        * (instance.half_extent + instance.stroke_width_pixels * 0.5 + QUAD_MARGIN_PIXELS);

    var output: ShapeVertexOutput;
    output.clip_position = get_clip_position(instance.center + local_position);
    output.local_position = local_position;
    output.half_extent = instance.half_extent;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.stroke_width_pixels = instance.stroke_width_pixels;
    output.shape_kind = instance.shape_kind;

    return output;
}

fn get_shape_distance(shape_kind: u32, local_position: vec2<f32>, half_extent: vec2<f32>) -> f32 {
    switch shape_kind {
        case SHAPE_KIND_RECTANGLE, default: {
            return get_rectangle_distance(local_position, half_extent);
        }
        case SHAPE_KIND_ELLIPSE: {
            return get_ellipse_distance(local_position, half_extent);
        }
    }
}

@fragment
fn shape_fragment_main(input: ShapeVertexOutput) -> @location(0) vec4<f32> {
    let signed_distance_pixels: f32 = get_shape_distance(input.shape_kind, input.local_position, input.half_extent);

    return get_stroke_over_fill(
        input.fill_color,
        get_coverage(signed_distance_pixels),
        input.stroke_color,
        get_stroke_coverage(signed_distance_pixels, input.stroke_width_pixels),
    );
}

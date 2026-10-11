// The instance layout matches CellInstance in cell_pipeline.rs.

const INSTANCE_SKIN_GRID: u32 = 0u;
const INSTANCE_SKIN_CIRCLES: u32 = 1u;
const INSTANCE_SKIN_GHOST: u32 = 2u;
const INSTANCE_SKIN_NONE: u32 = 3u;
const INSTANCE_SKIN_SOLID: u32 = 4u;
const INSTANCE_SCALE_UNITS_PER_CELL: f32 = 1000.0;
// Half of geometry::CELL_WIDTH_PIXELS.
const CELL_HALF_WIDTH_PIXELS: f32 = 3.0;
const GRID_STROKE_COLOR: vec4<f32> = vec4<f32>(40.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0, 1.0);
const GRID_STROKE_WIDTH_PIXELS: f32 = 0.25;
const OUTLINE_STROKE_WIDTH_PIXELS: f32 = 1.0;

struct CellInstanceInput {
    @location(0) center: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: u32,
}

struct CellVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) half_width_pixels: f32,
    @location(3) @interpolate(flat) skin_kind: u32,
}

fn instance_skin_kind(params: u32) -> u32 {
    return params & 0xffu;
}

fn instance_scale(params: u32) -> f32 {
    return f32(params >> 16u) / INSTANCE_SCALE_UNITS_PER_CELL;
}

fn instance_color(color: vec4<f32>) -> vec4<f32> {
    return color;
}

@vertex
fn cell_vertex_main(@builtin(vertex_index) vertex_index: u32, instance: CellInstanceInput) -> CellVertexOutput {
    let half_width_pixels: f32 = CELL_HALF_WIDTH_PIXELS * instance_scale(instance.params);
    let local_position: vec2<f32> = get_quad_corner(vertex_index)
        * (half_width_pixels + OUTLINE_STROKE_WIDTH_PIXELS * 0.5 + QUAD_MARGIN_PIXELS);

    var output: CellVertexOutput;
    output.clip_position = get_clip_position(instance.center + local_position);
    output.local_position = local_position;
    output.color = instance_color(instance.color);
    output.half_width_pixels = half_width_pixels;
    output.skin_kind = instance_skin_kind(instance.params);

    return output;
}

@fragment
fn cell_fragment_main(input: CellVertexOutput) -> @location(0) vec4<f32> {
    let half_extent: vec2<f32> = vec2<f32>(input.half_width_pixels);
    let square_distance: f32 = get_rectangle_distance(input.local_position, half_extent);

    switch input.skin_kind {
        case INSTANCE_SKIN_GRID: {
            return get_stroke_over_fill(
                input.color,
                get_coverage(square_distance),
                GRID_STROKE_COLOR,
                get_stroke_coverage(square_distance, GRID_STROKE_WIDTH_PIXELS),
            );
        }
        case INSTANCE_SKIN_CIRCLES: {
            return get_premultiplied(input.color, get_coverage(get_ellipse_distance(input.local_position, half_extent)));
        }
        case INSTANCE_SKIN_GHOST: {
            return get_premultiplied(input.color, get_stroke_coverage(square_distance, OUTLINE_STROKE_WIDTH_PIXELS));
        }
        case INSTANCE_SKIN_NONE: {
            return get_stroke_over_fill(
                input.color,
                get_coverage(square_distance),
                input.color,
                get_stroke_coverage(square_distance, OUTLINE_STROKE_WIDTH_PIXELS),
            );
        }
        case INSTANCE_SKIN_SOLID: {
            return get_premultiplied(input.color, get_coverage(square_distance));
        }
        default: {
            return get_premultiplied(input.color, get_coverage(square_distance));
        }
    }
}

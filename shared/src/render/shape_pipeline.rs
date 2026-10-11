use wgpu::{
    BufferAddress, Device, PipelineLayout, RenderPipeline, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexStepMode,
};

use crate::play::InterpolatedPoint;
use crate::render::{InstancedQuadPipelineSource, RenderColor};
use crate::render::{render_color, scene_pipelines};

const SHAPE_SHADER_SOURCE: &str = concat!(include_str!("scene_common.wgsl"), include_str!("shape.wgsl"));
const SHAPE_INSTANCE_ATTRIBUTES: [VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x2,
    2 => Unorm8x4,
    3 => Unorm8x4,
    4 => Float32,
    5 => Uint32,
];

/// The codes match the `SHAPE_KIND_*` constants of `shape.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

impl ShapeKind {
    pub fn code(self) -> u32 {
        match self {
            ShapeKind::Rectangle => 0,
            ShapeKind::Ellipse => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeGeometry {
    pub kind: ShapeKind,
    /// World pixels.
    pub center: InterpolatedPoint,
    pub half_width_pixels: f32,
    pub half_height_pixels: f32,
}

/// The stroke is centred on the shape's edge and drawn over the fill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapePaint {
    pub fill: RenderColor,
    pub stroke: RenderColor,
    pub stroke_width_pixels: f32,
}

impl ShapePaint {
    pub const fn fill(fill: RenderColor) -> ShapePaint {
        ShapePaint {
            fill,
            stroke: render_color::TRANSPARENT_COLOR,
            stroke_width_pixels: 0.0,
        }
    }

    pub const fn stroke(stroke: RenderColor, stroke_width_pixels: f32) -> ShapePaint {
        ShapePaint {
            fill: render_color::TRANSPARENT_COLOR,
            stroke,
            stroke_width_pixels,
        }
    }

    pub const fn fill_and_stroke(fill: RenderColor, stroke: RenderColor, stroke_width_pixels: f32) -> ShapePaint {
        ShapePaint {
            fill,
            stroke,
            stroke_width_pixels,
        }
    }
}

/// Field order and formats match `SHAPE_INSTANCE_ATTRIBUTES` and `ShapeInstanceInput` in `shape.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeInstance {
    /// World pixels.
    pub center: [f32; 2],
    pub half_extent: [f32; 2],
    pub fill_color: RenderColor,
    pub stroke_color: RenderColor,
    pub stroke_width_pixels: f32,
    pub shape_kind: u32,
}

const _: () = assert!(std::mem::size_of::<ShapeInstance>() == 32);

impl ShapeInstance {
    pub fn new(geometry: ShapeGeometry, paint: ShapePaint) -> ShapeInstance {
        ShapeInstance {
            center: [geometry.center.x, geometry.center.y],
            half_extent: [geometry.half_width_pixels, geometry.half_height_pixels],
            fill_color: paint.fill,
            stroke_color: paint.stroke,
            stroke_width_pixels: paint.stroke_width_pixels,
            shape_kind: geometry.kind.code(),
        }
    }
}

pub fn create_shape_pipeline(
    device: &Device,
    pipeline_layout: &PipelineLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    scene_pipelines::create_instanced_quad_pipeline(
        device,
        pipeline_layout,
        surface_format,
        InstancedQuadPipelineSource {
            label: "bacter-shape-pipeline",
            shader_source: SHAPE_SHADER_SOURCE,
            vertex_entry_point: "shape_vertex_main",
            fragment_entry_point: "shape_fragment_main",
            instance_layout: VertexBufferLayout {
                array_stride: std::mem::size_of::<ShapeInstance>() as BufferAddress,
                step_mode: VertexStepMode::Instance,
                attributes: &SHAPE_INSTANCE_ATTRIBUTES,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_copies_the_geometry_and_the_paint() {
        let shape_instance: ShapeInstance = ShapeInstance::new(
            ShapeGeometry {
                kind: ShapeKind::Ellipse,
                center: InterpolatedPoint { x: 4.0, y: -2.5 },
                half_width_pixels: 60.0,
                half_height_pixels: 30.0,
            },
            ShapePaint::fill_and_stroke(RenderColor::opaque(1, 2, 3), RenderColor::opaque(4, 5, 6), 3.0),
        );

        assert_eq!(
            shape_instance,
            ShapeInstance {
                center: [4.0, -2.5],
                half_extent: [60.0, 30.0],
                fill_color: RenderColor::opaque(1, 2, 3),
                stroke_color: RenderColor::opaque(4, 5, 6),
                stroke_width_pixels: 3.0,
                shape_kind: 1,
            },
        );
    }

    #[test]
    fn fill_leaves_the_stroke_transparent() {
        assert_eq!(
            ShapePaint::fill(RenderColor::opaque(9, 9, 9)),
            ShapePaint {
                fill: RenderColor::opaque(9, 9, 9),
                stroke: render_color::TRANSPARENT_COLOR,
                stroke_width_pixels: 0.0,
            },
        );
    }

    #[test]
    fn stroke_leaves_the_fill_transparent() {
        assert_eq!(
            ShapePaint::stroke(RenderColor::opaque(9, 9, 9), 2.0),
            ShapePaint {
                fill: render_color::TRANSPARENT_COLOR,
                stroke: RenderColor::opaque(9, 9, 9),
                stroke_width_pixels: 2.0,
            },
        );
    }

    #[test]
    fn code_gives_every_shape_kind_its_own_value() {
        assert_eq!((ShapeKind::Rectangle.code(), ShapeKind::Ellipse.code()), (0, 1));
    }
}

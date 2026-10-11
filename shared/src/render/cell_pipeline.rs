use wgpu::{
    BufferAddress, Device, PipelineLayout, RenderPipeline, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexStepMode,
};

use crate::member::SkinKind;
use crate::play::InterpolatedPoint;
use crate::render::scene_pipelines;
use crate::render::{InstancedQuadPipelineSource, RenderColor};

const CELL_SHADER_SOURCE: &str = concat!(include_str!("scene_common.wgsl"), include_str!("cell.wgsl"));
const CELL_INSTANCE_ATTRIBUTES: [VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4, 2 => Uint32];
const INSTANCE_SKIN_KIND_MASK: u32 = 0xff;
const INSTANCE_SCALE_SHIFT: u32 = 16;

/// The codes match the `INSTANCE_SKIN_*` constants of `cell.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceSkinKind {
    Grid,
    Circles,
    Ghost,
    None,
    /// A filled square without a stroke.
    Solid,
}

impl InstanceSkinKind {
    pub fn for_cell(skin: SkinKind) -> InstanceSkinKind {
        match skin {
            SkinKind::Grid => InstanceSkinKind::Grid,
            SkinKind::Circles => InstanceSkinKind::Circles,
            SkinKind::Ghost => InstanceSkinKind::Ghost,
            SkinKind::None => InstanceSkinKind::None,
        }
    }

    /// The original drew the spores and shots of every other skin as unstroked squares.
    pub fn for_projectile(skin: SkinKind) -> InstanceSkinKind {
        match skin {
            SkinKind::Circles => InstanceSkinKind::Circles,
            SkinKind::Ghost => InstanceSkinKind::Ghost,
            SkinKind::Grid | SkinKind::None => InstanceSkinKind::Solid,
        }
    }

    pub fn code(self) -> u32 {
        match self {
            InstanceSkinKind::Grid => 0,
            InstanceSkinKind::Circles => 1,
            InstanceSkinKind::Ghost => 2,
            InstanceSkinKind::None => 3,
            InstanceSkinKind::Solid => 4,
        }
    }
}

/// Thousandths of a cell width; `cell.wgsl`'s `instance_scale` divides by `INSTANCE_SCALE_UNITS_PER_CELL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellScale(pub u16);

impl CellScale {
    pub const FULL: CellScale = CellScale(1000);
    /// The original drew shots at 0.8 of a cell.
    pub const SHOT: CellScale = CellScale(800);
}

/// Field order and formats match `CELL_INSTANCE_ATTRIBUTES` and `CellInstanceInput` in `cell.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    /// World pixels.
    pub center: [f32; 2],
    pub color: RenderColor,
    /// The skin code in the low byte, the scale in the high half.
    pub params: u32,
}

const _: () = assert!(std::mem::size_of::<CellInstance>() == 16);

impl CellInstance {
    pub fn new(
        center: InterpolatedPoint,
        color: RenderColor,
        skin: InstanceSkinKind,
        scale: CellScale,
    ) -> CellInstance {
        CellInstance {
            center: [center.x, center.y],
            color,
            params: skin.code() | u32::from(scale.0) << INSTANCE_SCALE_SHIFT,
        }
    }

    pub fn skin_code(&self) -> u32 {
        self.params & INSTANCE_SKIN_KIND_MASK
    }

    pub fn scale(&self) -> CellScale {
        CellScale((self.params >> INSTANCE_SCALE_SHIFT) as u16)
    }
}

pub fn create_cell_pipeline(
    device: &Device,
    pipeline_layout: &PipelineLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    scene_pipelines::create_instanced_quad_pipeline(
        device,
        pipeline_layout,
        surface_format,
        InstancedQuadPipelineSource {
            label: "bacter-cell-pipeline",
            shader_source: CELL_SHADER_SOURCE,
            vertex_entry_point: "cell_vertex_main",
            fragment_entry_point: "cell_fragment_main",
            instance_layout: VertexBufferLayout {
                array_stride: std::mem::size_of::<CellInstance>() as BufferAddress,
                step_mode: VertexStepMode::Instance,
                attributes: &CELL_INSTANCE_ATTRIBUTES,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::member;

    #[test]
    fn new_packs_the_skin_code_and_the_scale_into_params() {
        let cell_instance: CellInstance = CellInstance::new(
            InterpolatedPoint { x: 12.5, y: -3.0 },
            RenderColor::opaque(1, 2, 3),
            InstanceSkinKind::Ghost,
            CellScale::SHOT,
        );

        assert_eq!(cell_instance.center, [12.5, -3.0]);
        assert_eq!(cell_instance.params, 2 | 800 << 16);
        assert_eq!(cell_instance.skin_code(), InstanceSkinKind::Ghost.code());
        assert_eq!(cell_instance.scale(), CellScale::SHOT);
    }

    #[test]
    fn for_cell_keeps_every_skin() {
        let instance_skins: Vec<InstanceSkinKind> =
            member::SKIN_ORDER.into_iter().map(InstanceSkinKind::for_cell).collect();

        assert_eq!(
            instance_skins,
            vec![
                InstanceSkinKind::Grid,
                InstanceSkinKind::Circles,
                InstanceSkinKind::Ghost,
                InstanceSkinKind::None,
            ],
        );
    }

    #[test]
    fn for_projectile_draws_grid_and_none_as_solid_squares() {
        let instance_skins: Vec<InstanceSkinKind> =
            member::SKIN_ORDER.into_iter().map(InstanceSkinKind::for_projectile).collect();

        assert_eq!(
            instance_skins,
            vec![
                InstanceSkinKind::Solid,
                InstanceSkinKind::Circles,
                InstanceSkinKind::Ghost,
                InstanceSkinKind::Solid,
            ],
        );
    }

    #[test]
    fn code_gives_every_skin_its_own_byte() {
        let codes: Vec<u32> = [
            InstanceSkinKind::Grid,
            InstanceSkinKind::Circles,
            InstanceSkinKind::Ghost,
            InstanceSkinKind::None,
            InstanceSkinKind::Solid,
        ]
        .into_iter()
        .map(InstanceSkinKind::code)
        .collect();

        assert_eq!(codes, vec![0, 1, 2, 3, 4]);
    }
}

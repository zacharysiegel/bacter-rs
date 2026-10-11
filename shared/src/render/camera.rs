use crate::play::InterpolatedPoint;
use crate::play::local_game;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

impl PhysicalSize {
    /// A surface cannot be configured with a zero dimension.
    pub fn at_least_one_pixel(self) -> PhysicalSize {
        PhysicalSize {
            width_pixels: self.width_pixels.max(1),
            height_pixels: self.height_pixels.max(1),
        }
    }
}

/// One view pixel is one world pixel and `device_pixel_ratio` physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportSize {
    pub physical_size: PhysicalSize,
    pub device_pixel_ratio: f32,
}

impl ViewportSize {
    pub fn view_width_pixels(&self) -> f32 {
        self.physical_size.width_pixels as f32 / self.device_pixel_ratio
    }

    pub fn view_height_pixels(&self) -> f32 {
        self.physical_size.height_pixels as f32 / self.device_pixel_ratio
    }
}

/// View pixels from the view's top left corner, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPoint {
    pub x: f32,
    pub y: f32,
}

/// A diagonal linear map followed by a translation: `destination = scale * source + offset` on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisAlignedTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl AxisAlignedTransform {
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.scale_x * x + self.offset_x, self.scale_y * y + self.offset_y)
    }

    /// `self` first, then `next`.
    pub fn then(self, next: AxisAlignedTransform) -> AxisAlignedTransform {
        AxisAlignedTransform {
            scale_x: next.scale_x * self.scale_x,
            scale_y: next.scale_y * self.scale_y,
            offset_x: next.scale_x * self.offset_x + next.offset_x,
            offset_y: next.scale_y * self.offset_y + next.offset_y,
        }
    }

    /// Column-major, as WGSL's `mat4x4<f32>` is laid out.
    pub fn to_column_major_matrix(self) -> [[f32; 4]; 4] {
        [
            [self.scale_x, 0.0, 0.0, 0.0],
            [0.0, self.scale_y, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [self.offset_x, self.offset_y, 0.0, 1.0],
        ]
    }
}

/// Matches `CameraUniform` in `scene_common.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub world_to_clip: [[f32; 4]; 4],
    pub physical_pixels_per_world_pixel: f32,
    pub padding: [f32; 3],
}

const _: () = assert!(std::mem::size_of::<CameraUniform>() % 16 == 0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The world point at the view's top left corner.
    pub view_origin: InterpolatedPoint,
    pub viewport: ViewportSize,
}

impl Camera {
    pub fn centred_on(world_point: InterpolatedPoint, viewport: ViewportSize) -> Camera {
        Camera {
            view_origin: InterpolatedPoint {
                x: world_point.x - viewport.view_width_pixels() / 2.0,
                y: world_point.y - viewport.view_height_pixels() / 2.0,
            },
            viewport,
        }
    }

    /// A local game's world fills the window inside its margin.
    pub fn for_local_game(viewport: ViewportSize) -> Camera {
        let margin_pixels: f32 = local_game::LOCAL_WORLD_MARGIN_PIXELS as f32;

        Camera {
            view_origin: InterpolatedPoint {
                x: -margin_pixels,
                y: -margin_pixels,
            },
            viewport,
        }
    }

    pub fn world_to_view(&self) -> AxisAlignedTransform {
        AxisAlignedTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: -self.view_origin.x,
            offset_y: -self.view_origin.y,
        }
    }

    /// Orthographic; clip space has y up.
    pub fn view_to_clip(&self) -> AxisAlignedTransform {
        AxisAlignedTransform {
            scale_x: 2.0 / self.viewport.view_width_pixels(),
            scale_y: -2.0 / self.viewport.view_height_pixels(),
            offset_x: -1.0,
            offset_y: 1.0,
        }
    }

    pub fn world_to_clip(&self) -> AxisAlignedTransform {
        self.world_to_view().then(self.view_to_clip())
    }

    pub fn get_view_point(&self, world_point: InterpolatedPoint) -> ViewPoint {
        let (x, y): (f32, f32) = self.world_to_view().apply(world_point.x, world_point.y);

        ViewPoint { x, y }
    }

    pub fn to_uniform(&self) -> CameraUniform {
        CameraUniform {
            world_to_clip: self.world_to_clip().to_column_major_matrix(),
            physical_pixels_per_world_pixel: self.viewport.device_pixel_ratio,
            padding: [0.0; 3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 512 by 256 view pixels.
    const VIEWPORT: ViewportSize = ViewportSize {
        physical_size: PhysicalSize {
            width_pixels: 1024,
            height_pixels: 512,
        },
        device_pixel_ratio: 2.0,
    };

    #[test]
    fn view_width_pixels_divides_the_physical_size_by_the_device_pixel_ratio() {
        assert_eq!(
            (VIEWPORT.view_width_pixels(), VIEWPORT.view_height_pixels()),
            (512.0, 256.0),
        );
    }

    #[test]
    fn at_least_one_pixel_raises_only_zero_dimensions() {
        assert_eq!(
            PhysicalSize {
                width_pixels: 0,
                height_pixels: 7,
            }
            .at_least_one_pixel(),
            PhysicalSize {
                width_pixels: 1,
                height_pixels: 7,
            },
        );
    }

    #[test]
    fn centred_on_puts_the_world_point_at_the_view_centre() {
        let camera: Camera = Camera::centred_on(InterpolatedPoint { x: 1000.5, y: -40.0 }, VIEWPORT);

        assert_eq!(
            camera.get_view_point(InterpolatedPoint { x: 1000.5, y: -40.0 }),
            ViewPoint { x: 256.0, y: 128.0 },
        );
        assert_eq!(camera.view_origin, InterpolatedPoint { x: 744.5, y: -168.0 });
    }

    #[test]
    fn for_local_game_puts_the_world_origin_at_the_margin() {
        let camera: Camera = Camera::for_local_game(VIEWPORT);

        assert_eq!(
            camera.get_view_point(InterpolatedPoint { x: 0.0, y: 0.0 }),
            ViewPoint { x: 25.0, y: 25.0 },
        );
    }

    #[test]
    fn world_to_clip_maps_the_view_corners_to_the_clip_corners() {
        let camera: Camera = Camera::centred_on(InterpolatedPoint { x: 300.0, y: 200.0 }, VIEWPORT);
        let world_to_clip: AxisAlignedTransform = camera.world_to_clip();

        assert_eq!(world_to_clip.apply(44.0, 72.0), (-1.0, 1.0));
        assert_eq!(world_to_clip.apply(556.0, 328.0), (1.0, -1.0));
        assert_eq!(world_to_clip.apply(300.0, 200.0), (0.0, 0.0));
    }

    #[test]
    fn then_applies_the_first_transform_first() {
        let doubling: AxisAlignedTransform = AxisAlignedTransform {
            scale_x: 2.0,
            scale_y: 2.0,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        let shifting: AxisAlignedTransform = AxisAlignedTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 3.0,
            offset_y: -1.0,
        };

        assert_eq!(doubling.then(shifting).apply(5.0, 5.0), (13.0, 9.0));
        assert_eq!(shifting.then(doubling).apply(5.0, 5.0), (16.0, 8.0));
    }

    #[test]
    fn to_uniform_holds_world_to_clip_column_major_and_the_device_pixel_ratio() {
        let camera: Camera = Camera::for_local_game(VIEWPORT);
        let camera_uniform: CameraUniform = camera.to_uniform();

        assert_eq!(
            camera_uniform.world_to_clip,
            [
                [2.0 / 512.0, 0.0, 0.0, 0.0],
                [0.0, -2.0 / 256.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [25.0 * 2.0 / 512.0 - 1.0, 1.0 - 25.0 * 2.0 / 256.0, 0.0, 1.0],
            ],
        );
        assert_eq!(camera_uniform.physical_pixels_per_world_pixel, 2.0);
    }
}

use crate::ability::Projectile;
use crate::geometry;
use crate::geometry::{SubpixelPoint, Subpixels, WorldPoint};

const SUBPIXELS_PER_PIXEL: f64 = geometry::SUBPIXELS_PER_PIXEL as f64;

/// World pixels, unrounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolatedPoint {
    pub x: f32,
    pub y: f32,
}

impl InterpolatedPoint {
    pub fn from_subpixel_point(point: SubpixelPoint) -> InterpolatedPoint {
        InterpolatedPoint {
            x: get_pixels(Subpixels(point.x)),
            y: get_pixels(Subpixels(point.y)),
        }
    }

    pub fn from_world_point(point: WorldPoint) -> InterpolatedPoint {
        InterpolatedPoint {
            x: point.x as f32,
            y: point.y as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolationFrame {
    /// 0 at the last applied tick, 1 one tick period later.
    pub alpha: f32,
}

impl InterpolationFrame {
    /// A flying projectile moved by exactly its velocity in the last applied tick.
    pub fn get_flying_projectile_position(&self, projectile: &Projectile) -> InterpolatedPoint {
        let remaining_proportion: f64 = 1.0 - f64::from(self.alpha);

        InterpolatedPoint {
            x: get_interpolated_pixels(projectile.position.x, projectile.velocity.x, remaining_proportion),
            y: get_interpolated_pixels(projectile.position.y, projectile.velocity.y, remaining_proportion),
        }
    }
}

pub fn get_pixels(subpixels: Subpixels) -> f32 {
    get_pixels_from_unrounded_subpixels(f64::from(subpixels.0))
}

fn get_interpolated_pixels(position: i32, velocity: i32, remaining_proportion: f64) -> f32 {
    let subpixels: f64 = f64::from(position) - f64::from(velocity) * remaining_proportion;

    get_pixels_from_unrounded_subpixels(subpixels)
}

fn get_pixels_from_unrounded_subpixels(subpixels: f64) -> f32 {
    (subpixels / SUBPIXELS_PER_PIXEL) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SubpixelVector;

    const PROJECTILE: Projectile = Projectile {
        position: SubpixelPoint { x: 10_240, y: 0 },
        velocity: SubpixelVector { x: 1024, y: -2048 },
    };

    #[test]
    fn get_flying_projectile_position_moves_from_the_previous_tick_position_to_the_current_one() {
        let positions: Vec<InterpolatedPoint> = [0.0, 0.5, 1.0]
            .into_iter()
            .map(|alpha| InterpolationFrame { alpha }.get_flying_projectile_position(&PROJECTILE))
            .collect();

        assert_eq!(
            positions,
            vec![
                InterpolatedPoint { x: 9.0, y: 2.0 },
                InterpolatedPoint { x: 9.5, y: 1.0 },
                InterpolatedPoint { x: 10.0, y: 0.0 },
            ],
        );
    }

    #[test]
    fn from_subpixel_point_converts_to_unrounded_pixels() {
        assert_eq!(
            InterpolatedPoint::from_subpixel_point(SubpixelPoint { x: 1536, y: -256 }),
            InterpolatedPoint { x: 1.5, y: -0.25 },
        );
    }

    #[test]
    fn from_world_point_keeps_the_pixel_coordinates() {
        assert_eq!(
            InterpolatedPoint::from_world_point(WorldPoint { x: 106, y: -3 }),
            InterpolatedPoint { x: 106.0, y: -3.0 },
        );
    }

    #[test]
    fn get_pixels_keeps_partial_pixels() {
        assert_eq!(get_pixels(Subpixels(geometry::SUBPIXELS_PER_PIXEL * 3 / 4)), 0.75);
    }
}

use std::f32::consts::SQRT_2;

use crate::ability::{Loadout, ShotPhase, SporePhase};
use crate::game::GameState;
use crate::geometry;
use crate::geometry::WorldPoint;
use crate::member::SkinKind;
use crate::organism::Organism;
use crate::play::InterpolatedPoint;
use crate::play::interpolation;
use crate::render::render_color;
use crate::render::{
    CellInstance, CellScale, InstanceSkinKind, RenderColor, ShapeGeometry, ShapeInstance, ShapeKind, ShapePaint,
};
use crate::world::{World, WorldShapeKind};

/// The original's `cell_width / cos 45° * 2.9`.
pub const SPORE_SECRETION_RADIUS_PIXELS: f32 = geometry::CELL_WIDTH_PIXELS as f32 * SQRT_2 * 2.9;
pub const SHOT_SECRETION_RADIUS_PIXELS: f32 = SPORE_SECRETION_RADIUS_PIXELS / 2.0;
/// The square root of `ability_constants::FIELD_RADIUS_SQUARED_PIXELS`.
pub const FIELD_RADIUS_PIXELS: f32 = 60.0;
const FIELD_RING_WIDTH_PIXELS: f32 = 3.0;
const WORLD_BORDER_WIDTH_PIXELS: f32 = 1.0;
const GHOST_SECRETION_RING_WIDTH_PIXELS: f32 = 2.0;
const RECTANGLE_WORLD_SHADOW_OFFSET: InterpolatedPoint = InterpolatedPoint { x: 7.0, y: 6.0 };
const ELLIPSE_WORLD_SHADOW_OFFSET: InterpolatedPoint = InterpolatedPoint { x: 5.0, y: 4.0 };

/// Instances which change only when a tick is applied; drawn under the frame layer.
#[derive(Clone, Debug, PartialEq)]
pub struct TickLayer {
    /// In draw order: world shadow, world, toxin fields, secretions, neutralize fields.
    pub background_shapes: Vec<ShapeInstance>,
    pub organism_cells: Vec<CellInstance>,
}

impl TickLayer {
    pub fn from_state(state: &GameState) -> TickLayer {
        let rendered_organisms: Vec<RenderedOrganism> = get_rendered_organisms(state);
        let mut background_shapes: Vec<ShapeInstance> = get_world_shapes(&state.world);

        background_shapes.extend(rendered_organisms.iter().filter_map(RenderedOrganism::get_toxin_field_shape));
        background_shapes.extend(rendered_organisms.iter().flat_map(RenderedOrganism::get_secretion_shapes));
        background_shapes.extend(rendered_organisms.iter().filter_map(RenderedOrganism::get_neutralize_field_shape));

        let organism_cells: Vec<CellInstance> =
            rendered_organisms.iter().flat_map(RenderedOrganism::get_cell_instances).collect();

        TickLayer {
            background_shapes,
            organism_cells,
        }
    }
}

/// An organism with the loadout it is drawn in.
struct RenderedOrganism<'state> {
    organism: &'state Organism,
    loadout: &'state Loadout,
}

impl RenderedOrganism<'_> {
    fn color(&self) -> RenderColor {
        render_color::get_organism_color(self.loadout.appearance.color)
    }

    fn skin(&self) -> SkinKind {
        self.loadout.appearance.skin
    }

    fn get_toxin_field_shape(&self) -> Option<ShapeInstance> {
        let field_center: WorldPoint = self.organism.abilities.get_toxin_field_center(self.loadout)?;

        Some(get_field_shape(field_center, render_color::TOXIN_RING_COLOR))
    }

    fn get_neutralize_field_shape(&self) -> Option<ShapeInstance> {
        let field_center: WorldPoint = self.organism.abilities.get_neutralize_field_center(self.loadout)?;

        Some(get_field_shape(field_center, render_color::NEUTRALIZE_RING_COLOR))
    }

    /// Spore secretions, then shot secretions in slot order.
    fn get_secretion_shapes(&self) -> Vec<ShapeInstance> {
        let mut secretion_shapes: Vec<ShapeInstance> = Vec::new();

        if let SporePhase::Secreting { spores, .. } = &self.organism.abilities.spore {
            secretion_shapes.extend(spores.iter().map(|spore| {
                self.get_secretion_shape(
                    InterpolatedPoint::from_subpixel_point(spore.position),
                    SPORE_SECRETION_RADIUS_PIXELS,
                )
            }));
        }

        for shot_phase in &self.organism.abilities.shots {
            if let ShotPhase::Secreting { center, .. } = shot_phase {
                secretion_shapes.push(self.get_secretion_shape(
                    InterpolatedPoint::from_subpixel_point(*center),
                    SHOT_SECRETION_RADIUS_PIXELS,
                ));
            }
        }

        secretion_shapes
    }

    fn get_secretion_shape(&self, center: InterpolatedPoint, radius_pixels: f32) -> ShapeInstance {
        let paint: ShapePaint = match self.skin() {
            SkinKind::Ghost => ShapePaint::stroke(self.color(), GHOST_SECRETION_RING_WIDTH_PIXELS),
            SkinKind::Grid | SkinKind::Circles | SkinKind::None => ShapePaint::fill(self.color()),
        };

        ShapeInstance::new(get_circle_geometry(center, radius_pixels), paint)
    }

    fn get_cell_instances(&self) -> Vec<CellInstance> {
        let skin: InstanceSkinKind = InstanceSkinKind::for_cell(self.skin());

        self.organism
            .cells
            .iter()
            .map(|lattice_coordinate| {
                CellInstance::new(
                    InterpolatedPoint::from_world_point(self.organism.cell_center(lattice_coordinate)),
                    self.color(),
                    skin,
                    CellScale::FULL,
                )
            })
            .collect()
    }
}

/// Members with an organism, in member id order.
fn get_rendered_organisms(state: &GameState) -> Vec<RenderedOrganism<'_>> {
    state
        .members
        .values()
        .filter_map(|member| {
            Some(RenderedOrganism {
                organism: member.organism.as_ref()?,
                loadout: member.loadout.as_ref()?,
            })
        })
        .collect()
}

/// The shadow, then the world body with its border.
fn get_world_shapes(world: &World) -> Vec<ShapeInstance> {
    let half_width_pixels: f32 = interpolation::get_pixels(world.bounds.width) / 2.0;
    let half_height_pixels: f32 = interpolation::get_pixels(world.bounds.height) / 2.0;
    let world_center: InterpolatedPoint = InterpolatedPoint {
        x: interpolation::get_pixels(world.bounds.left) + half_width_pixels,
        y: interpolation::get_pixels(world.bounds.top) + half_height_pixels,
    };
    let (shape_kind, shadow_offset): (ShapeKind, InterpolatedPoint) = match world.shape {
        WorldShapeKind::Rectangle => (ShapeKind::Rectangle, RECTANGLE_WORLD_SHADOW_OFFSET),
        WorldShapeKind::Ellipse => (ShapeKind::Ellipse, ELLIPSE_WORLD_SHADOW_OFFSET),
    };
    let world_geometry: ShapeGeometry = ShapeGeometry {
        kind: shape_kind,
        center: world_center,
        half_width_pixels,
        half_height_pixels,
    };
    let shadow_geometry: ShapeGeometry = ShapeGeometry {
        kind: shape_kind,
        center: InterpolatedPoint {
            x: world_center.x + shadow_offset.x,
            y: world_center.y + shadow_offset.y,
        },
        half_width_pixels,
        half_height_pixels,
    };

    vec![
        ShapeInstance::new(shadow_geometry, ShapePaint::fill(render_color::SHADOW_COLOR)),
        ShapeInstance::new(
            world_geometry,
            ShapePaint::fill_and_stroke(
                render_color::WORLD_BODY_COLOR,
                render_color::WORLD_BORDER_COLOR,
                WORLD_BORDER_WIDTH_PIXELS,
            ),
        ),
    ]
}

fn get_field_shape(field_center: WorldPoint, ring_color: RenderColor) -> ShapeInstance {
    ShapeInstance::new(
        get_circle_geometry(InterpolatedPoint::from_world_point(field_center), FIELD_RADIUS_PIXELS),
        ShapePaint::fill_and_stroke(render_color::FIELD_FILL_COLOR, ring_color, FIELD_RING_WIDTH_PIXELS),
    )
}

fn get_circle_geometry(center: InterpolatedPoint, radius_pixels: f32) -> ShapeGeometry {
    ShapeGeometry {
        kind: ShapeKind::Ellipse,
        center,
        half_width_pixels: radius_pixels,
        half_height_pixels: radius_pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, Projectile, ThirdAbilityKind, ability_constants};
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, Subpixels};
    use crate::member::{Member, MemberId, OrganismColorKind};

    const OCEAN_COLOR: RenderColor = RenderColor::opaque(102, 136, 244);
    /// At (10, 0) px; at (9.5, 1) px half a tick earlier.
    const PROJECTILE: Projectile = Projectile {
        position: SubpixelPoint { x: 10_240, y: 0 },
        velocity: SubpixelVector { x: 1024, y: -2048 },
    };

    fn create_state_with_one_organism() -> GameState {
        test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }])
    }

    fn get_member_mut(state: &mut GameState, member_id: MemberId) -> &mut Member {
        state.members.get_mut(&member_id).unwrap()
    }

    fn create_circle(center: InterpolatedPoint, radius_pixels: f32, paint: ShapePaint) -> ShapeInstance {
        ShapeInstance::new(get_circle_geometry(center, radius_pixels), paint)
    }

    #[test]
    fn tick_layer_from_state_draws_the_shadow_then_the_world_of_a_rectangle_world() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(
            tick_layer,
            TickLayer {
                background_shapes: vec![
                    ShapeInstance {
                        center: [407.0, 406.0],
                        half_extent: [400.0, 400.0],
                        fill_color: RenderColor::opaque(50, 50, 50),
                        stroke_color: render_color::TRANSPARENT_COLOR,
                        stroke_width_pixels: 0.0,
                        shape_kind: ShapeKind::Rectangle.code(),
                    },
                    ShapeInstance {
                        center: [400.0, 400.0],
                        half_extent: [400.0, 400.0],
                        fill_color: RenderColor::opaque(0, 0, 0),
                        stroke_color: RenderColor::opaque(210, 210, 210),
                        stroke_width_pixels: 1.0,
                        shape_kind: ShapeKind::Rectangle.code(),
                    },
                ],
                organism_cells: Vec::new(),
            },
        );
    }

    #[test]
    fn tick_layer_from_state_offsets_the_shadow_of_an_ellipse_world_less() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 800);

        let tick_layer: TickLayer = TickLayer::from_state(&state);
        let shadow: ShapeInstance = tick_layer.background_shapes[0];

        assert_eq!(shadow.center, [405.0, 404.0]);
        assert_eq!(shadow.shape_kind, ShapeKind::Ellipse.code());
        assert_eq!(tick_layer.background_shapes[1].shape_kind, ShapeKind::Ellipse.code());
    }

    #[test]
    fn tick_layer_from_state_follows_the_shrunk_world_bounds() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.world.bounds.left = Subpixels(geometry::SUBPIXELS_PER_PIXEL / 2);
        state.world.bounds.width = Subpixels(799 * geometry::SUBPIXELS_PER_PIXEL);

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(tick_layer.background_shapes[1].center, [400.0, 400.0]);
        assert_eq!(tick_layer.background_shapes[1].half_extent, [399.5, 400.0]);
    }

    #[test]
    fn tick_layer_from_state_draws_every_cell_in_the_loadout_appearance() {
        let mut state: GameState = create_state_with_one_organism();
        test_fixture::get_organism_mut(&mut state, MemberId(0))
            .cells
            .insert(LatticeCoordinate { i: 1, j: 0 });

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(
            tick_layer.organism_cells,
            vec![
                CellInstance::new(
                    InterpolatedPoint { x: 100.0, y: 100.0 },
                    OCEAN_COLOR,
                    InstanceSkinKind::Circles,
                    CellScale::FULL,
                ),
                CellInstance::new(
                    InterpolatedPoint { x: 106.0, y: 100.0 },
                    OCEAN_COLOR,
                    InstanceSkinKind::Circles,
                    CellScale::FULL,
                ),
            ],
        );
    }

    #[test]
    fn tick_layer_from_state_skips_members_without_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(MemberId(0), test_fixture::create_participant(MemberId(0)));
        state.members.insert(MemberId(1), test_fixture::create_spectator(MemberId(1)));

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(tick_layer.background_shapes.len(), 2);
        assert_eq!(tick_layer.organism_cells, Vec::new());
    }

    #[test]
    fn tick_layer_from_state_draws_toxin_fields_then_secretions_then_neutralize_fields() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );

        let neutralizer: &mut Member = get_member_mut(&mut state, MemberId(0));
        let neutralizer_organism: &mut Organism = neutralizer.organism.as_mut().unwrap();
        neutralizer_organism.abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        neutralizer_organism.abilities.third_center = Some(WorldPoint { x: 100, y: 100 });

        let poisoner: &mut Member = get_member_mut(&mut state, MemberId(1));
        poisoner.loadout.as_mut().unwrap().third = ThirdAbilityKind::Toxin;
        poisoner.loadout.as_mut().unwrap().appearance.color = OrganismColorKind::Fire;

        let poisoner_organism: &mut Organism = poisoner.organism.as_mut().unwrap();
        poisoner_organism.abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        poisoner_organism.abilities.third_center = Some(WorldPoint { x: 300, y: 300 });

        poisoner_organism.abilities.spore = SporePhase::Secreting {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };
        poisoner_organism.abilities.shots[1] = ShotPhase::Secreting {
            ends_at: Tick(9),
            center: SubpixelPoint { x: 2048, y: 3072 },
        };

        let tick_layer: TickLayer = TickLayer::from_state(&state);
        let fire_color: RenderColor = RenderColor::opaque(255, 90, 81);

        assert_eq!(
            tick_layer.background_shapes[2..],
            [
                create_circle(
                    InterpolatedPoint { x: 300.0, y: 300.0 },
                    60.0,
                    ShapePaint::fill_and_stroke(
                        RenderColor::opaque(100, 100, 100),
                        RenderColor::opaque(255, 111, 92),
                        3.0,
                    ),
                ),
                create_circle(
                    InterpolatedPoint { x: 10.0, y: 0.0 },
                    SPORE_SECRETION_RADIUS_PIXELS,
                    ShapePaint::fill(fire_color),
                ),
                create_circle(
                    InterpolatedPoint { x: 2.0, y: 3.0 },
                    SHOT_SECRETION_RADIUS_PIXELS,
                    ShapePaint::fill(fire_color),
                ),
                create_circle(
                    InterpolatedPoint { x: 100.0, y: 100.0 },
                    60.0,
                    ShapePaint::fill_and_stroke(
                        RenderColor::opaque(100, 100, 100),
                        RenderColor::opaque(0, 179, 12),
                        3.0,
                    ),
                ),
            ],
        );
    }

    #[test]
    fn tick_layer_from_state_strokes_the_secretions_of_a_ghost_skin() {
        let mut state: GameState = create_state_with_one_organism();
        let member: &mut Member = get_member_mut(&mut state, MemberId(0));
        member.loadout.as_mut().unwrap().appearance.skin = SkinKind::Ghost;
        member.organism.as_mut().unwrap().abilities.spore = SporePhase::Secreting {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(
            tick_layer.background_shapes[2..],
            [create_circle(
                InterpolatedPoint { x: 10.0, y: 0.0 },
                SPORE_SECRETION_RADIUS_PIXELS,
                ShapePaint::stroke(OCEAN_COLOR, 2.0),
            )],
        );
    }

    #[test]
    fn secretion_and_field_radii_match_the_simulation_radii() {
        let subpixels_per_pixel: f64 = f64::from(geometry::SUBPIXELS_PER_PIXEL);
        let spore_radius_squared_subpixels: f64 =
            (f64::from(SPORE_SECRETION_RADIUS_PIXELS) * subpixels_per_pixel).powi(2);
        let shot_radius_squared_subpixels: f64 =
            (f64::from(SHOT_SECRETION_RADIUS_PIXELS) * subpixels_per_pixel).powi(2);

        assert!(
            (spore_radius_squared_subpixels / ability_constants::SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS as f64 - 1.0)
                .abs()
                < 1e-6
        );
        assert!(
            (shot_radius_squared_subpixels / ability_constants::SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS as f64 - 1.0)
                .abs()
                < 1e-6
        );
        assert_eq!(
            (FIELD_RADIUS_PIXELS * FIELD_RADIUS_PIXELS) as i64,
            ability_constants::FIELD_RADIUS_SQUARED_PIXELS,
        );
    }
}

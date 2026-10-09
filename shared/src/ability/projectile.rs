use std::cmp::Ordering;

use crate::ability::ability_constants;
use crate::ability::{AimVector, Projectile};
use crate::geometry::{LatticeCoordinate, SubpixelVector};
use crate::organism::{LatticeCentroid, Organism};

/// Every exposed cell leaves the organism in lattice order; each flies away from the centroid of the cells still
/// in the organism, and one with no offset from it is dropped without a projectile.
pub fn launch_spores(organism: &mut Organism) -> Vec<Projectile> {
    let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();
    let mut centroid: LatticeCentroid = organism.centroid();
    let mut spores: Vec<Projectile> = Vec::new();

    for lattice_coordinate in exposed_cells {
        let (offset_i, offset_j): (i64, i64) = centroid.get_scaled_offset(lattice_coordinate);
        centroid = centroid.without(lattice_coordinate);
        organism.cells.remove(lattice_coordinate);

        if offset_i == 0 && offset_j == 0 {
            continue;
        }

        spores.push(Projectile {
            position: organism.cell_center(lattice_coordinate).to_subpixel_point(),
            velocity: get_scaled_direction(offset_i, offset_j, ability_constants::SPORE_SPEED_SUBPIXELS_PER_TICK),
        });
    }

    spores
}

/// `None` for a zero aim or an organism without cells; the chosen cell leaves the organism and flies along `aim`.
pub fn launch_shot(organism: &mut Organism, aim: AimVector) -> Option<Projectile> {
    if aim.is_zero() {
        return None;
    }

    let lattice_coordinate: LatticeCoordinate = select_shot_cell(organism, aim)?;
    organism.cells.remove(lattice_coordinate);

    Some(Projectile {
        position: organism.cell_center(lattice_coordinate).to_subpixel_point(),
        velocity: get_scaled_direction(
            i64::from(aim.x),
            i64::from(aim.y),
            ability_constants::SHOT_SPEED_SUBPIXELS_PER_TICK,
        ),
    })
}

/// The exposed cell whose offset from the centroid has the largest cosine with `aim`, the first in lattice order
/// on a tie; a cell at the centroid is chosen only when it is the only exposed cell.
pub fn select_shot_cell(organism: &Organism, aim: AimVector) -> Option<LatticeCoordinate> {
    let centroid: LatticeCentroid = organism.centroid();
    let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();
    let offset_cells: Vec<(LatticeCoordinate, (i64, i64))> = exposed_cells
        .iter()
        .map(|lattice_coordinate| (*lattice_coordinate, centroid.get_scaled_offset(*lattice_coordinate)))
        .filter(|(_, offset)| *offset != (0, 0))
        .collect();

    let Some(first_offset_cell) = offset_cells.first() else {
        return exposed_cells.first().copied();
    };

    let mut best_offset_cell: (LatticeCoordinate, (i64, i64)) = *first_offset_cell;

    for offset_cell in &offset_cells[1..] {
        let alignment_ordering: Ordering = compare_alignment(offset_cell.1, best_offset_cell.1, aim);

        if alignment_ordering == Ordering::Greater {
            best_offset_cell = *offset_cell;
        }
    }

    Some(best_offset_cell.0)
}

/// `speed` along the non-zero `(x, y)`, each component rounded once.
pub fn get_scaled_direction(x: i64, y: i64, speed: i32) -> SubpixelVector {
    // Exact: offsets and aims are far below 2^53.
    let x_float: f64 = x as f64;
    let y_float: f64 = y as f64;
    let length: f64 = libm::sqrt(x_float * x_float + y_float * y_float);
    let speed_float: f64 = f64::from(speed);

    // Bounded by `speed`.
    SubpixelVector {
        x: libm::round(x_float / length * speed_float) as i32,
        y: libm::round(y_float / length * speed_float) as i32,
    }
}

/// Orders the cosines of `first` and `second` with `aim`, exactly.
fn compare_alignment(first: (i64, i64), second: (i64, i64), aim: AimVector) -> Ordering {
    let first_dot: i128 = get_dot_product(first, aim);
    let second_dot: i128 = get_dot_product(second, aim);
    let sign_ordering: Ordering = first_dot.signum().cmp(&second_dot.signum());

    if sign_ordering != Ordering::Equal {
        return sign_ordering;
    }

    // Exact while offset components stay below 2^23; saturates beyond.
    let first_scaled_square: i128 = first_dot.saturating_mul(first_dot).saturating_mul(get_length_squared(second));
    let second_scaled_square: i128 = second_dot.saturating_mul(second_dot).saturating_mul(get_length_squared(first));
    let magnitude_ordering: Ordering = first_scaled_square.cmp(&second_scaled_square);

    if first_dot < 0 {
        return magnitude_ordering.reverse();
    }

    magnitude_ordering
}

fn get_dot_product(offset: (i64, i64), aim: AimVector) -> i128 {
    i128::from(offset.0) * i128::from(aim.x) + i128::from(offset.1) * i128::from(aim.y)
}

fn get_length_squared(offset: (i64, i64)) -> i128 {
    i128::from(offset.0) * i128::from(offset.0) + i128::from(offset.1) * i128::from(offset.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{SubpixelPoint, WorldPoint};
    use crate::organism::CellOccupancy;

    fn create_organism(lattice_coordinates: &[(i32, i32)]) -> Organism {
        let mut organism: Organism = Organism::new(WorldPoint { x: 100, y: 100 });
        organism.cells = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }

        organism
    }

    fn get_cells(organism: &Organism) -> Vec<LatticeCoordinate> {
        organism.cells.iter().collect()
    }

    #[test]
    fn get_scaled_direction_keeps_axis_directions_exact() {
        assert_eq!(get_scaled_direction(5, 0, 10_752), SubpixelVector { x: 10_752, y: 0 },);
        assert_eq!(get_scaled_direction(0, -1, 8960), SubpixelVector { x: 0, y: -8960 },);
    }

    #[test]
    fn get_scaled_direction_rounds_each_component_once() {
        assert_eq!(get_scaled_direction(1, 1, 10_752), SubpixelVector { x: 7603, y: 7603 },);
        assert_eq!(get_scaled_direction(-3, 4, 8960), SubpixelVector { x: -5376, y: 7168 },);
    }

    #[test]
    fn launch_spores_uses_the_progressive_centroid() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert!(organism.cells.is_empty());
        assert_eq!(
            spores,
            vec![
                Projectile {
                    position: SubpixelPoint { x: 102_400, y: 102_400 },
                    velocity: SubpixelVector { x: -10_752, y: 0 },
                },
                Projectile {
                    position: SubpixelPoint { x: 108_544, y: 102_400 },
                    velocity: SubpixelVector { x: -10_752, y: 0 },
                },
            ],
        );
    }

    #[test]
    fn launch_spores_keeps_enclosed_cells() {
        let block: Vec<(i32, i32)> = (-1..=1).flat_map(|j| (-1..=1).map(move |i| (i, j))).collect();
        let mut organism: Organism = create_organism(&block);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert_eq!(get_cells(&organism), vec![LatticeCoordinate { i: 0, j: 0 }]);
        assert_eq!(spores.len(), 8);
        assert_eq!(spores[0].velocity, SubpixelVector { x: -7603, y: -7603 });
        assert_eq!(spores[1].velocity, SubpixelVector { x: -1187, y: -10_686 });
    }

    #[test]
    fn launch_spores_drops_a_spore_at_the_centroid() {
        let mut organism: Organism = create_organism(&[(0, 0)]);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert_eq!(spores, Vec::new());
        assert!(organism.cells.is_empty());
    }

    #[test]
    fn select_shot_cell_maximises_the_cosine_with_the_aim() {
        let organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0), (1, 1), (1, -1)]);

        assert_eq!(
            select_shot_cell(&organism, AimVector { x: 10, y: 1 }),
            Some(LatticeCoordinate { i: 2, j: 0 }),
        );
        assert_eq!(
            select_shot_cell(&organism, AimVector { x: 1, y: -10 }),
            Some(LatticeCoordinate { i: 1, j: -1 }),
        );
    }

    #[test]
    fn select_shot_cell_handles_aims_across_the_negative_x_axis() {
        let organism: Organism = create_organism(&[(0, 0), (1, 0), (-2, -1), (-2, 1)]);

        assert_eq!(
            select_shot_cell(&organism, AimVector { x: -20, y: 1 }),
            Some(LatticeCoordinate { i: -2, j: 1 }),
        );
        assert_eq!(
            select_shot_cell(&organism, AimVector { x: -20, y: -1 }),
            Some(LatticeCoordinate { i: -2, j: -1 }),
        );
    }

    #[test]
    fn select_shot_cell_breaks_ties_in_lattice_order() {
        let organism: Organism = create_organism(&[(0, -1), (0, 0), (0, 1)]);

        assert_eq!(
            select_shot_cell(&organism, AimVector { x: 1, y: 0 }),
            Some(LatticeCoordinate { i: 0, j: -1 }),
        );
    }

    #[test]
    fn select_shot_cell_takes_a_centroid_cell_only_when_alone() {
        let single_cell_organism: Organism = create_organism(&[(4, 4)]);
        let centered_organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        assert_eq!(
            select_shot_cell(&single_cell_organism, AimVector { x: 0, y: 1 }),
            Some(LatticeCoordinate { i: 4, j: 4 }),
        );
        assert_eq!(
            select_shot_cell(&centered_organism, AimVector { x: 0, y: 1 }),
            Some(LatticeCoordinate { i: 0, j: 0 }),
        );
    }

    #[test]
    fn launch_shot_flies_along_the_aim_from_the_chosen_cell() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        let shot: Option<Projectile> = launch_shot(&mut organism, AimVector { x: 0, y: 7 });

        assert_eq!(
            shot,
            Some(Projectile {
                position: SubpixelPoint { x: 102_400, y: 102_400 },
                velocity: SubpixelVector { x: 0, y: 8960 },
            }),
        );
        assert_eq!(
            get_cells(&organism),
            vec![LatticeCoordinate { i: 1, j: 0 }, LatticeCoordinate { i: 2, j: 0 }],
        );
    }

    #[test]
    fn launch_shot_ignores_a_zero_aim() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0)]);

        assert_eq!(launch_shot(&mut organism, AimVector { x: 0, y: 0 }), None);
        assert_eq!(organism.cells.count(), 2);
    }

    #[test]
    fn launch_shot_can_take_the_last_cell() {
        let mut organism: Organism = create_organism(&[(0, 0)]);

        assert!(launch_shot(&mut organism, AimVector { x: 3, y: 0 }).is_some());
        assert!(organism.cells.is_empty());
    }
}

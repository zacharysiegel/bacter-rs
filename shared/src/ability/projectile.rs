use crate::ability::Projectile;
use crate::ability::ability_constants;
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
}

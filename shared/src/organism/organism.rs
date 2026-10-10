use crate::ability::OrganismAbilities;
use crate::geometry;
use crate::geometry::{LatticeCoordinate, WorldPoint};
use crate::member::MemberId;
use crate::organism::CellOccupancy;

/// The centroid of a cell set as the exact lattice sum over the cell count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatticeCentroid {
    pub sum_i: i64,
    pub sum_j: i64,
    pub cell_count: i64,
}

impl LatticeCentroid {
    /// `(cell - centroid) * cell_count`: the direction from the centroid, scaled to stay an integer.
    pub fn get_scaled_offset(self, lattice_coordinate: LatticeCoordinate) -> (i64, i64) {
        (
            i64::from(lattice_coordinate.i) * self.cell_count - self.sum_i,
            i64::from(lattice_coordinate.j) * self.cell_count - self.sum_j,
        )
    }

    pub fn without(self, lattice_coordinate: LatticeCoordinate) -> LatticeCentroid {
        LatticeCentroid {
            sum_i: self.sum_i - i64::from(lattice_coordinate.i),
            sum_j: self.sum_j - i64::from(lattice_coordinate.j),
            cell_count: self.cell_count - 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Organism {
    /// World position of lattice coordinate (0, 0).
    pub anchor: WorldPoint,
    pub cells: CellOccupancy,
    pub cursor: WorldPoint,
    pub last_hitter: Option<MemberId>,
    pub abilities: OrganismAbilities,
}

impl Organism {
    /// One cell at `position`, with the anchor and the cursor there.
    pub fn new(position: WorldPoint) -> Organism {
        Organism {
            anchor: position,
            cells: CellOccupancy::with_cell(LatticeCoordinate { i: 0, j: 0 }),
            cursor: position,
            last_hitter: None,
            abilities: OrganismAbilities::all_ready(),
        }
    }

    pub fn cell_center(&self, lattice_coordinate: LatticeCoordinate) -> WorldPoint {
        lattice_coordinate.to_cell_center(self.anchor)
    }

    pub fn centroid(&self) -> LatticeCentroid {
        let mut centroid: LatticeCentroid = LatticeCentroid {
            sum_i: 0,
            sum_j: 0,
            cell_count: 0,
        };

        for lattice_coordinate in self.cells.iter() {
            centroid.sum_i += i64::from(lattice_coordinate.i);
            centroid.sum_j += i64::from(lattice_coordinate.j);
            centroid.cell_count += 1;
        }

        centroid
    }

    /// Cells with fewer than four orthogonal own neighbours, in lattice order.
    pub fn exposed_cells(&self) -> Vec<LatticeCoordinate> {
        self.cells
            .iter()
            .filter(|lattice_coordinate| self.empty_neighbors(*lattice_coordinate).next().is_some())
            .collect()
    }

    /// For each cell in lattice order, each empty neighbour in direction order; a site next to k cells appears k
    /// times.
    pub fn adjacent_sites(&self) -> Vec<LatticeCoordinate> {
        self.cells.iter().flat_map(|lattice_coordinate| self.empty_neighbors(lattice_coordinate)).collect()
    }

    /// In direction order.
    fn empty_neighbors(&self, lattice_coordinate: LatticeCoordinate) -> impl Iterator<Item = LatticeCoordinate> + '_ {
        geometry::NEIGHBOR_DIRECTIONS
            .into_iter()
            .map(move |direction| lattice_coordinate.neighbor(direction))
            .filter(|neighbor| !self.cells.contains(*neighbor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_organism(lattice_coordinates: &[(i32, i32)]) -> Organism {
        let mut organism: Organism = Organism::new(WorldPoint { x: 0, y: 0 });
        organism.cells = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }

        organism
    }

    fn count_occurrences(lattice_coordinates: &[LatticeCoordinate], target: LatticeCoordinate) -> usize {
        lattice_coordinates.iter().filter(|lattice_coordinate| **lattice_coordinate == target).count()
    }

    #[test]
    fn new_places_one_cell_at_the_position() {
        let organism: Organism = Organism::new(WorldPoint { x: 40, y: 70 });

        assert_eq!(organism.anchor, WorldPoint { x: 40, y: 70 });
        assert_eq!(organism.cursor, WorldPoint { x: 40, y: 70 });
        assert_eq!(
            organism.cells.iter().collect::<Vec<LatticeCoordinate>>(),
            vec![LatticeCoordinate { i: 0, j: 0 }],
        );
        assert_eq!(
            organism.cell_center(LatticeCoordinate { i: 0, j: 0 }),
            WorldPoint { x: 40, y: 70 },
        );
        assert_eq!(organism.abilities, OrganismAbilities::all_ready());
        assert_eq!(organism.last_hitter, None);
    }

    #[test]
    fn exposed_cells_excludes_enclosed_cells() {
        let block: Vec<(i32, i32)> = (0..3).flat_map(|j| (0..3).map(move |i| (i, j))).collect();
        let organism: Organism = create_organism(&block);
        let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();

        assert_eq!(exposed_cells.len(), 8);
        assert!(!exposed_cells.contains(&LatticeCoordinate { i: 1, j: 1 }));
        assert_eq!(exposed_cells[0], LatticeCoordinate { i: 0, j: 0 });
        assert_eq!(exposed_cells[7], LatticeCoordinate { i: 2, j: 2 });
    }

    #[test]
    fn adjacent_sites_of_one_cell_follow_direction_order() {
        let organism: Organism = create_organism(&[(0, 0)]);

        assert_eq!(
            organism.adjacent_sites(),
            vec![
                LatticeCoordinate { i: -1, j: 0 },
                LatticeCoordinate { i: 0, j: -1 },
                LatticeCoordinate { i: 1, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn adjacent_sites_repeat_a_site_once_per_neighboring_cell() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 1)]);
        let adjacent_sites: Vec<LatticeCoordinate> = organism.adjacent_sites();

        assert_eq!(adjacent_sites.len(), 12);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: 1, j: 0 }), 3);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: 0, j: 1 }), 2);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: -1, j: 0 }), 1);
    }

    #[test]
    fn centroid_sums_the_lattice_coordinates() {
        let organism: Organism = create_organism(&[(0, 0), (3, 1), (-1, 2)]);

        assert_eq!(
            organism.centroid(),
            LatticeCentroid {
                sum_i: 2,
                sum_j: 3,
                cell_count: 3,
            },
        );
    }

    #[test]
    fn get_scaled_offset_points_away_from_the_centroid() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 3)]);
        let centroid: LatticeCentroid = organism.centroid();

        assert_eq!(centroid.get_scaled_offset(LatticeCoordinate { i: 2, j: 0 }), (3, -3));
        assert_eq!(centroid.get_scaled_offset(LatticeCoordinate { i: 1, j: 1 }), (0, 0));
    }

    #[test]
    fn without_removes_one_cell_from_the_centroid() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 3)]);
        let centroid: LatticeCentroid = organism.centroid().without(LatticeCoordinate { i: 1, j: 3 });

        assert_eq!(
            centroid,
            LatticeCentroid {
                sum_i: 2,
                sum_j: 0,
                cell_count: 2,
            },
        );
        assert_eq!(centroid.get_scaled_offset(LatticeCoordinate { i: 2, j: 0 }), (2, 0));
    }
}

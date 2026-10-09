use crate::geometry::LatticeCoordinate;

/// Equal cell sets are equal values once both are re-tightened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellOccupancy {
    origin: LatticeCoordinate,
    width: u16,
    height: u16,
    /// Row-major, `ceil(width / 8)` bytes per row; column `c` is bit `c % 8` of byte `c / 8`.
    row_bitmap_bytes: Vec<u8>,
}

impl CellOccupancy {
    pub fn empty() -> CellOccupancy {
        CellOccupancy {
            origin: LatticeCoordinate { i: 0, j: 0 },
            width: 0,
            height: 0,
            row_bitmap_bytes: Vec::new(),
        }
    }

    pub fn with_cell(lattice_coordinate: LatticeCoordinate) -> CellOccupancy {
        CellOccupancy {
            origin: lattice_coordinate,
            width: 1,
            height: 1,
            row_bitmap_bytes: vec![1],
        }
    }

    /// `None` unless the box is non-empty, every edge row and column holds a cell, and no bit past `width` is set.
    pub fn from_tight_bitmap(
        origin: LatticeCoordinate,
        width: u16,
        height: u16,
        row_bitmap_bytes: Vec<u8>,
    ) -> Option<CellOccupancy> {
        if width == 0 || height == 0 {
            return None;
        }

        let expected_byte_count: usize = get_bytes_per_row(width) * usize::from(height);

        if row_bitmap_bytes.len() != expected_byte_count {
            return None;
        }

        let candidate: CellOccupancy = CellOccupancy {
            origin,
            width,
            height,
            row_bitmap_bytes,
        };

        let has_padding_bits: bool = candidate.has_bits_past_width();
        let tightened_box: Option<CellBox> = candidate.get_tight_box();

        if has_padding_bits || tightened_box != Some(candidate.get_box()) {
            return None;
        }

        Some(candidate)
    }

    pub fn origin(&self) -> LatticeCoordinate {
        self.origin
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn row_bitmap_bytes(&self) -> &[u8] {
        &self.row_bitmap_bytes
    }

    pub fn contains(&self, lattice_coordinate: LatticeCoordinate) -> bool {
        let Some(bit_position) = self.get_bit_position(lattice_coordinate) else {
            return false;
        };

        self.row_bitmap_bytes[bit_position.byte_index] & bit_position.mask != 0
    }

    pub fn insert(&mut self, lattice_coordinate: LatticeCoordinate) {
        if self.get_bit_position(lattice_coordinate).is_none() {
            self.expand_to_include(lattice_coordinate);
        }

        let bit_position: BitPosition = self.get_bit_position(lattice_coordinate).unwrap();
        self.row_bitmap_bytes[bit_position.byte_index] |= bit_position.mask;
    }

    pub fn remove(&mut self, lattice_coordinate: LatticeCoordinate) {
        let Some(bit_position) = self.get_bit_position(lattice_coordinate) else {
            return;
        };

        self.row_bitmap_bytes[bit_position.byte_index] &= !bit_position.mask;
    }

    pub fn count(&self) -> u32 {
        self.row_bitmap_bytes.iter().map(|byte| byte.count_ones()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.row_bitmap_bytes.iter().all(|byte| *byte == 0)
    }

    /// Ascending `LatticeCoordinate` order.
    pub fn iter(&self) -> impl Iterator<Item = LatticeCoordinate> + '_ {
        let rows: std::ops::Range<i32> = 0..i32::from(self.height);

        rows.flat_map(move |row| {
            let columns: std::ops::Range<i32> = 0..i32::from(self.width);

            columns.map(move |column| LatticeCoordinate {
                i: self.origin.i + column,
                j: self.origin.j + row,
            })
        })
        .filter(|lattice_coordinate| self.contains(*lattice_coordinate))
    }

    pub fn retighten(&mut self) {
        let Some(tight_box) = self.get_tight_box() else {
            *self = CellOccupancy::empty();
            return;
        };

        if tight_box != self.get_box() {
            self.rebuild(tight_box);
        }
    }

    fn get_box(&self) -> CellBox {
        CellBox {
            minimum: self.origin,
            maximum: LatticeCoordinate {
                i: self.origin.i + i32::from(self.width) - 1,
                j: self.origin.j + i32::from(self.height) - 1,
            },
        }
    }

    fn get_tight_box(&self) -> Option<CellBox> {
        let lattice_coordinates: Vec<LatticeCoordinate> = self.iter().collect();
        let first: LatticeCoordinate = *lattice_coordinates.first()?;
        let mut tight_box: CellBox = CellBox {
            minimum: first,
            maximum: first,
        };

        for lattice_coordinate in &lattice_coordinates[1..] {
            tight_box = tight_box.including(*lattice_coordinate);
        }

        Some(tight_box)
    }

    fn expand_to_include(&mut self, lattice_coordinate: LatticeCoordinate) {
        let expanded_box: CellBox = match self.get_tight_box() {
            Some(current_box) => current_box.including(lattice_coordinate),
            None => CellBox {
                minimum: lattice_coordinate,
                maximum: lattice_coordinate,
            },
        };

        self.rebuild(expanded_box);
    }

    fn rebuild(&mut self, cell_box: CellBox) {
        let lattice_coordinates: Vec<LatticeCoordinate> = self.iter().collect();
        let width: u16 = u16::try_from(cell_box.maximum.i - cell_box.minimum.i + 1).unwrap();
        let height: u16 = u16::try_from(cell_box.maximum.j - cell_box.minimum.j + 1).unwrap();

        *self = CellOccupancy {
            origin: cell_box.minimum,
            width,
            height,
            row_bitmap_bytes: vec![0; get_bytes_per_row(width) * usize::from(height)],
        };

        for lattice_coordinate in lattice_coordinates {
            self.insert(lattice_coordinate);
        }
    }

    fn get_bit_position(&self, lattice_coordinate: LatticeCoordinate) -> Option<BitPosition> {
        let column: i32 = lattice_coordinate.i - self.origin.i;
        let row: i32 = lattice_coordinate.j - self.origin.j;
        let is_inside_box: bool =
            (0..i32::from(self.width)).contains(&column) && (0..i32::from(self.height)).contains(&row);

        if !is_inside_box {
            return None;
        }

        let column_index: usize = usize::try_from(column).unwrap();
        let row_index: usize = usize::try_from(row).unwrap();

        Some(BitPosition {
            byte_index: row_index * get_bytes_per_row(self.width) + column_index / 8,
            mask: 1 << (column_index % 8),
        })
    }

    fn has_bits_past_width(&self) -> bool {
        let used_bits_in_last_byte: u16 = self.width % 8;

        if used_bits_in_last_byte == 0 {
            return false;
        }

        let padding_mask: u8 = !((1_u8 << used_bits_in_last_byte) - 1);
        let bytes_per_row: usize = get_bytes_per_row(self.width);

        self.row_bitmap_bytes
            .chunks(bytes_per_row)
            .any(|row_bytes| row_bytes[bytes_per_row - 1] & padding_mask != 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellBox {
    minimum: LatticeCoordinate,
    maximum: LatticeCoordinate,
}

impl CellBox {
    fn including(self, lattice_coordinate: LatticeCoordinate) -> CellBox {
        CellBox {
            minimum: LatticeCoordinate {
                i: self.minimum.i.min(lattice_coordinate.i),
                j: self.minimum.j.min(lattice_coordinate.j),
            },
            maximum: LatticeCoordinate {
                i: self.maximum.i.max(lattice_coordinate.i),
                j: self.maximum.j.max(lattice_coordinate.j),
            },
        }
    }
}

struct BitPosition {
    byte_index: usize,
    mask: u8,
}

fn get_bytes_per_row(width: u16) -> usize {
    usize::from(width).div_ceil(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_occupancy(lattice_coordinates: &[(i32, i32)]) -> CellOccupancy {
        let mut cell_occupancy: CellOccupancy = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            cell_occupancy.insert(LatticeCoordinate { i: *i, j: *j });
        }

        cell_occupancy
    }

    #[test]
    fn insert_makes_cells_contained() {
        let cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (-3, 2), (9, -1)]);

        assert!(cell_occupancy.contains(LatticeCoordinate { i: -3, j: 2 }));
        assert!(cell_occupancy.contains(LatticeCoordinate { i: 9, j: -1 }));
        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 1, j: 0 }));
        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 40, j: 40 }));
        assert_eq!(cell_occupancy.count(), 3);
    }

    #[test]
    fn remove_clears_only_that_cell() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (1, 0)]);
        cell_occupancy.remove(LatticeCoordinate { i: 0, j: 0 });
        cell_occupancy.remove(LatticeCoordinate { i: 50, j: 50 });

        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 0, j: 0 }));
        assert!(cell_occupancy.contains(LatticeCoordinate { i: 1, j: 0 }));
        assert_eq!(cell_occupancy.count(), 1);
    }

    #[test]
    fn iter_yields_row_major_order() {
        let cell_occupancy: CellOccupancy = create_occupancy(&[(3, 0), (0, 1), (-2, 0), (1, -1), (12, 0)]);
        let lattice_coordinates: Vec<LatticeCoordinate> = cell_occupancy.iter().collect();

        assert_eq!(
            lattice_coordinates,
            vec![
                LatticeCoordinate { i: 1, j: -1 },
                LatticeCoordinate { i: -2, j: 0 },
                LatticeCoordinate { i: 3, j: 0 },
                LatticeCoordinate { i: 12, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn retighten_shrinks_the_box_to_the_remaining_cells() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (20, 5), (1, 0)]);
        cell_occupancy.remove(LatticeCoordinate { i: 20, j: 5 });
        cell_occupancy.retighten();

        assert_eq!(cell_occupancy.origin(), LatticeCoordinate { i: 0, j: 0 });
        assert_eq!(cell_occupancy.width(), 2);
        assert_eq!(cell_occupancy.height(), 1);
        assert_eq!(cell_occupancy.row_bitmap_bytes(), &[0b0000_0011]);
    }

    #[test]
    fn retighten_gives_equal_values_for_equal_sets() {
        let mut first: CellOccupancy = create_occupancy(&[(0, 0), (1, 0), (0, 1)]);
        let mut second: CellOccupancy = create_occupancy(&[(0, 1), (-7, 9), (1, 0), (0, 0)]);
        second.remove(LatticeCoordinate { i: -7, j: 9 });

        first.retighten();
        second.retighten();

        assert_eq!(first, second);
    }

    #[test]
    fn retighten_of_no_cells_gives_empty() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(4, 4)]);
        cell_occupancy.remove(LatticeCoordinate { i: 4, j: 4 });
        cell_occupancy.retighten();

        assert_eq!(cell_occupancy, CellOccupancy::empty());
        assert!(cell_occupancy.is_empty());
    }

    #[test]
    fn from_tight_bitmap_accepts_its_own_parts() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (9, 1), (4, 2)]);
        cell_occupancy.retighten();

        let restored: Option<CellOccupancy> = CellOccupancy::from_tight_bitmap(
            cell_occupancy.origin(),
            cell_occupancy.width(),
            cell_occupancy.height(),
            cell_occupancy.row_bitmap_bytes().to_vec(),
        );

        assert_eq!(restored, Some(cell_occupancy));
    }

    #[test]
    fn from_tight_bitmap_rejects_wrong_length() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 9, 1, vec![0b1]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 1, 1, vec![1, 0]), None);
    }

    #[test]
    fn from_tight_bitmap_rejects_non_tight_box() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 2, 1, vec![0b01]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 1, 2, vec![0, 1]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 0, 0, Vec::new()), None);
    }

    #[test]
    fn from_tight_bitmap_rejects_bits_past_width() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 2, 1, vec![0b0000_0111]), None);
    }
}

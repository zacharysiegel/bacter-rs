use bitcode::{Decode, Encode};

use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct WorldPointSerialOut {
    pub x: i32,
    pub y: i32,
}

impl From<&WorldPoint> for WorldPointSerialOut {
    fn from(world_point: &WorldPoint) -> WorldPointSerialOut {
        WorldPointSerialOut {
            x: world_point.x,
            y: world_point.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SubpixelPointSerial {
    pub x: i32,
    pub y: i32,
}

impl From<&SubpixelPoint> for SubpixelPointSerial {
    fn from(subpixel_point: &SubpixelPoint) -> SubpixelPointSerial {
        SubpixelPointSerial {
            x: subpixel_point.x,
            y: subpixel_point.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SubpixelVectorSerialOut {
    pub x: i32,
    pub y: i32,
}

impl From<&SubpixelVector> for SubpixelVectorSerialOut {
    fn from(subpixel_vector: &SubpixelVector) -> SubpixelVectorSerialOut {
        SubpixelVectorSerialOut {
            x: subpixel_vector.x,
            y: subpixel_vector.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LatticeCoordinateSerialOut {
    pub i: i32,
    pub j: i32,
}

impl From<&LatticeCoordinate> for LatticeCoordinateSerialOut {
    fn from(lattice_coordinate: &LatticeCoordinate) -> LatticeCoordinateSerialOut {
        LatticeCoordinateSerialOut {
            i: lattice_coordinate.i,
            j: lattice_coordinate.j,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_copies_each_geometry_type_field_for_field() {
        assert_eq!(
            WorldPointSerialOut::from(&WorldPoint { x: -3, y: 9 }),
            WorldPointSerialOut { x: -3, y: 9 },
        );
        assert_eq!(
            SubpixelPointSerial::from(&SubpixelPoint { x: 1024, y: -1 }),
            SubpixelPointSerial { x: 1024, y: -1 },
        );
        assert_eq!(
            SubpixelVectorSerialOut::from(&SubpixelVector { x: -7603, y: 7603 }),
            SubpixelVectorSerialOut { x: -7603, y: 7603 },
        );
        assert_eq!(
            LatticeCoordinateSerialOut::from(&LatticeCoordinate { i: 4, j: -2 }),
            LatticeCoordinateSerialOut { i: 4, j: -2 },
        );
    }
}

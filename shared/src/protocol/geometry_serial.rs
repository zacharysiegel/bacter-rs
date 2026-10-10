use bitcode::{Decode, Encode};

use crate::ability::AimVector;
use crate::error::AppError;
use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
use crate::protocol::protocol_limits;

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

impl TryFrom<WorldPointSerialOut> for WorldPoint {
    type Error = AppError;

    fn try_from(world_point_serial_out: WorldPointSerialOut) -> Result<WorldPoint, AppError> {
        Ok(WorldPoint {
            x: check_coordinate(world_point_serial_out.x, protocol_limits::COORDINATE_LIMIT_PIXELS)?,
            y: check_coordinate(world_point_serial_out.y, protocol_limits::COORDINATE_LIMIT_PIXELS)?,
        })
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

impl TryFrom<SubpixelPointSerial> for SubpixelPoint {
    type Error = AppError;

    fn try_from(subpixel_point_serial: SubpixelPointSerial) -> Result<SubpixelPoint, AppError> {
        Ok(SubpixelPoint {
            x: check_coordinate(subpixel_point_serial.x, protocol_limits::COORDINATE_LIMIT_SUBPIXELS)?,
            y: check_coordinate(subpixel_point_serial.y, protocol_limits::COORDINATE_LIMIT_SUBPIXELS)?,
        })
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

impl TryFrom<SubpixelVectorSerialOut> for SubpixelVector {
    type Error = AppError;

    fn try_from(subpixel_vector_serial_out: SubpixelVectorSerialOut) -> Result<SubpixelVector, AppError> {
        Ok(SubpixelVector {
            x: check_coordinate(
                subpixel_vector_serial_out.x,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?,
            y: check_coordinate(
                subpixel_vector_serial_out.y,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?,
        })
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

impl TryFrom<LatticeCoordinateSerialOut> for LatticeCoordinate {
    type Error = AppError;

    fn try_from(lattice_coordinate_serial_out: LatticeCoordinateSerialOut) -> Result<LatticeCoordinate, AppError> {
        Ok(LatticeCoordinate {
            i: check_coordinate(
                lattice_coordinate_serial_out.i,
                protocol_limits::LATTICE_COORDINATE_LIMIT,
            )?,
            j: check_coordinate(
                lattice_coordinate_serial_out.j,
                protocol_limits::LATTICE_COORDINATE_LIMIT,
            )?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct AimVectorSerial {
    pub x: i16,
    pub y: i16,
}

impl From<&AimVector> for AimVectorSerial {
    fn from(aim: &AimVector) -> AimVectorSerial {
        AimVectorSerial { x: aim.x, y: aim.y }
    }
}

impl From<AimVectorSerial> for AimVector {
    fn from(aim_serial: AimVectorSerial) -> AimVector {
        AimVector {
            x: aim_serial.x,
            y: aim_serial.y,
        }
    }
}

/// Within `limit` of zero, either sign.
pub fn check_coordinate(coordinate: i32, limit: i32) -> Result<i32, AppError> {
    if coordinate.unsigned_abs() > limit.unsigned_abs() {
        return Err(AppError::new(&format!(
            "coordinate {coordinate} is beyond the limit {limit}"
        )));
    }

    Ok(coordinate)
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

    #[test]
    fn try_from_accepts_coordinates_at_the_limits() {
        assert_eq!(
            WorldPoint::try_from(WorldPointSerialOut {
                x: -262_144,
                y: 262_144
            })
            .unwrap(),
            WorldPoint {
                x: -262_144,
                y: 262_144
            },
        );
        assert_eq!(
            SubpixelPoint::try_from(SubpixelPointSerial {
                x: 268_435_456,
                y: -268_435_456,
            })
            .unwrap(),
            SubpixelPoint {
                x: 268_435_456,
                y: -268_435_456,
            },
        );
        assert_eq!(
            LatticeCoordinate::try_from(LatticeCoordinateSerialOut { i: 43_690, j: -43_690 }).unwrap(),
            LatticeCoordinate { i: 43_690, j: -43_690 },
        );
    }

    #[test]
    fn try_from_rejects_coordinates_past_the_limits() {
        assert!(WorldPoint::try_from(WorldPointSerialOut { x: 262_145, y: 0 }).is_err());
        assert!(SubpixelPoint::try_from(SubpixelPointSerial { x: 0, y: i32::MIN }).is_err());
        assert!(SubpixelVector::try_from(SubpixelVectorSerialOut { x: 268_435_457, y: 0 }).is_err());
        assert!(LatticeCoordinate::try_from(LatticeCoordinateSerialOut { i: 0, j: 43_691 }).is_err());
    }

    #[test]
    fn aim_vector_serial_converts_both_ways() {
        let aim: AimVector = AimVector { x: -300, y: 41 };

        assert_eq!(AimVector::from(AimVectorSerial::from(&aim)), aim);
    }
}

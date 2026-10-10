use bitcode::{Decode, Encode};

use crate::ability::{AbilityPressSet, AimVector, Loadout};
use crate::error::AppError;
use crate::game::{InputBundle, MemberEvent, PlayerTickInput, Tick};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, MemberRoleKind, TeamKind};
use crate::protocol::protocol_limits;
use crate::protocol::{
    AimVectorSerial, AppearanceSerial, LoadoutSerial, MemberRoleKindSerialOut, RejectionKind, TeamKindSerial,
    WorldPointSerialOut,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct InputBundleSerialOut {
    pub tick: u32,
    pub member_events: Vec<MemberEventSerialOut>,
    pub player_inputs: Vec<PlayerTickInputSerialOut>,
}

impl From<&InputBundle> for InputBundleSerialOut {
    fn from(bundle: &InputBundle) -> InputBundleSerialOut {
        InputBundleSerialOut {
            tick: bundle.tick.0,
            member_events: bundle.member_events.iter().map(MemberEventSerialOut::from).collect(),
            player_inputs: bundle.player_inputs.iter().map(PlayerTickInputSerialOut::from).collect(),
        }
    }
}

impl TryFrom<InputBundleSerialOut> for InputBundle {
    type Error = AppError;

    fn try_from(bundle_serial_out: InputBundleSerialOut) -> Result<InputBundle, AppError> {
        let player_input_count: u32 = u32::try_from(bundle_serial_out.player_inputs.len())?;

        if player_input_count > protocol_limits::MEMBER_LIMIT_HIGHEST {
            return Err(AppError::new(
                "a bundle has more player inputs than any game has members",
            ));
        }

        let member_events: Vec<MemberEvent> = bundle_serial_out
            .member_events
            .into_iter()
            .map(MemberEvent::try_from)
            .collect::<Result<Vec<MemberEvent>, AppError>>()?;
        let player_inputs: Vec<PlayerTickInput> = bundle_serial_out
            .player_inputs
            .into_iter()
            .map(PlayerTickInput::try_from)
            .collect::<Result<Vec<PlayerTickInput>, AppError>>()?;

        Ok(InputBundle {
            tick: Tick(bundle_serial_out.tick),
            member_events,
            player_inputs,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PlayerTickInputSerialOut {
    pub member_id: u32,
    pub cursor: WorldPointSerialOut,
    pub ability_presses: u8,
    pub aim: Option<AimVectorSerial>,
}

impl From<&PlayerTickInput> for PlayerTickInputSerialOut {
    fn from(player_input: &PlayerTickInput) -> PlayerTickInputSerialOut {
        PlayerTickInputSerialOut {
            member_id: player_input.member_id.0,
            cursor: WorldPointSerialOut::from(&player_input.cursor),
            ability_presses: player_input.ability_presses.bits(),
            aim: player_input.aim.as_ref().map(AimVectorSerial::from),
        }
    }
}

impl TryFrom<PlayerTickInputSerialOut> for PlayerTickInput {
    type Error = AppError;

    fn try_from(player_input_serial_out: PlayerTickInputSerialOut) -> Result<PlayerTickInput, AppError> {
        Ok(PlayerTickInput {
            member_id: MemberId(player_input_serial_out.member_id),
            cursor: WorldPoint::try_from(player_input_serial_out.cursor)?,
            ability_presses: convert_ability_presses(player_input_serial_out.ability_presses)?,
            aim: player_input_serial_out.aim.map(AimVector::from),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MemberEventSerialOut {
    Joined {
        member_id: u32,
        screen_name: String,
        role: MemberRoleKindSerialOut,
        loadout: Option<LoadoutSerial>,
        team: Option<TeamKindSerial>,
    },
    Left {
        member_id: u32,
    },
    SpawnRequested {
        member_id: u32,
        loadout: LoadoutSerial,
        team: Option<TeamKindSerial>,
    },
    AppearanceChanged {
        member_id: u32,
        appearance: AppearanceSerial,
    },
}

impl From<&MemberEvent> for MemberEventSerialOut {
    fn from(member_event: &MemberEvent) -> MemberEventSerialOut {
        match member_event {
            MemberEvent::Joined {
                member_id,
                screen_name,
                role,
                loadout,
                team,
            } => MemberEventSerialOut::Joined {
                member_id: member_id.0,
                screen_name: screen_name.clone(),
                role: MemberRoleKindSerialOut::from(role),
                loadout: loadout.as_ref().map(LoadoutSerial::from),
                team: team.as_ref().map(TeamKindSerial::from),
            },
            MemberEvent::Left { member_id } => MemberEventSerialOut::Left { member_id: member_id.0 },
            MemberEvent::SpawnRequested {
                member_id,
                loadout,
                team,
            } => MemberEventSerialOut::SpawnRequested {
                member_id: member_id.0,
                loadout: LoadoutSerial::from(loadout),
                team: team.as_ref().map(TeamKindSerial::from),
            },
            MemberEvent::AppearanceChanged { member_id, appearance } => MemberEventSerialOut::AppearanceChanged {
                member_id: member_id.0,
                appearance: AppearanceSerial::from(appearance),
            },
        }
    }
}

impl TryFrom<MemberEventSerialOut> for MemberEvent {
    type Error = AppError;

    fn try_from(member_event_serial_out: MemberEventSerialOut) -> Result<MemberEvent, AppError> {
        let member_event: MemberEvent = match member_event_serial_out {
            MemberEventSerialOut::Joined {
                member_id,
                screen_name,
                role,
                loadout,
                team,
            } => {
                protocol_limits::check_screen_name(&screen_name).map_err(RejectionKind::to_app_error)?;

                MemberEvent::Joined {
                    member_id: MemberId(member_id),
                    screen_name,
                    role: MemberRoleKind::from(role),
                    loadout: loadout.map(Loadout::from),
                    team: team.map(TeamKind::from),
                }
            }
            MemberEventSerialOut::Left { member_id } => MemberEvent::Left {
                member_id: MemberId(member_id),
            },
            MemberEventSerialOut::SpawnRequested {
                member_id,
                loadout,
                team,
            } => MemberEvent::SpawnRequested {
                member_id: MemberId(member_id),
                loadout: Loadout::from(loadout),
                team: team.map(TeamKind::from),
            },
            MemberEventSerialOut::AppearanceChanged { member_id, appearance } => MemberEvent::AppearanceChanged {
                member_id: MemberId(member_id),
                appearance: Appearance::from(appearance),
            },
        };

        Ok(member_event)
    }
}

pub fn convert_ability_presses(ability_press_bits: u8) -> Result<AbilityPressSet, AppError> {
    AbilityPressSet::from_bits(ability_press_bits).ok_or_else(|| AppError::new("unknown ability press bits"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::member::{OrganismColorKind, SkinKind};

    fn create_bundle() -> InputBundle {
        let loadout: Loadout = test_fixture::create_loadout();

        InputBundle {
            tick: Tick(12),
            member_events: vec![
                MemberEvent::Joined {
                    member_id: MemberId(3),
                    screen_name: String::from("Blob"),
                    role: MemberRoleKind::Participant,
                    loadout: Some(loadout),
                    team: Some(TeamKind::Green),
                },
                MemberEvent::SpawnRequested {
                    member_id: MemberId(3),
                    loadout,
                    team: Some(TeamKind::Green),
                },
                MemberEvent::Left { member_id: MemberId(1) },
                MemberEvent::AppearanceChanged {
                    member_id: MemberId(2),
                    appearance: Appearance {
                        color: OrganismColorKind::Royal,
                        skin: SkinKind::Grid,
                    },
                },
            ],
            player_inputs: vec![
                PlayerTickInput {
                    member_id: MemberId(0),
                    cursor: WorldPoint { x: 410, y: -3 },
                    ability_presses: AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
                    aim: Some(AimVector { x: -20, y: 7 }),
                },
                PlayerTickInput {
                    member_id: MemberId(2),
                    cursor: WorldPoint { x: 5, y: 6 },
                    ability_presses: AbilityPressSet::NONE,
                    aim: None,
                },
            ],
        }
    }

    #[test]
    fn from_copies_the_tick_and_the_press_bits() {
        let bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());

        assert_eq!(bundle_serial_out.tick, 12);
        assert_eq!(
            bundle_serial_out.player_inputs[0],
            PlayerTickInputSerialOut {
                member_id: 0,
                cursor: WorldPointSerialOut { x: 410, y: -3 },
                ability_presses: 0b1001,
                aim: Some(AimVectorSerial { x: -20, y: 7 }),
            },
        );
    }

    #[test]
    fn try_from_restores_a_bundle_with_every_member_event() {
        let bundle: InputBundle = create_bundle();

        assert_eq!(
            InputBundle::try_from(InputBundleSerialOut::from(&bundle)).unwrap(),
            bundle
        );
    }

    #[test]
    fn try_from_rejects_unknown_press_bits() {
        let mut bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());
        bundle_serial_out.player_inputs[1].ability_presses = 0b1_0000;

        assert!(InputBundle::try_from(bundle_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_an_invalid_screen_name_of_a_joiner() {
        let mut bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());
        bundle_serial_out.member_events[0] = MemberEventSerialOut::Joined {
            member_id: 3,
            screen_name: String::new(),
            role: MemberRoleKindSerialOut::Spectator,
            loadout: None,
            team: None,
        };

        assert!(InputBundle::try_from(bundle_serial_out).is_err());
    }

    fn create_bundle_serial_out_with_player_input_count(player_input_count: u32) -> InputBundleSerialOut {
        let mut bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());
        let player_input_serial_out: PlayerTickInputSerialOut = bundle_serial_out.player_inputs[1];
        bundle_serial_out.player_inputs = vec![player_input_serial_out; usize::try_from(player_input_count).unwrap()];

        bundle_serial_out
    }

    #[test]
    fn try_from_accepts_player_inputs_up_to_the_member_limit() {
        let bundle_serial_out: InputBundleSerialOut =
            create_bundle_serial_out_with_player_input_count(protocol_limits::MEMBER_LIMIT_HIGHEST);

        assert!(InputBundle::try_from(bundle_serial_out).is_ok());
    }

    #[test]
    fn try_from_rejects_more_player_inputs_than_any_game_has_members() {
        let bundle_serial_out: InputBundleSerialOut =
            create_bundle_serial_out_with_player_input_count(protocol_limits::MEMBER_LIMIT_HIGHEST + 1);

        assert!(InputBundle::try_from(bundle_serial_out).is_err());
    }
}

use bitcode::{Decode, Encode};

use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::error::AppError;
use crate::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind, TeamKind};
use crate::organism::Organism;
use crate::protocol::protocol_limits;
use crate::protocol::{OrganismSerialOut, RejectionKind};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct MemberSerialOut {
    pub member_id: u32,
    pub screen_name: String,
    pub role: MemberRoleKindSerialOut,
    pub loadout: Option<LoadoutSerial>,
    pub team: Option<TeamKindSerial>,
    pub score: ScoreSerialOut,
    pub organism: Option<OrganismSerialOut>,
}

impl From<&Member> for MemberSerialOut {
    fn from(member: &Member) -> MemberSerialOut {
        MemberSerialOut {
            member_id: member.member_id.0,
            screen_name: member.screen_name.clone(),
            role: MemberRoleKindSerialOut::from(&member.role),
            loadout: member.loadout.as_ref().map(LoadoutSerial::from),
            team: member.team.as_ref().map(TeamKindSerial::from),
            score: ScoreSerialOut::from(&member.score),
            organism: member.organism.as_ref().map(OrganismSerialOut::from),
        }
    }
}

impl TryFrom<MemberSerialOut> for Member {
    type Error = AppError;

    fn try_from(member_serial_out: MemberSerialOut) -> Result<Member, AppError> {
        protocol_limits::check_screen_name(&member_serial_out.screen_name).map_err(RejectionKind::to_app_error)?;

        let role: MemberRoleKind = MemberRoleKind::from(member_serial_out.role);
        let is_participant: bool = role == MemberRoleKind::Participant;

        if member_serial_out.loadout.is_some() != is_participant {
            return Err(AppError::new("a member has a loadout exactly when it is a Participant"));
        }

        if member_serial_out.organism.is_some() && !is_participant {
            return Err(AppError::new("only a Participant has an organism"));
        }

        let organism: Option<Organism> = member_serial_out.organism.map(Organism::try_from).transpose()?;

        Ok(Member {
            member_id: MemberId(member_serial_out.member_id),
            screen_name: member_serial_out.screen_name,
            role,
            loadout: member_serial_out.loadout.map(Loadout::from),
            team: member_serial_out.team.map(TeamKind::from),
            score: Score::from(member_serial_out.score),
            organism,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MemberRoleKindSerialOut {
    Participant,
    Spectator,
}

impl From<&MemberRoleKind> for MemberRoleKindSerialOut {
    fn from(role: &MemberRoleKind) -> MemberRoleKindSerialOut {
        match role {
            MemberRoleKind::Participant => MemberRoleKindSerialOut::Participant,
            MemberRoleKind::Spectator => MemberRoleKindSerialOut::Spectator,
        }
    }
}

impl From<MemberRoleKindSerialOut> for MemberRoleKind {
    fn from(role_serial_out: MemberRoleKindSerialOut) -> MemberRoleKind {
        match role_serial_out {
            MemberRoleKindSerialOut::Participant => MemberRoleKind::Participant,
            MemberRoleKindSerialOut::Spectator => MemberRoleKind::Spectator,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ScoreSerialOut {
    pub kills: u32,
    pub deaths: u32,
    pub wins: u32,
}

impl From<&Score> for ScoreSerialOut {
    fn from(score: &Score) -> ScoreSerialOut {
        ScoreSerialOut {
            kills: score.kills,
            deaths: score.deaths,
            wins: score.wins,
        }
    }
}

impl From<ScoreSerialOut> for Score {
    fn from(score_serial_out: ScoreSerialOut) -> Score {
        Score {
            kills: score_serial_out.kills,
            deaths: score_serial_out.deaths,
            wins: score_serial_out.wins,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LoadoutSerial {
    pub appearance: AppearanceSerial,
    pub first: FirstAbilityKindSerial,
    pub second: SecondAbilityKindSerial,
    pub third: ThirdAbilityKindSerial,
}

impl From<&Loadout> for LoadoutSerial {
    fn from(loadout: &Loadout) -> LoadoutSerial {
        LoadoutSerial {
            appearance: AppearanceSerial::from(&loadout.appearance),
            first: FirstAbilityKindSerial::from(&loadout.first),
            second: SecondAbilityKindSerial::from(&loadout.second),
            third: ThirdAbilityKindSerial::from(&loadout.third),
        }
    }
}

impl From<LoadoutSerial> for Loadout {
    fn from(loadout_serial: LoadoutSerial) -> Loadout {
        Loadout {
            appearance: Appearance::from(loadout_serial.appearance),
            first: FirstAbilityKind::from(loadout_serial.first),
            second: SecondAbilityKind::from(loadout_serial.second),
            third: ThirdAbilityKind::from(loadout_serial.third),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum FirstAbilityKindSerial {
    Extend,
    Compress,
}

impl From<&FirstAbilityKind> for FirstAbilityKindSerial {
    fn from(first: &FirstAbilityKind) -> FirstAbilityKindSerial {
        match first {
            FirstAbilityKind::Extend => FirstAbilityKindSerial::Extend,
            FirstAbilityKind::Compress => FirstAbilityKindSerial::Compress,
        }
    }
}

impl From<FirstAbilityKindSerial> for FirstAbilityKind {
    fn from(first_serial: FirstAbilityKindSerial) -> FirstAbilityKind {
        match first_serial {
            FirstAbilityKindSerial::Extend => FirstAbilityKind::Extend,
            FirstAbilityKindSerial::Compress => FirstAbilityKind::Compress,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SecondAbilityKindSerial {
    Immortality,
    Freeze,
}

impl From<&SecondAbilityKind> for SecondAbilityKindSerial {
    fn from(second: &SecondAbilityKind) -> SecondAbilityKindSerial {
        match second {
            SecondAbilityKind::Immortality => SecondAbilityKindSerial::Immortality,
            SecondAbilityKind::Freeze => SecondAbilityKindSerial::Freeze,
        }
    }
}

impl From<SecondAbilityKindSerial> for SecondAbilityKind {
    fn from(second_serial: SecondAbilityKindSerial) -> SecondAbilityKind {
        match second_serial {
            SecondAbilityKindSerial::Immortality => SecondAbilityKind::Immortality,
            SecondAbilityKindSerial::Freeze => SecondAbilityKind::Freeze,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ThirdAbilityKindSerial {
    Neutralize,
    Toxin,
}

impl From<&ThirdAbilityKind> for ThirdAbilityKindSerial {
    fn from(third: &ThirdAbilityKind) -> ThirdAbilityKindSerial {
        match third {
            ThirdAbilityKind::Neutralize => ThirdAbilityKindSerial::Neutralize,
            ThirdAbilityKind::Toxin => ThirdAbilityKindSerial::Toxin,
        }
    }
}

impl From<ThirdAbilityKindSerial> for ThirdAbilityKind {
    fn from(third_serial: ThirdAbilityKindSerial) -> ThirdAbilityKind {
        match third_serial {
            ThirdAbilityKindSerial::Neutralize => ThirdAbilityKind::Neutralize,
            ThirdAbilityKindSerial::Toxin => ThirdAbilityKind::Toxin,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct AppearanceSerial {
    pub color: OrganismColorKindSerial,
    pub skin: SkinKindSerial,
}

impl From<&Appearance> for AppearanceSerial {
    fn from(appearance: &Appearance) -> AppearanceSerial {
        AppearanceSerial {
            color: OrganismColorKindSerial::from(&appearance.color),
            skin: SkinKindSerial::from(&appearance.skin),
        }
    }
}

impl From<AppearanceSerial> for Appearance {
    fn from(appearance_serial: AppearanceSerial) -> Appearance {
        Appearance {
            color: OrganismColorKind::from(appearance_serial.color),
            skin: SkinKind::from(appearance_serial.skin),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum OrganismColorKindSerial {
    Fire,
    Camel,
    Clay,
    Sun,
    Leaf,
    Lime,
    Sky,
    Lake,
    Ocean,
    Royal,
    Petal,
    Hot,
}

impl From<&OrganismColorKind> for OrganismColorKindSerial {
    fn from(color: &OrganismColorKind) -> OrganismColorKindSerial {
        match color {
            OrganismColorKind::Fire => OrganismColorKindSerial::Fire,
            OrganismColorKind::Camel => OrganismColorKindSerial::Camel,
            OrganismColorKind::Clay => OrganismColorKindSerial::Clay,
            OrganismColorKind::Sun => OrganismColorKindSerial::Sun,
            OrganismColorKind::Leaf => OrganismColorKindSerial::Leaf,
            OrganismColorKind::Lime => OrganismColorKindSerial::Lime,
            OrganismColorKind::Sky => OrganismColorKindSerial::Sky,
            OrganismColorKind::Lake => OrganismColorKindSerial::Lake,
            OrganismColorKind::Ocean => OrganismColorKindSerial::Ocean,
            OrganismColorKind::Royal => OrganismColorKindSerial::Royal,
            OrganismColorKind::Petal => OrganismColorKindSerial::Petal,
            OrganismColorKind::Hot => OrganismColorKindSerial::Hot,
        }
    }
}

impl From<OrganismColorKindSerial> for OrganismColorKind {
    fn from(color_serial: OrganismColorKindSerial) -> OrganismColorKind {
        match color_serial {
            OrganismColorKindSerial::Fire => OrganismColorKind::Fire,
            OrganismColorKindSerial::Camel => OrganismColorKind::Camel,
            OrganismColorKindSerial::Clay => OrganismColorKind::Clay,
            OrganismColorKindSerial::Sun => OrganismColorKind::Sun,
            OrganismColorKindSerial::Leaf => OrganismColorKind::Leaf,
            OrganismColorKindSerial::Lime => OrganismColorKind::Lime,
            OrganismColorKindSerial::Sky => OrganismColorKind::Sky,
            OrganismColorKindSerial::Lake => OrganismColorKind::Lake,
            OrganismColorKindSerial::Ocean => OrganismColorKind::Ocean,
            OrganismColorKindSerial::Royal => OrganismColorKind::Royal,
            OrganismColorKindSerial::Petal => OrganismColorKind::Petal,
            OrganismColorKindSerial::Hot => OrganismColorKind::Hot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SkinKindSerial {
    Grid,
    Circles,
    Ghost,
    None,
}

impl From<&SkinKind> for SkinKindSerial {
    fn from(skin: &SkinKind) -> SkinKindSerial {
        match skin {
            SkinKind::Grid => SkinKindSerial::Grid,
            SkinKind::Circles => SkinKindSerial::Circles,
            SkinKind::Ghost => SkinKindSerial::Ghost,
            SkinKind::None => SkinKindSerial::None,
        }
    }
}

impl From<SkinKindSerial> for SkinKind {
    fn from(skin_serial: SkinKindSerial) -> SkinKind {
        match skin_serial {
            SkinKindSerial::Grid => SkinKind::Grid,
            SkinKindSerial::Circles => SkinKind::Circles,
            SkinKindSerial::Ghost => SkinKind::Ghost,
            SkinKindSerial::None => SkinKind::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum TeamKindSerial {
    Red,
    Blue,
    Green,
    Pink,
}

impl From<&TeamKind> for TeamKindSerial {
    fn from(team: &TeamKind) -> TeamKindSerial {
        match team {
            TeamKind::Red => TeamKindSerial::Red,
            TeamKind::Blue => TeamKindSerial::Blue,
            TeamKind::Green => TeamKindSerial::Green,
            TeamKind::Pink => TeamKindSerial::Pink,
        }
    }
}

impl From<TeamKindSerial> for TeamKind {
    fn from(team_serial: TeamKindSerial) -> TeamKind {
        match team_serial {
            TeamKindSerial::Red => TeamKind::Red,
            TeamKindSerial::Blue => TeamKind::Blue,
            TeamKindSerial::Green => TeamKind::Green,
            TeamKindSerial::Pink => TeamKind::Pink,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;

    #[test]
    fn from_copies_a_member_with_its_loadout_and_team() {
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(7), WorldPoint { x: 1, y: 2 });
        member.team = Some(TeamKind::Pink);
        member.score = Score {
            kills: 3,
            deaths: 1,
            wins: 2,
        };

        let member_serial_out: MemberSerialOut = MemberSerialOut::from(&member);

        assert_eq!(member_serial_out.member_id, 7);
        assert_eq!(member_serial_out.screen_name, "player 7");
        assert_eq!(member_serial_out.role, MemberRoleKindSerialOut::Participant);
        assert_eq!(
            member_serial_out.loadout,
            Some(LoadoutSerial {
                appearance: AppearanceSerial {
                    color: OrganismColorKindSerial::Ocean,
                    skin: SkinKindSerial::Circles,
                },
                first: FirstAbilityKindSerial::Extend,
                second: SecondAbilityKindSerial::Immortality,
                third: ThirdAbilityKindSerial::Neutralize,
            }),
        );
        assert_eq!(member_serial_out.team, Some(TeamKindSerial::Pink));
        assert_eq!(
            member_serial_out.score,
            ScoreSerialOut {
                kills: 3,
                deaths: 1,
                wins: 2,
            },
        );
        assert!(member_serial_out.organism.is_some());
    }

    #[test]
    fn try_from_restores_a_member_with_its_organism() {
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(7), WorldPoint { x: 1, y: 2 });
        member.team = Some(TeamKind::Green);
        member.score = Score {
            kills: 4,
            deaths: 5,
            wins: 6,
        };

        assert_eq!(Member::try_from(MemberSerialOut::from(&member)).unwrap(), member);
    }

    #[test]
    fn try_from_rejects_a_loadout_which_does_not_match_the_role() {
        let mut spectator: Member = test_fixture::create_participant(MemberId(1));
        spectator.role = MemberRoleKind::Spectator;
        let mut participant: Member = test_fixture::create_participant(MemberId(2));
        participant.loadout = None;

        assert!(Member::try_from(MemberSerialOut::from(&spectator)).is_err());
        assert!(Member::try_from(MemberSerialOut::from(&participant)).is_err());
    }

    #[test]
    fn try_from_rejects_an_organism_for_a_spectator() {
        let mut spectator: Member =
            test_fixture::create_participant_with_organism(MemberId(1), WorldPoint { x: 1, y: 2 });
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;

        assert!(Member::try_from(MemberSerialOut::from(&spectator)).is_err());
    }

    #[test]
    fn try_from_rejects_an_invalid_screen_name() {
        let mut member_serial_out: MemberSerialOut =
            MemberSerialOut::from(&test_fixture::create_participant(MemberId(1)));
        member_serial_out.screen_name = String::from("tab\there");

        assert!(Member::try_from(member_serial_out).is_err());
    }

    #[test]
    fn appearance_serial_converts_back_to_every_color_and_skin() {
        let colors: [OrganismColorKind; 12] = [
            OrganismColorKind::Fire,
            OrganismColorKind::Camel,
            OrganismColorKind::Clay,
            OrganismColorKind::Sun,
            OrganismColorKind::Leaf,
            OrganismColorKind::Lime,
            OrganismColorKind::Sky,
            OrganismColorKind::Lake,
            OrganismColorKind::Ocean,
            OrganismColorKind::Royal,
            OrganismColorKind::Petal,
            OrganismColorKind::Hot,
        ];
        let skins: [SkinKind; 4] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost, SkinKind::None];

        for color in colors {
            for skin in skins {
                let appearance: Appearance = Appearance { color, skin };

                assert_eq!(Appearance::from(AppearanceSerial::from(&appearance)), appearance);
            }
        }
    }

    #[test]
    fn loadout_serial_converts_back_to_every_ability_choice() {
        for first in [FirstAbilityKind::Extend, FirstAbilityKind::Compress] {
            for second in [SecondAbilityKind::Immortality, SecondAbilityKind::Freeze] {
                for third in [ThirdAbilityKind::Neutralize, ThirdAbilityKind::Toxin] {
                    let loadout: Loadout = Loadout {
                        appearance: test_fixture::create_loadout().appearance,
                        first,
                        second,
                        third,
                    };

                    assert_eq!(Loadout::from(LoadoutSerial::from(&loadout)), loadout);
                }
            }
        }
    }
}

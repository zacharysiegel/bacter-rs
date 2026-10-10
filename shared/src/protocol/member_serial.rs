use bitcode::{Decode, Encode};

use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::member::{Appearance, Member, MemberRoleKind, OrganismColorKind, Score, SkinKind, TeamKind};
use crate::protocol::OrganismSerialOut;

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
    use crate::member::MemberId;

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
}

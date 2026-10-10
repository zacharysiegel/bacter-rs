use crate::ability::Loadout;
use crate::member::Score;
use crate::organism::Organism;

pub const TEAM_ORDER: [TeamKind; 4] = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink];
const RED_TEAM_NAME: &str = "red";
const BLUE_TEAM_NAME: &str = "blue";
const GREEN_TEAM_NAME: &str = "green";
const PINK_TEAM_NAME: &str = "pink";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub member_id: MemberId,
    pub screen_name: String,
    pub role: MemberRoleKind,
    /// Some for a Participant.
    pub loadout: Option<Loadout>,
    /// Some only in skirmish.
    pub team: Option<TeamKind>,
    pub score: Score,
    /// None when dead, awaiting spawn, or spectating.
    pub organism: Option<Organism>,
}

/// Never reused within a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemberId(pub u32);

impl MemberId {
    pub fn next(self) -> MemberId {
        MemberId(self.0.saturating_add(1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberRoleKind {
    Participant,
    Spectator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub color: OrganismColorKind,
    pub skin: SkinKind,
}

impl Appearance {
    /// A team member's colour is always its team's colour.
    pub fn with_team_color(self, team: Option<TeamKind>) -> Appearance {
        let Some(team) = team else {
            return self;
        };

        Appearance {
            color: team.forced_color(),
            skin: self.skin,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganismColorKind {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinKind {
    Grid,
    Circles,
    Ghost,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TeamKind {
    Red,
    Blue,
    Green,
    Pink,
}

impl TeamKind {
    pub fn forced_color(self) -> OrganismColorKind {
        match self {
            TeamKind::Red => OrganismColorKind::Fire,
            TeamKind::Blue => OrganismColorKind::Sky,
            TeamKind::Green => OrganismColorKind::Lime,
            TeamKind::Pink => OrganismColorKind::Petal,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TeamKind::Red => RED_TEAM_NAME,
            TeamKind::Blue => BLUE_TEAM_NAME,
            TeamKind::Green => GREEN_TEAM_NAME,
            TeamKind::Pink => PINK_TEAM_NAME,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownTeamName {
    pub name: String,
}

impl TryFrom<&str> for TeamKind {
    type Error = UnknownTeamName;

    fn try_from(name: &str) -> Result<TeamKind, UnknownTeamName> {
        match name {
            RED_TEAM_NAME => Ok(TeamKind::Red),
            BLUE_TEAM_NAME => Ok(TeamKind::Blue),
            GREEN_TEAM_NAME => Ok(TeamKind::Green),
            PINK_TEAM_NAME => Ok(TeamKind::Pink),
            _ => Err(UnknownTeamName {
                name: String::from(name),
            }),
        }
    }
}

/// The first `team_count` teams in team order.
pub fn get_game_teams(team_count: u8) -> Vec<TeamKind> {
    TEAM_ORDER.into_iter().take(usize::from(team_count)).collect()
}

/// Members without a team are nobody's teammates.
pub fn is_same_team(first_team: Option<TeamKind>, second_team: Option<TeamKind>) -> bool {
    first_team.is_some() && first_team == second_team
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_team_color_forces_the_team_color_and_keeps_the_skin() {
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Camel,
            skin: SkinKind::Ghost,
        };

        assert_eq!(
            appearance.with_team_color(Some(TeamKind::Blue)),
            Appearance {
                color: OrganismColorKind::Sky,
                skin: SkinKind::Ghost,
            },
        );
        assert_eq!(appearance.with_team_color(None), appearance);
    }

    #[test]
    fn forced_color_maps_each_team() {
        let forced_colors: Vec<OrganismColorKind> = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink]
            .iter()
            .map(|team| team.forced_color())
            .collect();

        assert_eq!(
            forced_colors,
            vec![
                OrganismColorKind::Fire,
                OrganismColorKind::Sky,
                OrganismColorKind::Lime,
                OrganismColorKind::Petal,
            ],
        );
    }

    #[test]
    fn next_saturates_at_the_maximum_id() {
        assert_eq!(MemberId(4).next(), MemberId(5));
        assert_eq!(MemberId(u32::MAX).next(), MemberId(u32::MAX));
    }

    #[test]
    fn is_same_team_requires_one_team() {
        assert!(is_same_team(Some(TeamKind::Green), Some(TeamKind::Green)));
        assert!(!is_same_team(Some(TeamKind::Green), Some(TeamKind::Red)));
        assert!(!is_same_team(None, None));
    }

    #[test]
    fn get_game_teams_takes_the_first_teams_in_order() {
        assert_eq!(get_game_teams(2), vec![TeamKind::Red, TeamKind::Blue]);
        assert_eq!(get_game_teams(4), TEAM_ORDER.to_vec());
    }

    #[test]
    fn as_str_gives_the_lowercase_team_names() {
        let team_names: Vec<&str> = TEAM_ORDER.iter().map(|team| team.as_str()).collect();

        assert_eq!(team_names, vec!["red", "blue", "green", "pink"]);
    }

    #[test]
    fn try_from_reads_every_team_name() {
        for team in TEAM_ORDER {
            assert_eq!(TeamKind::try_from(team.as_str()), Ok(team));
        }
    }

    #[test]
    fn try_from_rejects_an_unknown_team_name() {
        assert_eq!(
            TeamKind::try_from("Red"),
            Err(UnknownTeamName {
                name: String::from("Red"),
            }),
        );
    }
}

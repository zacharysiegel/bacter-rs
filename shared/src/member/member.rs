use crate::ability::Loadout;
use crate::member::Score;
use crate::organism::Organism;

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
}

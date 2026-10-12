use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::{GameModeKind, GameState};
use shared::member::{Member, MemberId, TeamKind};

use crate::menu::TeamSize;
use crate::menu::loadout_fields;

/// The own member's part of a game, as the in-game menus show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameMenuView {
    pub mode: GameModeKind,
    /// `None` for a Spectator.
    pub own_loadout: Option<Loadout>,
    pub own_team: Option<TeamKind>,
    pub team_sizes: Vec<TeamSize>,
}

impl GameMenuView {
    /// `None` until the own member's `Joined` is applied.
    pub fn from_state(state: &GameState, own_member_id: MemberId) -> Option<GameMenuView> {
        let own_member: &Member = state.members.get(&own_member_id)?;

        Some(GameMenuView {
            mode: state.settings.mode,
            own_loadout: own_member.loadout,
            own_team: own_member.team,
            team_sizes: loadout_fields::get_state_team_sizes(state),
        })
    }
}

pub fn get_game_menu_mode(game_menu: RwSignal<Option<GameMenuView>>) -> Signal<GameModeKind> {
    Signal::derive(move || {
        game_menu
            .with(|game_menu| game_menu.as_ref().map_or(GameModeKind::FreeForAll, |game_menu_view| game_menu_view.mode))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::test_fixture;
    use shared::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(1);

    #[test]
    fn from_state_is_none_before_the_own_member_joins() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(GameMenuView::from_state(&state, OWN_MEMBER_ID), None);
    }

    #[test]
    fn from_state_takes_the_own_loadout_team_and_the_team_sizes() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        let mut own_member: Member = test_fixture::create_participant(OWN_MEMBER_ID);
        own_member.team = Some(TeamKind::Blue);
        test_fixture::insert_member(&mut state, own_member);

        let game_menu_view: GameMenuView = GameMenuView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(game_menu_view.mode, GameModeKind::Skirmish);
        assert_eq!(game_menu_view.own_loadout, Some(test_fixture::create_loadout()));
        assert_eq!(game_menu_view.own_team, Some(TeamKind::Blue));
        assert_eq!(
            game_menu_view.team_sizes,
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 0,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 1,
                },
            ],
        );
    }
}

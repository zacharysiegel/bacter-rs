use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::GameModeKind;
use shared::member::Appearance;
use shared::protocol::{AppearanceSerial, MessageSerialIn};

use crate::app::{AppSignals, ClientSettings};
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::game_menu_view;
use crate::menu::loadout_fields;
use crate::menu::{
    AutoAssignKind, LoadoutFieldValues, LoadoutFields, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow,
    RadioSelectionKind,
};

const PAUSE_HEADER: &str = "Pause Options";

#[derive(Clone, Copy, Debug)]
struct ClientSettingFields {
    name_labels_index: RwSignal<Option<usize>>,
    messages_index: RwSignal<Option<usize>>,
}

impl ClientSettingFields {
    fn new(client_settings: ClientSettings) -> ClientSettingFields {
        ClientSettingFields {
            name_labels_index: RwSignal::new(get_single_radio_index(client_settings.name_labels_shown)),
            messages_index: RwSignal::new(get_single_radio_index(client_settings.messages_shown)),
        }
    }

    fn get_client_settings(&self) -> ClientSettings {
        ClientSettings {
            name_labels_shown: self.name_labels_index.get_untracked().is_some(),
            messages_shown: self.messages_index.get_untracked().is_some(),
        }
    }

    fn create_rows(&self) -> Vec<MenuRow> {
        vec![
            MenuRow::shown(
                MenuFieldKind::NameLabels,
                "Name Labels",
                loadout_fields::create_radios_input(
                    &loadout_fields::SINGLE_RADIO_LABELS,
                    self.name_labels_index,
                    RadioSelectionKind::Deselectable,
                ),
            ),
            MenuRow::shown(
                MenuFieldKind::Messages,
                "Messages",
                loadout_fields::create_radios_input(
                    &loadout_fields::SINGLE_RADIO_LABELS,
                    self.messages_index,
                    RadioSelectionKind::Deselectable,
                ),
            ),
        ]
    }
}

#[component]
pub fn PauseGameMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let own_loadout: Option<Loadout> = signals.game_menu.get_untracked().and_then(|game_menu| game_menu.own_loadout);
    let mode: Signal<GameModeKind> = game_menu_view::get_game_menu_mode(signals.game_menu);
    let initial_values: LoadoutFieldValues = own_loadout.map_or_else(
        || LoadoutFieldValues::join_defaults(&[]),
        |own_loadout| LoadoutFieldValues::from_loadout(&own_loadout, None, AutoAssignKind::Unselected),
    );
    let loadout_fields: LoadoutFields = LoadoutFields::new(&initial_values, Signal::stored(Vec::new()));
    let setting_fields: ClientSettingFields = ClientSettingFields::new(signals.client_settings.get_untracked());
    let mut rows: Vec<MenuRow> = loadout_fields::create_appearance_rows(loadout_fields, mode);

    rows.extend(setting_fields.create_rows());
    rows.push(loadout_fields::create_leave_game_row());

    let on_submit: Callback<()> = Callback::new(move |_| {
        let appearance: Appearance = loadout_fields.get_values().appearance();
        let appearance_update: Option<MessageSerialIn> = get_appearance_update(own_loadout.as_ref(), appearance);

        if let Some(appearance_update) = appearance_update {
            game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(appearance_update));
        }

        apply_client_settings(signals, setting_fields.get_client_settings());
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header=PAUSE_HEADER
            submit_label="Apply"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

#[component]
pub fn PauseSpectateMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let setting_fields: ClientSettingFields = ClientSettingFields::new(signals.client_settings.get_untracked());
    let mut rows: Vec<MenuRow> = setting_fields.create_rows();

    rows.push(loadout_fields::create_leave_game_row());

    let on_submit: Callback<()> =
        Callback::new(move |_| apply_client_settings(signals, setting_fields.get_client_settings()));
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header=PAUSE_HEADER
            submit_label="Apply"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

#[component]
pub fn PauseTutorialMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let rows: Vec<MenuRow> = vec![MenuRow::shown(
        MenuFieldKind::LeaveTutorial,
        "Leave Tutorial",
        MenuInput::Button {
            on_click: Callback::new(move |_| {
                game_canvas::dispatch_driver_command(DriverCommandKind::LeaveTutorial);
            }),
        },
    )];
    let go_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header=PAUSE_HEADER
            submit_label="Back"
            rows=rows
            issues=issues
            on_submit=go_back
            on_back=go_back
        />
    }
}

/// `None` when nothing would change or the member has no loadout.
pub fn get_appearance_update(own_loadout: Option<&Loadout>, appearance: Appearance) -> Option<MessageSerialIn> {
    let own_loadout: &Loadout = own_loadout?;

    if own_loadout.appearance == appearance {
        return None;
    }

    Some(MessageSerialIn::UpdateAppearance {
        appearance: AppearanceSerial::from(&appearance),
    })
}

fn apply_client_settings(signals: AppSignals, client_settings: ClientSettings) {
    game_canvas::dispatch_driver_command(DriverCommandKind::StoreClientSettings(client_settings));
    signals.client_settings.set(client_settings);
    signals.go_back();
}

fn get_single_radio_index(is_selected: bool) -> Option<usize> {
    if is_selected { Some(0) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::test_fixture;
    use shared::member::{OrganismColorKind, SkinKind};

    #[test]
    fn get_appearance_update_sends_a_changed_appearance() {
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Hot,
            skin: SkinKind::Grid,
        };

        assert_eq!(
            get_appearance_update(Some(&test_fixture::create_loadout()), appearance),
            Some(MessageSerialIn::UpdateAppearance {
                appearance: AppearanceSerial::from(&appearance),
            }),
        );
    }

    #[test]
    fn get_appearance_update_sends_nothing_for_an_unchanged_appearance_or_no_loadout() {
        let loadout: Loadout = test_fixture::create_loadout();

        assert_eq!(get_appearance_update(Some(&loadout), loadout.appearance), None);
        assert_eq!(get_appearance_update(None, loadout.appearance), None);
    }
}

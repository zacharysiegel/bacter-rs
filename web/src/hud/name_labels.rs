use leptos::prelude::*;

use shared::game::GameState;
use shared::geometry;
use shared::member::MemberId;
use shared::organism::{LatticeCentroid, Organism};
use shared::play::InterpolatedPoint;
use shared::render::{Camera, ViewPoint};

/// Longer names are cut to `TRUNCATED_NAME_CHARACTER_COUNT` characters and an ellipsis (faithful).
pub const NAME_LABEL_CHARACTER_LIMIT: usize = 30;
pub const TRUNCATED_NAME_CHARACTER_COUNT: usize = 20;
const TRUNCATION_SUFFIX: &str = "...";
/// The original's two cell widths plus 8 px below the organism's radius.
const LABEL_GAP_PIXELS: f32 = 2.0 * geometry::CELL_WIDTH_PIXELS as f32 + 8.0;

#[derive(Clone, Debug, PartialEq)]
pub struct NameLabelView {
    pub member_id: MemberId,
    pub text: String,
    /// The text's baseline centre.
    pub position: ViewPoint,
}

#[component]
pub fn NameLabels(name_labels: Signal<Vec<NameLabelView>>) -> impl IntoView {
    move || {
        name_labels
            .get()
            .into_iter()
            .map(|name_label| {
                let label_style: String =
                    format!("left: {}px; top: {}px", name_label.position.x, name_label.position.y);

                view! { <p class="name-label" style=label_style>{name_label.text}</p> }
            })
            .collect_view()
    }
}

/// Under every organism's centroid, at the radius of a disc of its cell area.
pub fn get_name_labels(state: &GameState, camera: &Camera) -> Vec<NameLabelView> {
    state
        .members
        .values()
        .filter_map(|member| {
            let organism: &Organism = member.organism.as_ref()?;
            let label_point: InterpolatedPoint = get_label_point(organism)?;

            Some(NameLabelView {
                member_id: member.member_id,
                text: get_shown_name(&member.screen_name),
                position: camera.get_view_point(label_point),
            })
        })
        .collect()
}

pub fn get_shown_name(screen_name: &str) -> String {
    if screen_name.chars().count() <= NAME_LABEL_CHARACTER_LIMIT {
        return String::from(screen_name);
    }

    let truncated_name: String = screen_name.chars().take(TRUNCATED_NAME_CHARACTER_COUNT).collect();

    format!("{truncated_name}{TRUNCATION_SUFFIX}")
}

/// `None` for an organism without cells.
fn get_label_point(organism: &Organism) -> Option<InterpolatedPoint> {
    let centroid: LatticeCentroid = organism.centroid();

    if centroid.cell_count == 0 {
        return None;
    }

    let cell_count: f32 = centroid.cell_count as f32;
    let cell_width_pixels: f32 = geometry::CELL_WIDTH_PIXELS as f32;
    let centre_x: f32 = organism.anchor.x as f32 + cell_width_pixels * centroid.sum_i as f32 / cell_count;
    let centre_y: f32 = organism.anchor.y as f32 + cell_width_pixels * centroid.sum_j as f32 / cell_count;
    let disc_radius_pixels: f32 = (cell_width_pixels * cell_width_pixels * cell_count / std::f32::consts::PI).sqrt();

    Some(InterpolatedPoint {
        x: centre_x,
        y: centre_y + disc_radius_pixels + LABEL_GAP_PIXELS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::GameModeKind;
    use shared::game::test_fixture;
    use shared::geometry::{LatticeCoordinate, WorldPoint};
    use shared::member::Member;
    use shared::render::{PhysicalSize, ViewportSize};
    use shared::world::WorldShapeKind;

    const MEMBER_ID: MemberId = MemberId(2);

    fn create_camera() -> Camera {
        Camera {
            view_origin: InterpolatedPoint { x: 50.0, y: 20.0 },
            viewport: ViewportSize {
                physical_size: PhysicalSize {
                    width_pixels: 800,
                    height_pixels: 600,
                },
                device_pixel_ratio: 1.0,
            },
        }
    }

    #[test]
    fn get_name_labels_places_a_label_under_the_centroid_in_view_pixels() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        let mut member: Member =
            test_fixture::create_participant_with_organism(MEMBER_ID, WorldPoint { x: 100, y: 200 });
        member.organism.as_mut().unwrap().cells.insert(LatticeCoordinate { i: 1, j: 0 });
        test_fixture::insert_member(&mut state, member);
        test_fixture::insert_member(&mut state, test_fixture::create_participant(MemberId(3)));

        let name_labels: Vec<NameLabelView> = get_name_labels(&state, &create_camera());
        let disc_radius_pixels: f32 = (36.0 * 2.0 / std::f32::consts::PI).sqrt();

        assert_eq!(
            name_labels,
            vec![NameLabelView {
                member_id: MEMBER_ID,
                text: String::from("player 2"),
                position: ViewPoint {
                    x: 103.0 - 50.0,
                    y: 200.0 + disc_radius_pixels + 20.0 - 20.0,
                },
            }],
        );
    }

    #[test]
    fn get_shown_name_cuts_names_over_thirty_characters_to_twenty_and_an_ellipsis() {
        let thirty_character_name: String = "a".repeat(30);
        let long_name: String = format!("{}{}", "b".repeat(20), "c".repeat(11));

        assert_eq!(get_shown_name(&thirty_character_name), thirty_character_name);
        assert_eq!(get_shown_name(&long_name), format!("{}...", "b".repeat(20)));
    }
}

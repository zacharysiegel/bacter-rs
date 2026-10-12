use leptos::prelude::*;

use shared::ability::ability_icon;
use shared::ability::{AbilityIconKind, IconPixel};

/// A pixel point of the original's pen covers the square around it.
const PIXEL_HALF_WIDTH: f32 = 0.5;

/// The icon's pixels in black, centred on its slot.
#[component]
pub fn AbilityIconView(icon: AbilityIconKind) -> impl IntoView {
    view! {
        <svg class="ability-icon" viewBox="-30 -30 60 60" width="60" height="60" shape-rendering="crispEdges">
            <path d=get_icon_path_data(icon)></path>
        </svg>
    }
}

/// One unit square per pixel, in drawing order.
pub fn get_icon_path_data(icon: AbilityIconKind) -> String {
    ability_icon::get_icon_pixels(icon)
        .into_iter()
        .map(|icon_pixel: IconPixel| {
            let left: f32 = icon_pixel.x as f32 - PIXEL_HALF_WIDTH;
            let top: f32 = icon_pixel.y as f32 - PIXEL_HALF_WIDTH;

            format!("M{left} {top}h1v1h-1z")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_icon_path_data_draws_a_unit_square_for_every_pixel_of_the_pen() {
        let path_data: String = get_icon_path_data(AbilityIconKind::Spore);
        let pixel_count: usize = ability_icon::get_icon_pixels(AbilityIconKind::Spore).len();

        assert!(path_data.starts_with("M-1.5 -1.5h1v1h-1z"));
        assert_eq!(path_data.matches('M').count(), pixel_count);
    }
}

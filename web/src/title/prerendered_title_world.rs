use leptos::prelude::*;

use shared::play::InterpolatedPoint;
use shared::play::local_game;
use shared::render::RenderColor;
use shared::render::render_color;
use shared::render::scene;

use crate::app::{AppSignals, FirstFrameKind};

/// The title scene's world as the renderer draws it, shown until the canvas has a frame.
#[component]
pub fn PrerenderedTitleWorld() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let is_hidden = move || signals.first_frame.get() == FirstFrameKind::Drawn;

    view! {
        <div
            class="prerendered-title-world"
            class:prerendered-title-world-hidden=is_hidden
            style=get_prerendered_title_world_style()
        ></div>
    }
}

/// The renderer's values, as custom properties the stylesheet reads.
pub fn get_prerendered_title_world_style() -> String {
    let shadow_offset: InterpolatedPoint = scene::RECTANGLE_WORLD_SHADOW_OFFSET;

    format!(
        "--title-world-inset: {}px; --title-world-border-width: {}px; --title-world-body-color: {}; --title-world-border-color: {}; --title-world-shadow-color: {}; --title-world-shadow-offset-x: {}px; --title-world-shadow-offset-y: {}px",
        local_game::LOCAL_WORLD_MARGIN_PIXELS,
        scene::WORLD_BORDER_WIDTH_PIXELS,
        get_css_color(render_color::WORLD_BODY_COLOR),
        get_css_color(render_color::WORLD_BORDER_COLOR),
        get_css_color(render_color::SHADOW_COLOR),
        shadow_offset.x,
        shadow_offset.y,
    )
}

fn get_css_color(color: RenderColor) -> String {
    format!("rgb({}, {}, {})", color.red, color.green, color.blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_prerendered_title_world_style_gives_the_renderer_title_world() {
        assert_eq!(
            get_prerendered_title_world_style(),
            "--title-world-inset: 25px; --title-world-border-width: 1px; --title-world-body-color: rgb(0, 0, 0); --title-world-border-color: rgb(210, 210, 210); --title-world-shadow-color: rgb(50, 50, 50); --title-world-shadow-offset-x: 7px; --title-world-shadow-offset-y: 6px",
        );
    }
}

use shared::tutorial::DisplayModeKind;

const FULLSCREEN_MAXIMUM_CHROME_HEIGHT_PX: f64 = 1.0;

/// The original's test: a fullscreen window has no browser chrome above the page.
pub fn get_display_mode() -> DisplayModeKind {
    let Some(window) = web_sys::window() else {
        return DisplayModeKind::Windowed;
    };

    let outer_height: f64 = window.outer_height().ok().and_then(|height| height.as_f64()).unwrap_or(0.0);
    let inner_height: f64 = window.inner_height().ok().and_then(|height| height.as_f64()).unwrap_or(0.0);

    if outer_height - inner_height < FULLSCREEN_MAXIMUM_CHROME_HEIGHT_PX {
        DisplayModeKind::Fullscreen
    } else {
        DisplayModeKind::Windowed
    }
}

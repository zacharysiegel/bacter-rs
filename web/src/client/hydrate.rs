use crate::app::App;

/// The entry point cargo-leptos's generated script calls once the page has loaded.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();

    // The wasm32 build caps `log` at info in release, so Debug here leaves release output at info.
    _ = console_log::init_with_level(log::Level::Debug);

    leptos::mount::hydrate_body(App);
}

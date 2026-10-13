use wasm_bindgen::JsCast;
use web_sys::{Event, PageTransitionEvent};

use crate::connection::PageShowEffectKind;

pub fn get_page_show_effect(event: &Event) -> PageShowEffectKind {
    let persisted: bool = event.dyn_ref::<PageTransitionEvent>().is_some_and(PageTransitionEvent::persisted);

    PageShowEffectKind::from_persisted(persisted)
}

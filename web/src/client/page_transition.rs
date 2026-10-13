use wasm_bindgen::JsCast;
use web_sys::{Event, PageTransitionEvent};

use crate::connection::PageShowKind;

pub fn get_page_show(event: &Event) -> PageShowKind {
    let persisted: bool = event.dyn_ref::<PageTransitionEvent>().is_some_and(PageTransitionEvent::persisted);

    PageShowKind::from_persisted(persisted)
}

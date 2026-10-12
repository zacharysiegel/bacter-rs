use leptos::prelude::*;

/// Top left; the box takes the width of its text, as the DOM measures it.
#[component]
pub fn MessageBox(message_text: Signal<Option<String>>) -> impl IntoView {
    move || message_text.get().map(|message_text| view! { <p class="message-box">{message_text}</p> })
}

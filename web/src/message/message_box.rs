use leptos::prelude::*;

#[component]
pub fn MessageBox(message_text: Signal<Option<String>>) -> impl IntoView {
    move || message_text.get().map(|message_text| view! { <p class="message-box">{message_text}</p> })
}

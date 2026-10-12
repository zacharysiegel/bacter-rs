use leptos::prelude::*;

#[cfg(feature = "ssr")] // a browser build renders no document head
use leptos::hydration::{AutoReload, HydrationScripts};
#[cfg(feature = "ssr")] // a browser build renders no document head
use leptos_meta::HashedStylesheet;

#[cfg(feature = "ssr")] // a browser build renders no document
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en-US">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="theme-color" content="#000000" />
                <title>Bacter</title>
                <link rel="icon" href="/favicon.ico" />
                <HashedStylesheet options=options.clone() id="leptos" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! { <div class="screen-layer"></div> }
}

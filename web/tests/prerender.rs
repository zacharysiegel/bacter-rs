// a browser build has no prerender to test
#![cfg(feature = "ssr")]

use axum::body::Bytes;
use leptos::prelude::LeptosOptions;

/// Renders in-process; needs no built site.
async fn get_prerendered_document() -> String {
    let leptos_options: LeptosOptions = web::server::read_leptos_manifest_options().unwrap();
    let document_bytes: Bytes = web::server::prerender_document(leptos_options).await.unwrap();

    String::from_utf8(document_bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn prerender_document_references_the_client_bundle() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains("/pkg/"));
    assert!(document.contains(".wasm"));
    assert!(document.contains(".css"));
}

#[tokio::test]
async fn prerender_document_names_the_game_in_the_tab() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains("<title>Bacter</title>"));
}

#[tokio::test]
async fn prerender_document_opens_on_the_title_screen() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<div class="title-menu">"#));
    assert!(document.contains("Connecting to the game server"));
}

#[tokio::test]
async fn prerender_document_contains_the_canvas_root() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<canvas class="game-canvas""#));
}

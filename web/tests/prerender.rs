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
}

#[tokio::test]
async fn prerender_document_shows_the_loading_status_box() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<div class="title-status"><span class="title-status-text">Loading</span>"#));
    assert!(document.contains(r#"<span class="activity-indicator""#));
    assert!(!document.contains("Connecting to the game server"));
    assert!(!document.contains(r#"class="title-reconnect""#));
}

#[tokio::test]
async fn prerender_document_disables_every_title_menu_button() {
    let document: String = get_prerendered_document().await;

    for button_text in ["Host", "Join", "Tutorial"] {
        let disabled_button: String =
            format!(r#"<div class="title-menu-button title-menu-button-disabled">{button_text}</div>"#);

        assert!(document.contains(&disabled_button));
    }
}

#[tokio::test]
async fn prerender_document_draws_the_title_world() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<div class="prerendered-title-world" style="#));
    assert!(!document.contains("prerendered-title-world-hidden"));
}

#[tokio::test]
async fn prerender_document_preloads_the_title_font() {
    let document: String = get_prerendered_document().await;

    assert!(
        document.contains(r#"<link rel="preload" href="/fonts/bacter.ttf" as="font" type="font/ttf" crossorigin="">"#)
    );
}

#[tokio::test]
async fn prerender_document_contains_the_canvas_root() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<canvas class="game-canvas""#));
}

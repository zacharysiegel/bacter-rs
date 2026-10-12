// view! builds one nested tuple type per level of markup; a large screen nests past rustc's default limit of 128 in release builds.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod hud;
pub mod leaderboard;
pub mod menu;
pub mod message;
pub mod screen;
pub mod title;
pub mod tutorial;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;

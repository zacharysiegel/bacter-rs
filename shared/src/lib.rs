//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod error;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod play;
pub mod protocol;
pub mod random;
// The server builds shared without `render`, so it never links wgpu.
#[cfg(feature = "render")]
pub mod render;
pub mod replay;
pub mod round;
pub mod settings;
pub mod title;
pub mod tutorial;
pub mod world;

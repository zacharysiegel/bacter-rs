pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod membership;
pub mod pending_admissions;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use membership::*;
pub use pending_admissions::*;

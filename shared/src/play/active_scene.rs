/// Whether keyboard input steers the game or a menu over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFocusKind {
    Game,
    Menu,
}

//! The viewport stack shared by the desktop and the phone: what a pick scene answer becomes once a
//! pointer is over it — the hover, the selection a click and its modifiers build, the depth stack a
//! Tab cycles, the focus-channel weights that light it, and what a right-click may do with it.
pub mod menu;
pub mod patterns;
pub mod pins;
pub mod selection;
pub mod stones;

pub use menu::{MenuAction, MenuItem, context_items, heading};
pub use selection::{Mods, Sel, Selection, box_planes, label, tint};

//! Touch input shared by the apps: gesture classification, touch-sized hit areas and the CAD edit funnel's platform-free half.
pub mod funnel;
pub mod gesture;
pub mod hit;
pub mod parts;

pub use funnel::{Prepared, prepare};
pub use gesture::{Contact, Gesture, Phase, Signal, Tracker};
pub use hit::{APERTURE_PT, DepthWalk, FINGER_PT, TARGET_PT, coarse_first, handle_at, menu_pick};

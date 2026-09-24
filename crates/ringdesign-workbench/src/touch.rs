//! Touch input shared by the apps: gesture classification, touch-sized hit areas, box select, Measure, work planes, sketching, primitives dragged out, a part shown alone, where the view looks and the CAD edit funnel's platform-free half.
pub mod boxes;
pub mod funnel;
pub mod gesture;
pub mod hit;
pub mod isolate;
pub mod measure;
pub mod parts;
pub mod planes;
pub mod primitive;
pub mod sketch;
pub mod view;

pub use funnel::{Prepared, prepare};
pub use gesture::{Contact, Gesture, Phase, Signal, Tracker};
pub use hit::{APERTURE_PT, DepthWalk, FINGER_PT, TARGET_PT, coarse_first, handle_at, menu_pick};

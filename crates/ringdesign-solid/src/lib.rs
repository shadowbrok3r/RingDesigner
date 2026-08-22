//! Free-mode solids: mesh CSG on Manifold, behind the `manifold` feature.
//!
//! Without the feature this crate is empty of geometry — the workspace
//! builds without a C++ toolchain, and [`kernel_available`] says so. With
//! it, [`kernel`] wraps Manifold and ports the sibling `mandrel` crate's
//! primitives, frames, tubes and settings, with conversions to and from
//! the core's watertight mesh.

#[cfg(feature = "manifold")]
pub mod kernel;

/// Whether this build carries the Manifold kernel.
pub const fn kernel_available() -> bool {
    cfg!(feature = "manifold")
}

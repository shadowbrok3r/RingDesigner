//! OpenCascade for what the pure-Rust kernel cannot do, behind the `kernel-occt` feature.

/// Whether this build carries the OpenCascade kernel.
pub fn available() -> bool {
    cfg!(feature = "kernel-occt")
}

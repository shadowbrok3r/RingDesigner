//! OpenCascade in a child process behind `kernel-occt`: one JSON request on a worker's stdin, one response of packed meshes on its stdout.
pub mod client;
#[cfg(feature = "kernel-occt")]
pub mod kernel;
pub mod parts;
pub mod protocol;

/// Whether this build carries the OpenCascade kernel.
pub fn available() -> bool {
    cfg!(feature = "kernel-occt")
}

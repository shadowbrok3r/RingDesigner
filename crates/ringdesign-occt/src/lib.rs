//! OpenCascade in a child process: one JSON request on a worker's stdin, one response of packed meshes on its stdout.
//! Only the worker and its kernel need `kernel-occt`; a host finds a worker at run time, one it carries included.
pub mod client;
pub mod embedded;
#[cfg(feature = "kernel-occt")]
pub mod kernel;
pub mod parts;
pub mod protocol;

/// Whether this build of the crate carries the OpenCascade kernel; a host asks [`client::probe`] whether it has a worker.
pub fn available() -> bool {
    cfg!(feature = "kernel-occt")
}

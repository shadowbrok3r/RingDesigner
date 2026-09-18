//! The chosen graph node's highlight on the ring. The measuring, the aim and
//! the camera's turn are the workbench's, shared with the desktop; the phone
//! adds only its renderer's vertex order.

pub use ringdesign_workbench::focus::*;

use ringdesign_core::mesh::Mesh;

/// Per-vertex weights in the order the phone's renderer stages a mesh.
pub fn stage(mesh: &Mesh, weights: &[f32]) -> Vec<f32> {
    crate::viewport::GpuMeshRenderer::stage_focus(mesh, weights)
}

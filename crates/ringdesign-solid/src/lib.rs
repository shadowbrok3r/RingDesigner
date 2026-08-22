//! Free-mode solids: mesh CSG on Manifold, behind the `manifold` feature.
//!
//! Without the feature this crate is empty of geometry — the workspace
//! builds without a C++ toolchain, and [`kernel_available`] says so. With
//! it, [`kernel`] wraps Manifold and ports the sibling `mandrel` crate's
//! primitives, frames, tubes and settings, with conversions to and from
//! the core's watertight mesh.

#[cfg(feature = "manifold")]
pub mod kernel;
pub mod io;

use ringdesign_core::mesh::{Mesh, Vec3};

/// Area-weighted vertex normals for a face list.
pub fn vertex_normals(vertices: &[Vec3], faces: &[[u32; 3]]) -> Vec<Vec3> {
    let mut acc = vec![[0.0f32; 3]; vertices.len()];
    for f in faces {
        let [a, b, c] = [vertices[f[0] as usize], vertices[f[1] as usize], vertices[f[2] as usize]];
        let u = [b.0 - a.0, b.1 - a.1, b.2 - a.2];
        let v = [c.0 - a.0, c.1 - a.1, c.2 - a.2];
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        for i in f {
            let s = &mut acc[*i as usize];
            s[0] += n[0];
            s[1] += n[1];
            s[2] += n[2];
        }
    }
    acc.into_iter()
        .map(|n| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-12 { Vec3(n[0] / l, n[1] / l, n[2] / l) } else { Vec3(0.0, 0.0, 1.0) }
        })
        .collect()
}

/// A mesh from vertices and faces, normals computed.
pub fn mesh_from(vertices: Vec<Vec3>, faces: Vec<[u32; 3]>) -> Mesh {
    let normals = vertex_normals(&vertices, &faces);
    Mesh { vertices, normals, faces }
}

/// Whether this build carries the Manifold kernel.
pub const fn kernel_available() -> bool {
    cfg!(feature = "manifold")
}

//! Meshes flattened into the renderer's interleaved buffers, off any GL thread.
use ringdesign_core::castability::{CastReport, FaceClass};
use ringdesign_core::mesh::{Mesh, Vec3};

/// Floats per staged mesh vertex: position (3), normal (3), draft colour (3), wall colour (3).
pub const MESH_FLOATS: usize = 12;

/// Bore and inward faces sit out of the wall heatmap in a neutral grey.
pub const WALL_NEUTRAL: [f32; 3] = [0.42, 0.42, 0.45];

/// Wall-heatmap colour for a radial thickness, linear RGB: red at the minimum fill section and
/// under, amber to twice it, then through green into a quiet blue-grey for comfortably thick metal.
pub fn wall_color(thickness_mm: f64, min_section_mm: f64) -> [f32; 3] {
    let m = min_section_mm.max(0.05);
    let t = (thickness_mm / m).max(0.0);
    let lerp3 = |a: [f32; 3], b: [f32; 3], k: f64| {
        let k = k.clamp(0.0, 1.0) as f32;
        [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
    };
    const RED: [f32; 3] = [0.93, 0.27, 0.36];
    const AMBER: [f32; 3] = [0.95, 0.76, 0.24];
    const GREEN: [f32; 3] = [0.32, 0.78, 0.45];
    const THICK: [f32; 3] = [0.36, 0.55, 0.72];
    if t <= 1.0 {
        RED
    } else if t <= 2.0 {
        lerp3(RED, AMBER, t - 1.0)
    } else if t <= 3.5 {
        lerp3(AMBER, GREEN, (t - 2.0) / 1.5)
    } else {
        lerp3(GREEN, THICK, (t - 3.5) / 2.5)
    }
}

/// Whether every corner of a face is a finite vertex of the mesh.
fn whole(mesh: &Mesh, face: &[u32; 3]) -> bool {
    face.iter().all(|&vi| mesh.vertices.get(vi as usize).is_some_and(|p| p.is_finite()))
}

/// The mesh as non-indexed triangles with the draft colours of `cast` and the wall heatmap of
/// `wall = (inner_radius_mm, min_section_mm)` baked in, so switching shade modes never re-uploads.
pub fn stage_mesh(mesh: &Mesh, cast: Option<&CastReport>, wall: (f64, f64)) -> Vec<f32> {
    let (inner_r, min_section) = wall;
    let mut data: Vec<f32> = Vec::with_capacity(mesh.faces.len() * 3 * MESH_FLOATS);
    let mut hard = 0;
    for (i, face) in mesh.faces.iter().enumerate() {
        if !whole(mesh, face) {
            continue;
        }
        // A solid's faces keep their creases; the band shades from its own vertex normals.
        let corners = mesh.face_normals(i, &mut hard);
        let rgb = cast.map_or([1.0; 3], |c| c.classes.get(i).map_or([1.0; 3], |k| k.rgb()));
        for (k, &vi) in face.iter().enumerate() {
            let p = mesh.vertices[vi as usize];
            let n = corners[k];
            // Radial metal under this vertex; the bore itself faces inward and is no wall.
            let r = (p.0 as f64).hypot(p.1 as f64);
            let inward = (n.0 as f64 * p.0 as f64 + n.1 as f64 * p.1 as f64) < 0.0;
            let w = if inward { WALL_NEUTRAL } else { wall_color(r - inner_r, min_section) };
            data.extend_from_slice(&[p.0, p.1, p.2, n.0, n.1, n.2, rgb[0], rgb[1], rgb[2], w[0], w[1], w[2]]);
        }
    }
    data
}

/// A CAD-only ring: a vertex keeps its smooth normal within 20° of its facet's and takes the facet's past it.
pub fn stage_cad(mesh: &Mesh) -> Vec<f32> {
    let mut display = Mesh::default();
    for face in &mesh.faces {
        let Some(normal) = mesh.face_normal(face) else { continue };
        let first = display.vertices.len() as u32;
        let facet = Vec3(normal[0] as f32, normal[1] as f32, normal[2] as f32);
        let mut kept = 0;
        for &id in face {
            let Some(p) = mesh.vertices.get(id as usize) else { continue };
            let smooth = mesh.normals.get(id as usize).copied().unwrap_or(facet);
            let dot = smooth.0 as f64 * normal[0] + smooth.1 as f64 * normal[1] + smooth.2 as f64 * normal[2];
            display.vertices.push(*p);
            display.normals.push(if dot > 0.94 { smooth } else { facet });
            kept += 1;
        }
        if kept == 3 {
            display.faces.push([first, first + 1, first + 2]);
        }
    }
    stage_mesh(&display, None, (0.0, 0.0))
}

/// A focus channel: `weight[v]` of each vertex in the order [`stage_mesh`] emits them, the same faces skipped.
pub fn stage_focus(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
    let mut data = Vec::with_capacity(mesh.faces.len() * 3);
    for face in mesh.faces.iter().filter(|f| whole(mesh, f)) {
        data.extend(face.iter().map(|&vi| weight.get(vi as usize).copied().unwrap_or(0.0)));
    }
    data
}

/// The select channel: `(chosen, hovered)` a vertex from `viewport::tint`'s `0 / 1 / 2` weights,
/// in [`stage_mesh`]'s order, the same faces skipped.
pub fn stage_select(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
    let mut data = Vec::with_capacity(mesh.faces.len() * 6);
    for face in mesh.faces.iter().filter(|f| whole(mesh, f)) {
        for &vi in face {
            let w = weight.get(vi as usize).copied().unwrap_or(0.0);
            data.push(if (0.5..1.5).contains(&w) { 1.0 } else { 0.0 });
            data.push(if w >= 1.5 { 1.0 } else { 0.0 });
        }
    }
    data
}

/// A part's tessellation, a vertex normal kept only within 20° of its facet's.
pub fn stage_part(mesh: &Mesh) -> Vec<f32> {
    stage_part_colored(mesh, |_| [1.0; 3])
}

/// [`stage_part`] with every face in its draft class's colour.
pub fn stage_part_classes(mesh: &Mesh, classes: &[FaceClass]) -> Vec<f32> {
    stage_part_colored(mesh, |i| classes.get(i).map_or([1.0; 3], |k| k.rgb()))
}

fn stage_part_colored(mesh: &Mesh, color: impl Fn(usize) -> [f32; 3]) -> Vec<f32> {
    let mut data = Vec::with_capacity(mesh.faces.len() * 3 * MESH_FLOATS);
    for (i, face) in mesh.faces.iter().enumerate() {
        if !whole(mesh, face) {
            continue;
        }
        let Some(facet) = mesh.face_normal(face) else { continue };
        let facet = Vec3(facet[0] as f32, facet[1] as f32, facet[2] as f32);
        let [r, g, b] = color(i);
        for &vi in face {
            let p = mesh.vertices[vi as usize];
            let n = match mesh.normals.get(vi as usize) {
                Some(n) if n.is_finite() && n.0 * facet.0 + n.1 * facet.1 + n.2 * facet.2 > 0.94 => *n,
                _ => facet,
            };
            data.extend_from_slice(&[p.0, p.1, p.2, n.0, n.1, n.2, r, g, b, 1.0, 1.0, 1.0]);
        }
    }
    data
}

/// A mesh in neutral colours; the pass that draws it supplies its own tint.
pub fn stage_plain(mesh: &Mesh) -> Vec<f32> {
    let mut data = Vec::with_capacity(mesh.faces.len() * 3 * MESH_FLOATS);
    for face in mesh.faces.iter().filter(|f| whole(mesh, f)) {
        for &vi in face {
            let p = mesh.vertices[vi as usize];
            let n = match mesh.normals.get(vi as usize) {
                Some(n) if n.is_finite() => *n,
                _ => Vec3(0.0, 0.0, 1.0),
            };
            data.extend_from_slice(&[p.0, p.1, p.2, n.0, n.1, n.2, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Mesh {
        Mesh {
            vertices: vec![Vec3(0.0, 0.0, 0.0), Vec3(1.0, 0.0, 0.0), Vec3(0.0, 1.0, 0.0), Vec3(f32::NAN, 0.0, 0.0)],
            normals: vec![Vec3(0.0, 0.0, 1.0); 4],
            faces: vec![[0, 1, 2], [0, 1, 3], [2, 1, 0], [0, 9, 9]],
            ..Default::default()
        }
    }

    #[test]
    fn every_buffer_skips_the_same_faces_so_the_channels_ride_vertex_for_vertex() {
        let mesh = triangle();
        let staged = stage_mesh(&mesh, None, (8.5, 0.8));
        assert_eq!(staged.len(), 2 * 3 * MESH_FLOATS, "the NaN corner and the missing ones drop their faces");
        assert_eq!(staged[MESH_FLOATS], 1.0, "the second vertex's x");
        assert_eq!(staged[6], 1.0, "no cast report is white");
        let focus = stage_focus(&mesh, &[0.0, 0.5, 1.0, 9.0]);
        assert_eq!(focus, vec![0.0, 0.5, 1.0, 1.0, 0.5, 0.0]);
        let select = stage_select(&mesh, &[0.0, 1.0, 2.0, 1.0]);
        assert_eq!(select, vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        for buffer in [stage_part(&mesh), stage_plain(&mesh), stage_cad(&mesh)] {
            assert_eq!(buffer.len(), staged.len());
        }
    }

    #[test]
    fn the_wall_heatmap_runs_red_amber_green_to_thick() {
        assert_eq!(wall_color(0.3, 0.8), [0.93, 0.27, 0.36]);
        assert_eq!(wall_color(1.6, 0.8), [0.95, 0.76, 0.24]);
        assert_eq!(wall_color(10.0, 0.8), [0.36, 0.55, 0.72]);
        // The bore faces inward and sits out in grey: a unit-radius ring's inside at x = 1.
        let mesh = Mesh {
            vertices: vec![Vec3(1.0, 0.0, 0.0), Vec3(1.0, 0.1, 0.0), Vec3(1.0, 0.0, 0.1)],
            normals: vec![Vec3(-1.0, 0.0, 0.0); 3],
            faces: vec![[0, 1, 2]],
            ..Default::default()
        };
        let staged = stage_mesh(&mesh, None, (0.5, 0.8));
        assert_eq!(&staged[9..12], &WALL_NEUTRAL);
    }
}

//! A hooked round prickle swept in the parting plane on a symmetric crest, judged by field, parts and ray release as it tilts.
//! cargo run -p ringdesign-core --release --example prickle_probe -- [OUT_DIR]
use anyhow::Result;
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{Attach, Component, ComponentRole, Document, Feature, Operation, Placement, Stage},
    castability, mesh,
    profile::{ShankKey, ShankKind},
    render,
    sketch::{Geometry, Sketch, Workplane},
};

#[path = "common/probe.rs"]
mod probe;

const PREVIEW: BuildParams = BuildParams { theta_steps: 256, profile_steps: 128, min_wall_mm: mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };
const EXPORT: BuildParams = BuildParams { theta_steps: 1024, profile_steps: 320, min_wall_mm: mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };

/// A prickle's centreline in the part's tangent-radial plane: `radial` mm straight out, then an arc of `bend_r` turning `bend_deg` round the ring.
fn path(radial: f64, bend_r: f64, bend_deg: f64) -> Sketch {
    let mut s = Sketch { plane: Workplane { x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0], ..Default::default() }, ..Sketch::default() };
    let a = s.point([0.0, 0.0]);
    let b = s.point([0.0, radial]);
    s.entity(Geometry::Line { a, b });
    let centre = s.point([bend_r, radial]);
    let t = (180.0 - bend_deg).to_radians();
    let start = s.point([bend_r + bend_r * t.cos(), radial + bend_r * t.sin()]);
    s.entity(Geometry::Arc { center: centre, start, end: b });
    s
}

/// The Rubus large prickle: a 1.1 mm round rising 0.5 mm, then 70° round a 1.6 mm bend, tapered to 0.28.
fn prickle() -> Operation {
    Operation::Twist { sketch: Sketch::circle(0.55).into(), path: path(0.5, 1.6, 70.0), degrees: 0.0, end_scale: 0.28 }
}

#[derive(Clone, Copy)]
struct Seat {
    theta: f64,
    spin: f64,
    tilt: f64,
    cant: f64,
    blend: f64,
}

const NOMINAL: Seat = Seat { theta: 90.0, spin: 0.0, tilt: 0.0, cant: 0.0, blend: 0.35 };

fn with_prickle(body: &RingDesign, seat: Seat) -> Result<RingDesign> {
    let mut d = body.clone();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Default::default() } })?;
    let mut c = Component::default();
    c.placement = Placement::Ring { theta_deg: seat.theta, across_mm: 0.0, height_mm: -0.35, spin_deg: seat.spin, tilt_deg: seat.tilt, cant_deg: seat.cant };
    c.attach = Attach::Join;
    c.stage = Stage::Cast;
    c.blend_mm = seat.blend;
    doc.append(Feature { id: 2, name: "Prickle".into(), enabled: true, operation: prickle(), component: c })?;
    d.cad = Some(doc);
    Ok(d)
}

fn rubus() -> RingDesign {
    let mut d = RingDesign::default();
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.crown_mm = 0.6;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    probe::cast_in(&mut d, &probe::sand_setup(0.1));
    d
}

fn knuckled() -> RingDesign {
    let mut d = rubus();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let step = 360.0 / 7.0;
    d.shank.keys = (0..7)
        .flat_map(|k| {
            let node = 90.0 + k as f64 * step;
            [
                ShankKey { theta_deg: node.rem_euclid(360.0), width_scale: 1.05, thickness_scale: 1.15, crown_scale: 1.1 },
                ShankKey { theta_deg: (node + step * 0.5).rem_euclid(360.0), width_scale: 0.97, thickness_scale: 0.93, crown_scale: 0.95 },
            ]
        })
        .collect();
    d
}

/// One row: the build, the band's field, the prickle's own faces, and the pull at the preview and a finer pitch.
fn judge(label: &str, body: &RingDesign, seat: Seat, params: BuildParams, lib: &AlphaLibrary) -> Result<Option<mesh::BuildResult>> {
    let d = with_prickle(body, seat)?;
    let built = match mesh::try_build(&d, lib, params) {
        Ok(b) => b,
        Err(e) => {
            println!("{label:<34} build refused: {e:#}");
            return Ok(None);
        }
    };
    let v = &built.report.validation;
    let f = castability::judged_field_report(&d, lib, &d.draft, 256, 128, Some(&built));
    let part = f.parts.first();
    let (inspection, fine) = probe::pull(&d, lib, params, 0.075)?;
    println!(
        "{label:<34} {} faces{}{} | field {:?} {:.4}% | prickle undercut {:.4} mm² (silhouette {:.4}), worst {:+.1}° | pull {} | at 0.075 {}",
        built.mesh.faces.len(),
        if v.watertight { "" } else { " OPEN" },
        if built.parts.notes.is_empty() { String::new() } else { format!(" notes {:?}", built.parts.notes) },
        f.verdict,
        f.undercut_fraction() * 100.0,
        part.map_or(f64::NAN, |p| p.undercut_area_mm2),
        part.map_or(f64::NAN, |p| p.silhouette_mm2),
        part.map_or(f64::NAN, |p| p.worst_draft_deg),
        probe::release_line(&inspection.release),
        probe::release_line(&fine),
    );
    Ok(Some(built))
}

fn main() -> Result<()> {
    let out = std::env::args().nth(1);
    let lib = AlphaLibrary::builtin();
    let body = rubus();
    println!("== Rubus LowDome 7.0 x 3.4, prickle on the crest at 90°, sunk 0.35 mm");
    for cant in [0.0, 1.0, 2.0, 5.0, 10.0] {
        judge(&format!("hook plane tilted {cant}°"), &body, Seat { cant, ..NOMINAL }, PREVIEW, &lib)?;
    }
    for blend in [0.0, 0.2] {
        judge(&format!("seam bead {blend} mm"), &body, Seat { blend, ..NOMINAL }, PREVIEW, &lib)?;
    }
    for spin in [90.0, 180.0] {
        judge(&format!("spun {spin}°"), &body, Seat { spin, ..NOMINAL }, PREVIEW, &lib)?;
    }
    judge("leaned 20° along the ring", &body, Seat { tilt: 20.0, ..NOMINAL }, PREVIEW, &lib)?;
    let export = judge("at export, 1024 x 320", &body, NOMINAL, EXPORT, &lib)?;
    judge("at export, tilted 2°", &body, Seat { cant: 2.0, ..NOMINAL }, EXPORT, &lib)?;
    println!("== Rubus knuckled, 14 keys, prickle on the node at 90°");
    judge("on the node", &knuckled(), NOMINAL, PREVIEW, &lib)?;
    judge("between nodes, 115.7°", &knuckled(), Seat { theta: 90.0 + 180.0 / 7.0, ..NOMINAL }, PREVIEW, &lib)?;
    println!("== 017 Tonneau sand master, native");
    let stock = probe::stock("017", true, None)?;
    judge("on the table at 90°", &stock, NOMINAL, PREVIEW, &lib)?;
    judge("on the shoulder at 50°", &stock, Seat { theta: 50.0, ..NOMINAL }, PREVIEW, &lib)?;
    judge("on the shank at 0°", &stock, Seat { theta: 0.0, ..NOMINAL }, PREVIEW, &lib)?;
    if let (Some(out), Some(built)) = (out, export) {
        std::fs::create_dir_all(&out)?;
        for (view, yaw, pitch) in [("hero", 0.48, 1.0), ("side", 0.0, 0.05), ("face", 0.0, std::f64::consts::FRAC_PI_2)] {
            render::write_png(std::path::Path::new(&out).join(format!("prickle-{view}.png")), &built.mesh, yaw, pitch, 1000, render::GOLD)?;
        }
    }
    Ok(())
}

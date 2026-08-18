// Collection, volume II: a second sweep of the range — new signet cuts, new
// shank silhouettes, and the four new procedural patterns (Honeycomb,
// Pyramids, Argyle, Lattice) as bands.
//
//   cargo run --release --example collection2 [out_dir]
//
// Builtin patterns only (portable); every ring is held to the field verdict
// before it renders, and saved under <designs>/collection2/.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::field::{Layer, LayerEntry, SignetOutline, SIDE_FACE_MIN_DRAFT_DEG};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ProfileStyle, ShankKind};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{library, render, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
const SILVER: [f32; 3] = [0.79, 0.80, 0.81];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

struct Shots {
    face: bool,
    flip: bool,
    hero_pitch: f64,
}
const HERO: Shots = Shots { face: false, flip: false, hero_pitch: 1.12 };

fn face_of(low: bool) -> Shots {
    Shots { face: true, flip: low, hero_pitch: 1.12 }
}

fn wider_is_low(d: &RingDesign) -> bool {
    let ctx = d.field_context();
    ctx.side_faces_std()
        .and_then(|f| f.wider())
        .map(|(lo, hi)| 0.5 * (lo + hi) < ctx.crest_v_mm)
        .unwrap_or(false)
}

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

/// A cut-dome signet: the face cut from a full crown, smooth filleted corners.
fn cut_dome_signet(outline: SignetOutline, width: f64, thickness: f64) -> RingDesign {
    let mut d = squared(width, thickness);
    d.shank.apply_signet(width);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d.shank.head.dome = 1.0;
    d
}

fn side_pattern(d: &RingDesign, alpha: &str, height: f64, edge: f64, repeat_scale: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    t.rows = 1;
    if !t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG) {
        panic!("{alpha}: no side face");
    }
    t.repeats_around = ((t.repeats_for_square_cells(&ctx) as f64 * repeat_scale).round() as u32).max(1);
    t.edge_mm = edge;
    t.mirror_v = true;
    t
}

fn pattern_band(alpha: &str, height: f64, edge: f64, repeat_scale: f64) -> (RingDesign, bool) {
    let d = squared(6.0, 2.6);
    let low = wider_is_low(&d);
    let mut d = d;
    let t = side_pattern(&d, alpha, height, edge, repeat_scale);
    d.layers.layers.push(LayerEntry::new(alpha, Layer::Tiling(t)));
    (d, low)
}

fn finish(dir: &str, slug: &str, blurb: &str, tint: [f32; 3], shots: Shots, mut d: RingDesign, lib: &mut AlphaLibrary) {
    d.name = slug.split_once('-').map(|(_, r)| r.replace('-', " ")).unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::analyze_field(&d, lib, &d.draft, 256, 128);
    println!("{slug:<24} {blurb}");
    println!(
        "{:<24} {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "", field.verdict.label(), field.undercut_fraction() * 100.0, field.worst_draft_deg, field.thinnest_wall_mm,
    );
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(&d, lib, BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() });
    assert!(out.report.validation.watertight, "{slug} not watertight");
    let pi = std::f64::consts::PI;
    let hero_pitch = if shots.flip { pi - shots.hero_pitch } else { shots.hero_pitch };
    render::write_png(format!("{dir}/{slug}-hero.png"), &out.mesh, 0.55, hero_pitch, 900, tint).unwrap();
    if shots.face {
        let (yaw, pitch) = if shots.flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png(format!("{dir}/{slug}-face.png"), &out.mesh, yaw, pitch, 900, tint).unwrap();
    }
    let designs = library::default_design_dir().join("collection2");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/ring-collection2".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- Signets: the cut-dome family on new outlines. -----------------------
    let d = cut_dome_signet(SignetOutline::Oval, 13.0, 2.2);
    finish(&dir, "01-oval-signet", "east-west oval, cut from a domed head", YELLOW, HERO, d, &mut lib);

    let d = cut_dome_signet(SignetOutline::Cushion, 13.0, 2.2);
    finish(&dir, "02-cushion-signet", "a pillow cushion facet on a domed head", SILVER, HERO, d, &mut lib);

    let d = cut_dome_signet(SignetOutline::Marquise, 14.0, 2.2);
    finish(&dir, "03-marquise-signet", "a pointed navette, cut from the crown", ROSE, HERO, d, &mut lib);

    // Cross has concavities, so it keeps the prism (a dome would leave a locked
    // pocket at the re-entrant corners); symmetric, so it fields clean.
    let mut d = squared(13.0, 2.4);
    d.shank.apply_signet(13.0);
    d.shank.head.outline = SignetOutline::Cross;
    d.shank.head.fit_length_to(13.0);
    finish(&dir, "04-cross-signet", "a cross tablet, prism-built for its re-entrant corners", WHITE, HERO, d, &mut lib);

    // --- Shank silhouettes. --------------------------------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.3;
    d.shank.kind = ShankKind::Wave;
    d.shank.amount = 0.6;
    finish(&dir, "05-wave-shank", "edges sliding along the finger, crest on the parting plane", PLATINUM, HERO, d, &mut lib);

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::KnifeEdge);
    d.profile.width_mm = 4.2;
    d.profile.thickness_mm = 2.5;
    finish(&dir, "06-knife-edge", "a sharp central crest falling to both edges", SILVER, HERO, d, &mut lib);

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.4;
    d.shank.kind = ShankKind::Split;
    d.shank.amount = 1.0;
    finish(&dir, "07-split-shank", "two diverging rails, grooved into the side faces", YELLOW, HERO, d, &mut lib);

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.4;
    d.shank.kind = ShankKind::Pinched;
    d.shank.amount = 0.7;
    finish(&dir, "08-pinched-shank", "a waisted band, narrowest at the base", ROSE, HERO, d, &mut lib);

    // --- The four new patterns as bands. -------------------------------------
    let (d, low) = pattern_band("Honeycomb", 0.3, 0.2, 0.7);
    finish(&dir, "09-honeycomb-band", "the Honeycomb generator on the side faces", YELLOW, face_of(low), d, &mut lib);

    let (d, low) = pattern_band("Pyramids", 0.32, 0.15, 0.9);
    finish(&dir, "10-pyramids-band", "faceted hobnail studs on the side faces", PLATINUM, face_of(low), d, &mut lib);

    let (d, low) = pattern_band("Argyle", 0.3, 0.2, 0.9);
    finish(&dir, "11-argyle-band", "a diamond lattice with dotted cells", ROSE, face_of(low), d, &mut lib);

    let (d, low) = pattern_band("Lattice", 0.32, 0.25, 0.9);
    finish(&dir, "12-lattice-band", "an orthogonal open grille on the side faces", SILVER, face_of(low), d, &mut lib);

    println!("renders in {dir}, designs in {}", library::default_design_dir().join("collection2").display());
}

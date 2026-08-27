// Three sketched rings — a heart signet, a half-eternity, a princess
// solitaire — built here with the same numbers handed to CrossGems through
// its Grasshopper components, so the two engines can be laid side by side
// on the same brief. Each is field-checked, rendered with its stones,
// exported as OBJ for the mesh comparison, and saved so it opens in the app.
//
//   cargo run --release --example sketches [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastProcess, Verdict};
use ringdesign_core::curve::{CurveLayer, WireProfile};
use ringdesign_core::field::{Decal, DecalLayer, Layer, LayerEntry, SeatPadLayer, SeatRunLayer, SeatStyle, SignetOutline, Window};
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKey, ShankKind, SIGNET_MIN_SHANK_FRAC, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::{gems, library, stonemap, stones, CustomOutline, ProfileStyle, RingDesign, RingSize};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];

/// Field-check, render with stones, sheet, map, OBJ, save.
fn finish(dir: &str, slug: &str, blurb: &str, tint: [f32; 3], mut d: RingDesign, lib: &mut AlphaLibrary) {
    d.name = slug.split_once('-').map(|(_, r)| r.replace('-', " ")).unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!("{slug:<22} {blurb}");
    println!(
        "{:<22} {} under {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        d.draft.process.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("{:<22}   note: {n}", "");
    }
    let dfm = ringdesign_core::dfm::findings_in(&d, lib);
    for f in &dfm {
        println!("{:<22}   dfm: {}: {}", "", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("{:<22}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        if let Some(p) = &s.closest {
            println!("{:<22}   closest pair: {} / {} — {:.2} mm at the girdle, {:.2} mm at the culet", "", p.a, p.b, p.gap_mm, p.gap_deep_mm);
        }
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<22}   stone warning: {w}", "");
                }
            }
        }
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(&d, lib, BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() });
    assert!(out.report.validation.watertight, "{slug} not watertight");
    println!("{:<22}   bore {:.2} mm, volume {:.1} mm3", "", out.report.inner_diameter_mm, out.report.volume_mm3);
    for m in out.report.metals.iter().take(2) {
        println!("{:<22}   {}: {:.2} g", "", m.metal, m.grams);
    }
    let stones_mesh = gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, 1.05, 900).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-top.png"), &parts, 0.0, 1.55, 700).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-side.png"), &parts, 1.5708, 0.0, 700).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-pattern.png"), &parts[..1], 0.55, 1.05, 900).unwrap();
    // The comparison mesh: dense enough to measure against, light enough to
    // measure with.
    let probe = mesh::build(&d, lib, BuildParams { theta_steps: 640, profile_steps: 160, ..Default::default() });
    ringdesign_core::stl::write_obj(format!("{dir}/{slug}.obj"), &probe.mesh, &d.name).unwrap();
    let sheet = ringdesign_core::spec::html(&d, &out.report, &field, report.as_ref(), &dfm, "sketches");
    std::fs::write(format!("{dir}/{slug}-sheet.html"), sheet).unwrap();
    if report.is_some() {
        stonemap::write_stone_map_svg(format!("{dir}/{slug}-stones.svg"), &d, report.as_ref()).unwrap();
    }
    let designs = library::default_design_dir().join("sketches");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

/// A tuning knob read from the environment, for probing a design.
fn knob(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// `SKETCHES=A,N` runs only the named designs.
fn want(key: &str) -> bool {
    std::env::var("SKETCHES").map(|v| v.split(',').any(|k| k.trim() == key)).unwrap_or(true)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    if want("A") {
    // --- A. Heart signet: the same numbers as CrossGems' Signet Ring ------
    // (Table Length 12 across the finger, Width 11.4 along the ring, Table
    // Height 3.0 over the bore, sides 4.5 x 1.5, Frontal/Lateral 2), through
    // the same mapping cg_signet.rs uses for a decoded preset.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.width_mm = 12.0;
    d.profile.thickness_mm = 1.75;
    d.profile.crown_mm = 1.49;
    d.profile.edge_round_mm = 0.05;
    d.shank.apply_signet(12.0);
    let shank_frac = (4.5_f64 / 12.0).clamp(SIGNET_MIN_SHANK_FRAC, 1.0);
    d.shank.amount = ((1.0 - shank_frac) / (1.0 - SIGNET_MIN_SHANK_FRAC)).clamp(0.0, 1.0);
    d.shank.head.outline = SignetOutline::Heart;
    d.shank.head.length_mm = 11.4;
    d.shank.head.rise_mm = 3.0 - 1.75;
    d.shank.head.rim_round_mm = 0.3;
    d.shank.head.table_dome_mm = 0.0;
    d.shank.head.loft_frontal_mm = 2.0;
    d.shank.head.loft_lateral_mm = 2.0;
    finish(&dir, "A-heart-signet", "lofted heart signet, 12 x 11.4 plate, flat table", YELLOW, d, &mut lib);
    }

    if want("B") {

    // --- B. Half-eternity: 2 mm rounds over 200 degrees --------------------
    // CrossGems packs them 0.2 mm apart for shared prongs or a channel; in
    // sand the seats are bosses drilled at the bench, the bridge is the
    // 0.3 mm fill floor, and the count follows from that.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 3.8;
    d.profile.thickness_mm = 2.2;
    let ctx = d.field_context();
    let seat = SeatPadLayer { v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.45, crown: 0.9, blend_mm: 0.4, ..Default::default() };
    let mut run = SeatRunLayer { seat, gem: Gem::calibrated(GemCut::Round, 2.0), bridge_mm: 0.3, ..Default::default() };
    run.solve_spacing(&ctx);
    let mut entry = LayerEntry::new("Half eternity", Layer::SeatRun(run));
    entry.window = Window::around(TOP_DEG, 200.0);
    d.layers.layers.push(entry);
    finish(&dir, "B-half-eternity", "2 mm rounds over 200 degrees on a 3.8 x 2.2 band", WHITE, d, &mut lib);
    }

    if want("C") {

    // --- C. Princess solitaire: a 4 mm stone on a 3 x 2 band ---------------
    // A prong head stands above the band and is lost-wax stock: here the
    // band widens to carry a boss with four prong bumps, and the sheet is
    // judged for lost wax, as CrossGems' head is.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 3.0;
    d.profile.thickness_mm = 2.0;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = vec![
        ShankKey { theta_deg: TOP_DEG, width_scale: 2.35, thickness_scale: 1.3, crown_scale: 0.6 },
        ShankKey { theta_deg: TOP_DEG + 55.0, width_scale: 1.35, thickness_scale: 1.1, crown_scale: 1.0 },
        ShankKey { theta_deg: TOP_DEG - 55.0, width_scale: 1.35, thickness_scale: 1.1, crown_scale: 1.0 },
        ShankKey { theta_deg: TOP_DEG + 180.0, width_scale: 1.0, thickness_scale: 1.0, crown_scale: 1.0 },
    ];
    d.draft.process = CastProcess::LostWax;
    let ctx = d.field_context();
    let mut seat = SeatPadLayer { theta_deg: TOP_DEG, v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.9, crown: 1.0, blend_mm: 0.45, prongs: 4, prong_mm: 0.55, ..Default::default() };
    seat.fit_stone(Gem::calibrated(GemCut::Princess, 4.0));
    d.layers.layers.push(LayerEntry::new("Princess", Layer::SeatPad(seat)));
    finish(&dir, "C-princess-solitaire", "4 mm princess in a four-prong boss on a widened 3 x 2 band", WHITE, d, &mut lib);
    }

    if want("N") {

    // --- N. Nocturnal Symmetry, the sand pattern ---------------------------
    // The sketch's elongated hexagon, 22 x 10 with 90-degree points, pulled
    // in to its own millgrain line (1.2 mm inside the rim): 18.6 x 7.6, the
    // long axis along the band like a signet. Flat table, no bezels — the
    // four 2.5 mm amethysts are set at the bench, so the pattern carries a
    // cast dot where each goes and the report carries the stones. The dots
    // ride the crest line: off it, on a zero-draft table, a dot's near flank
    // leans back (measured 14 deg with the face across the band).
    // Then the whole face scaled so its width is the band's own 4.5 mm:
    // 11.0 x 4.5, the dots scaled with it.
    let scale = knob("NOCT_SCALE", 4.5 / (2.0 * (5.0 - 1.2)));
    let hw = (5.0 - 1.2) * scale;
    let ay = (11.0 - 1.2 * std::f64::consts::SQRT_2) * scale;
    let cy = ay - hw;
    let corners = [[-cy, -hw], [-ay, 0.0], [-cy, hw], [cy, hw], [ay, 0.0], [cy, -hw]];
    // Twelve stations an edge: the importer wants a polyline, not six corners.
    let pts: Vec<[f64; 2]> = (0..6)
        .flat_map(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % 6]);
            (0..12).map(move |k| {
                let t = k as f64 / 12.0;
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            })
        })
        .collect();
    let outline = CustomOutline::from_points("Nocturnal bead line", &pts).expect("a hexagon makes an outline");
    let (along, across) = (2.0 * ay, 2.0 * hw);
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.width_mm = across;
    d.profile.thickness_mm = 1.85;
    d.profile.crown_mm = 1.56;
    d.profile.edge_round_mm = 0.05;
    d.shank.apply_signet(across);
    let shank_frac = (4.5 / across).clamp(SIGNET_MIN_SHANK_FRAC, 1.0);
    d.shank.amount = ((1.0 - shank_frac) / (1.0 - SIGNET_MIN_SHANK_FRAC)).clamp(0.0, 1.0);
    let o = d.shank.adopt_outline(outline);
    d.shank.head.outline = o;
    d.shank.head.length_mm = along;
    d.shank.head.rise_mm = 3.0 - 1.85;
    d.shank.head.table_dome_mm = 0.0;
    d.shank.head.dome = 0.0;
    d.shank.head.loft = knob("NOCT_LOFT", 1.0);
    d.shank.head.rim_round_mm = knob("NOCT_RIM", 0.3);
    d.shank.head.loft_frontal_mm = 2.0;
    d.shank.head.loft_lateral_mm = 2.0;
    let ctx = d.field_context();
    let plane_r = d.inner_radius_mm() + d.profile.thickness_mm + d.shank.head.rise_mm;
    for (i, dx) in [-6.6 * scale, -2.2 * scale, 2.2 * scale, 6.6 * scale].iter().enumerate() {
        // A 1 mm dot, 0.2 proud, at the ring angle its station on the table
        // plane projects to. Its skirt is the finest feature and reads
        // 0.38 mm at the chart's metal scale, over the 0.35 mm floor.
        let theta = TOP_DEG + (dx / plane_r).atan().to_degrees();
        let mut seat = SeatPadLayer { theta_deg: theta, v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: knob("NOCT_DOT_H", 0.2), crown: 1.0, blend_mm: 0.45, ..Default::default() };
        seat.fit_stone(Gem::calibrated(GemCut::Round, 2.5));
        seat.diameter_mm = 1.0;
        seat.elong = 1.0;
        d.layers.layers.push(LayerEntry::new(&format!("Amethyst {}", i + 1), Layer::SeatPad(seat)));
    }
    finish(&dir, "N-nocturnal-face", "11.0 x 4.5 hexagon face along the band, flat, four cast dots for 2.5 mm amethysts", WHITE, d, &mut lib);
    }

    if want("P") {
    // --- P. Bolt ring: eight shafts and the collars between them ----------
    // The sketch reads as bone; the brief says bolt, so the section is
    // beveled with flat sides, each shaft waists only a little, and the
    // collars are flat-topped wires run across the whole outer surface —
    // their flanks face round the ring, parallel to the pull, and on the
    // side faces they are relief where relief is always castable.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::Beveled);
    d.profile.width_mm = 4.0;
    d.profile.thickness_mm = 2.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let waist = knob("BOLT_WAIST", 0.85);
    d.shank.keys = (0..16)
        .map(|k| {
            let collar = k % 2 == 0;
            ShankKey {
                theta_deg: TOP_DEG + 22.5 * k as f64,
                width_scale: if collar { 1.0 } else { waist },
                thickness_scale: if collar { 1.0 } else { 0.5 + 0.5 * waist },
                crown_scale: 1.0,
            }
        })
        .collect();
    let ctx = d.field_context();
    let collar = CurveLayer {
        points: vec![[0.5, 0.0], [0.5, ctx.band_v_len_mm]],
        repeats_around: 8,
        closed: false,
        width_mm: knob("BOLT_COLLAR_W", 1.6),
        height_mm: knob("BOLT_COLLAR_H", 0.45),
        profile: WireProfile::Flat,
        taper: 0.0,
        mirror_v: false,
    };
    d.layers.layers.push(LayerEntry::new("Collars", Layer::Curve(collar)));
    finish(&dir, "P-bolt-ring", "eight beveled shafts, flat collars across the band", WHITE, d, &mut lib);
    }

    if want("Q") {
    // --- Q. Cloud ring: five lobes along the crest, swirls on the sides -----
    // The cloud is the band itself: keyframed stations lift the crest and
    // widen the plan at five lobes with dips between, so every section is
    // still one dome and the valleys run along the ring, never across it.
    // The curls are round wires on the side faces, the one place relief
    // cannot undercut.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 3.2;
    d.profile.thickness_mm = 2.0;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let lobe = knob("CLOUD_LOBE", 1.0);
    // Lobes stand three band-thicknesses proud with flatter crowns, so
    // each is a tall flat face with a rounded rim — the puff the sketch
    // draws curls on; the dips between them make five clouds, not a ridge.
    let station = |off: f64, w: f64, t: f64, c: f64| ShankKey { theta_deg: TOP_DEG + off, width_scale: 1.0 + (w - 1.0) * lobe, thickness_scale: 1.0 + (t - 1.0) * lobe, crown_scale: c };
    d.shank.keys = vec![
        station(0.0, 2.4, 2.9, 0.7),
        station(11.0, 1.9, 1.9, 0.8),
        station(-11.0, 1.9, 1.9, 0.8),
        station(22.0, 2.1, 2.4, 0.7),
        station(-22.0, 2.1, 2.4, 0.7),
        station(33.0, 1.4, 1.4, 0.8),
        station(-33.0, 1.4, 1.4, 0.8),
        station(44.0, 1.5, 1.65, 0.75),
        station(-44.0, 1.5, 1.65, 0.75),
        station(60.0, 1.05, 1.05, 1.0),
        station(-60.0, 1.05, 1.05, 1.0),
        station(180.0, 1.0, 1.0, 1.0),
    ];
    let ctx = d.field_context();
    // The curls: one hook of a turn and a tenth per lobe, on both side
    // faces. The chart's v is the section's arc normalized, so a stamp
    // stands taller than wide on a lobe by the lobe's own stretch; each
    // lobe's art is drawn squashed by that factor and comes out round.
    let inner_r = d.inner_radius_mm();
    let crest_r = inner_r + d.profile.thickness_mm;
    // Per lobe: the chart's stretch there, and the low side face's own run
    // in metal — the surface points whose normal leans 80 degrees or more
    // along the pull, walked from the low bore edge — so each hook is
    // centred on its face and sized to it.
    let face_of = |theta: f64| -> (f64, f64, f64) {
        let m = d.modulation_at(theta, inner_r, crest_r);
        let l = d.profile.sample_mod(inner_r, 192, &m);
        let k = l.surface_len_mm / ctx.band_v_len_mm;
        let steep = (80.0f64).to_radians().sin();
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for p in l.pts.iter().filter(|p| p.surface && p.v_mm < 0.5 * l.surface_len_mm) {
            if p.nz.abs() >= steep {
                lo = lo.min(p.v_mm);
                hi = hi.max(p.v_mm);
            } else if hi > lo {
                break;
            }
        }
        if hi <= lo { (k, 0.0, 0.0) } else { (k, lo, hi) }
    };
    let hook = |k: f64| -> String {
        let pts: Vec<String> = (0..=120)
            .map(|i| {
                let t = i as f64 / 120.0;
                let a = t * 1.05 * std::f64::consts::TAU;
                let r = 6.0 + 41.0 * t;
                format!("{:.1} {:.1}", 50.0 + r * a.cos(), 50.0 + r * a.sin() / k)
            })
            .collect();
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><path d="M{}" fill="none" stroke="#000" stroke-width="20" stroke-linecap="round"/></svg>"##,
            pts.join(" L")
        )
    };
    // The low side face's run, said explicitly: `wider()` ties on a
    // symmetric band and may hand back the high face.
    let face = ctx.side_faces_std().and_then(|f| f.wider());
    let vc_low = face.map_or(0.7, |(a, b)| {
        let c = 0.5 * (a + b);
        if c < ctx.crest_v_mm { c } else { ctx.band_v_len_mm - c }
    });
    let v_low = knob("CLOUD_CURL_V", 0.9).max(vc_low * 0.5);
    let faces = [face_of(TOP_DEG), face_of(TOP_DEG + 22.0), face_of(TOP_DEG + 44.0)];
    let mut layers = Vec::new();
    for (n, (name, off)) in [("Cloud curl centre", 0.0), ("Cloud curl mid", 22.0), ("Cloud curl outer", 44.0)].into_iter().enumerate() {
        let (k, lo, hi) = faces[n];
        d.svgs.push(SvgAlpha { name: name.into(), svg: hook(k), invert: false });
        // The hook fills 0.8 of the face's height, in metal; its stamp is
        // that wide, and the stamp's centre is the face's, read back into
        // the chart. Never under 2.2 mm, the size its stroke casts at.
        let size = (0.8 * (hi - lo) * knob("CLOUD_CURL_S", 1.0)).max(2.2);
        let v_face = if hi > lo { 0.5 * (lo + hi) / k } else { v_low };
        let mut decals = Vec::new();
        for sign in if off == 0.0 { vec![1.0] } else { vec![1.0, -1.0] } {
            let theta = TOP_DEG + sign * off;
            decals.push(Decal { theta_deg: theta, v_mm: v_face, size_mm: size, rotation_deg: 0.0, height_mm: 0.25, flip: false });
            decals.push(Decal { theta_deg: theta, v_mm: ctx.band_v_len_mm - v_face, size_mm: size, rotation_deg: 0.0, height_mm: 0.25, flip: true });
        }
        println!("{:<22}   {name}: stretch {k:.2}, face {lo:.2}..{hi:.2} mm of metal, hook {size:.2} mm at chart v {v_face:.2}", "");
        layers.push(DecalLayer { alpha: name.into(), decals, feather_mm: 0.25, invert: false });
    }
    for (n, l) in layers.into_iter().enumerate() {
        d.layers.layers.push(LayerEntry::new(&format!("Curls {}", n + 1), Layer::Decals(l)));
    }
    finish(&dir, "Q-cloud-ring", "five lobes keyframed into the crest, curls on the side faces", WHITE, d, &mut lib);
    }
}

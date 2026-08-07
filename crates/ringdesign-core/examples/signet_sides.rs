// Where on a signet can relief survive a +/-Z pull: table, head flank, or band side?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{
    Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SignetLayer, Uv, Window, wrap_delta,
};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::Flange;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{ProfileStyle, RingDesign};

const N_THETA: usize = 512;
const N_PROF: usize = 192;
const TOL_DEG: f64 = 0.5; // matches castability::VERTICAL_TOL_DEG

fn params() -> BuildParams {
    BuildParams { theta_steps: N_THETA, profile_steps: N_PROF, ..Default::default() }
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    faces: usize,
    undercut: usize,
    worst: f64,
    sum: f64,
}

impl Bucket {
    fn push(&mut self, draft: f64) {
        self.faces += 1;
        self.sum += draft;
        if draft < self.worst {
            self.worst = draft;
        }
        if draft < -TOL_DEG {
            self.undercut += 1;
        }
    }
    fn line(&self, name: &str) -> String {
        if self.faces == 0 {
            return format!("  {name:<26} -");
        }
        format!(
            "  {name:<26} {:>6} faces   {:>6.2}% undercut   mean {:>+6.1} deg   worst {:>+6.2} deg",
            self.faces,
            self.undercut as f64 / self.faces as f64 * 100.0,
            self.sum / self.faces as f64,
            self.worst,
        )
    }
}

/// Split every surface face by where it sits relative to the signet head.
fn regions(d: &RingDesign, lib: &AlphaLibrary, s: &SignetLayer) -> [Bucket; 5] {
    let built = mesh::build(d, lib, params());
    let ctx = d.field_context();
    let parting = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm()).parting_z_mm;
    let base = &built.reference;
    let u0 = ctx.u_of_theta(s.theta_deg);
    let edge = ctx.band_v_len_mm * 0.18;

    let mut out = [Bucket::default(); 5];
    for (fi, f) in built.mesh.faces.iter().enumerate() {
        let cell = fi / 2;
        let p = &base.pts[(cell % N_PROF).min(base.pts.len() - 1)];
        if !p.surface {
            continue;
        }
        let (Some(n), Some((a, b, c))) = (built.mesh.face_normal(f), built.mesh.triangle(f)) else {
            continue;
        };
        let draft = castability::draft_angle(n, (a[2] + b[2] + c[2]) / 3.0, parting);

        let u = (cell / N_PROF) as f64 / N_THETA as f64 * ctx.circumference_mm;
        let v = p.v_mm / base.surface_len_mm.max(1e-9) * ctx.band_v_len_mm;
        let du = wrap_delta(u - u0, ctx.circumference_mm);
        let dv = v - s.v_mm;
        let dist = s.outline_distance(du, dv);
        let on_head = dist < 1.0 + s.shoulder_mm / (du * du + dv * dv).sqrt().max(1e-6);

        let idx = if on_head {
            if dist <= s.top_flat {
                0
            } else if dv.abs() > du.abs() {
                1
            } else {
                2
            }
        } else if v < edge || v > ctx.band_v_len_mm - edge {
            3
        } else {
            4
        };
        out[idx].push(draft);
    }
    out
}

fn verdict(name: &str, d: &RingDesign, lib: &AlphaLibrary) {
    let built = mesh::build(d, lib, params());
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    println!(
        "  {name:<46} {:<18} {:>6.3}% undercut   worst {:>+7.2} deg",
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        rep.worst_draft_deg
    );
}

fn dump(title: &str, r: &[Bucket; 5]) {
    println!("\n{title}");
    println!("{}", r[0].line("head table"));
    println!("{}", r[1].line("head flank (across band)"));
    println!("{}", r[2].line("head end (around ring)"));
    println!("{}", r[3].line("band side face"));
    println!("{}", r[4].line("band crest"));
}

/// A signet on a band of the given style, table blank.
fn signet_ring(style: ProfileStyle) -> (RingDesign, SignetLayer) {
    let mut d = RingDesign::default();
    d.profile.apply_style(style);
    let s = SignetLayer::fitted_to(&d.field_context());
    d.layers.layers.push(LayerEntry::new("signet", Layer::Signet(s)));
    (d, s)
}

fn ornament(alpha: &str, d: &RingDesign, v_frac: f64, span_frac: f64, h: f64) -> TilingLayer {
    let c = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &c);
    t.repeats_around = 36;
    t.height_mm = h;
    t.v_center_mm = c.band_v_len_mm * v_frac;
    t.v_span_mm = c.band_v_len_mm * span_frac;
    t.feather_mm = 0.3;
    t
}

fn main() {
    let mut lib = AlphaLibrary::builtin();
    for dir in ringdesign_core::library::alpha_dirs() {
        let _ = lib.load_dir(dir);
    }
    let orn = ["ornament-a-01", "Rope"]
        .into_iter()
        .find(|n| lib.get(n).is_some())
        .unwrap_or("Rope");
    println!("ornament alpha: {orn}   library: {} alphas\n", lib.len());

    // --- 1. Where the draft is, on a blank signet. -------------------------
    let (d, s) = signet_ring(ProfileStyle::HalfRound);
    let c = d.field_context();
    println!("band {:.1} mm wide, v span {:.2} mm, crest at v {:.2} mm", d.profile.width_mm, c.band_v_len_mm, c.crest_v_mm);
    println!("head {:.1} x {:.1} mm, {:.2} mm proud, shoulder {:.2} mm\n", s.length_mm, s.width_mm, s.height_mm, s.shoulder_mm);
    println!("blank signet:");
    verdict("half-round band", &d, &lib);
    dump("draft by region, half-round:", &regions(&d, &lib, &s));

    let (fd, fs) = signet_ring(ProfileStyle::Flat);
    verdict("flat band", &fd, &lib);
    dump("draft by region, flat:", &regions(&fd, &lib, &fs));

    // --- 1b. Base draft across the band, and where relief actually survives.
    for (label, style, flange) in [
        ("Half round", ProfileStyle::HalfRound, false),
        ("Flat", ProfileStyle::Flat, false),
        ("Half round + edge flange", ProfileStyle::HalfRound, true),
    ] {
        let mut r = RingDesign::default();
        r.profile.apply_style(style);
        if flange {
            r.profile.flange = Flange { enabled: true, v_pos: 0.0, extent_mm: 1.1, thickness_mm: 0.9, edge_round_mm: 0.15 };
        }
        let rc = r.field_context();
        println!("\n{label} - base draft across the band, then relief undercut at that v:");
        match rc.side_faces(SIDE_FACE_MIN_DRAFT_DEG) {
            Some(f) => println!(
                "  side faces: {} and {}  ({:.0} deg+ draft)",
                f.low.map_or("-".into(), |(a, b): (f64, f64)| format!("{a:.2}..{b:.2} mm")),
                f.high.map_or("-".into(), |(a, b): (f64, f64)| format!("{a:.2}..{b:.2} mm")),
                SIDE_FACE_MIN_DRAFT_DEG
            ),
            None => println!("  side faces: none at {SIDE_FACE_MIN_DRAFT_DEG:.0} deg - this profile is all dome"),
        }
        println!("  {:<8} {:>11} {:>12} {:>12} {:>12}", "v", "base draft", "h=0.15", "h=0.30", "h=0.50");
        let band = 1.2f64;
        let repeats = (rc.circumference_mm / band).round().max(1.0) as u32;
        for k in 0..=10 {
            let vf = k as f64 / 10.0;
            let v = vf * rc.band_v_len_mm;
            let base_deg = rc.surface.draft_deg(v, rc.band_v_len_mm).unwrap_or(f64::NAN);
            let centre = v.clamp(band * 0.5, rc.band_v_len_mm - band * 0.5);
            let mut row = format!("  {v:<8.2} {base_deg:>9.1} deg");
            for h in [0.15, 0.30, 0.50] {
                let mut rr = r.clone();
                let mut t = ornament(orn, &rr, vf, 0.0, h);
                t.repeats_around = repeats;
                t.v_center_mm = centre;
                t.v_span_mm = band;
                rr.layers.layers.push(LayerEntry::new("orn", Layer::Tiling(t)));
                let built = mesh::build(&rr, &lib, params());
                let rep = castability::analyze(&built.mesh, &rr.draft, rr.inner_radius_mm());
                row += &format!("{:>12.3}", rep.undercut_fraction() * 100.0);
            }
            println!("{row}");
        }
    }

    // --- 2. How tall relief can be, side face vs crest. ---------------------
    println!("\nrelief height sweep, {orn} at 36 repeats (undercut % of area):");
    println!("  {:<10} {:>12} {:>12} {:>14}", "height", "on crest", "on side face", "side, flat band");
    for h in [0.10, 0.20, 0.30, 0.45, 0.60, 0.80, 1.00] {
        let mut row = format!("  {h:<10.2}");
        for (style, v_frac, span) in [
            (ProfileStyle::HalfRound, 0.50, 0.45),
            (ProfileStyle::HalfRound, 0.11, 0.18),
            (ProfileStyle::Flat, 0.09, 0.16),
        ] {
            let mut r = RingDesign::default();
            r.profile.apply_style(style);
            let t = ornament(orn, &r, v_frac, span, h);
            r.layers.layers.push(LayerEntry::new("orn", Layer::Tiling(t)));
            let built = mesh::build(&r, &lib, params());
            let rep = castability::analyze(&built.mesh, &r.draft, r.inner_radius_mm());
            row += &format!("{:>12.3}", rep.undercut_fraction() * 100.0);
        }
        println!("{row}");
    }

    // --- 2b. Ornament snapped onto the side faces by fit_to_side_faces. -----
    println!("\nornament fitted to the side faces (blank signet head at the top):");
    for (label, style, flange) in [
        ("Flat band", ProfileStyle::Flat, false),
        ("Low dome", ProfileStyle::LowDome, false),
        ("Half round", ProfileStyle::HalfRound, false),
        ("Half round + edge flange", ProfileStyle::HalfRound, true),
    ] {
        let mut r = RingDesign::default();
        r.profile.apply_style(style);
        if flange {
            r.profile.flange = Flange { enabled: true, v_pos: 0.0, extent_mm: 1.1, thickness_mm: 0.9, edge_round_mm: 0.15 };
        }
        let rc = r.field_context();
        let mut t = TilingLayer::default_for(orn, &rc);
        if !t.fit_to_side_faces(&rc, SIDE_FACE_MIN_DRAFT_DEG) {
            println!("  {label:<28} no side face - relief has nowhere square to the pull to sit");
            continue;
        }
        let (lo, hi) = t.v_bounds();
        println!(
            "  {label:<28} v {lo:.2}..{hi:.2} mm{}, {} tiles of {:.2} x {:.2} mm",
            if t.mirror_v { " mirrored" } else { " one side" },
            t.repeats_around,
            t.cell_size(&rc).0,
            t.cell_size(&rc).1
        );
        for h in [0.15, 0.30, 0.50, 0.80] {
            let mut rr = r.clone();
            let s = SignetLayer::fitted_to(&rc);
            rr.layers.layers.push(LayerEntry::new("signet", Layer::Signet(s)));
            let mut tt = t.clone();
            tt.height_mm = h;
            rr.layers.layers.push(LayerEntry::new("sides", Layer::Tiling(tt)));
            let built = mesh::build(&rr, &lib, params());
            let rep = castability::analyze(&built.mesh, &rr.draft, rr.inner_radius_mm());
            println!(
                "      relief {h:.2} mm  {:<18} {:>6.3}% undercut   worst {:>+7.2} deg",
                rep.verdict.label(),
                rep.undercut_fraction() * 100.0,
                rep.worst_draft_deg
            );
        }
    }

    // --- 3. The arrangement: blank head, ornament on the sides. -------------
    println!("\nsignet with ornament on the sides:");

    let (mut a, sa) = signet_ring(ProfileStyle::HalfRound);
    let t = ornament(orn, &a, 0.11, 0.18, 0.35);
    a.layers.layers.push(LayerEntry::new("side orn", Layer::Tiling(t)));
    verdict("half-round, ornament all the way round", &a, &lib);

    let (mut b, _) = signet_ring(ProfileStyle::HalfRound);
    let t = ornament(orn, &b, 0.11, 0.18, 0.35);
    b.layers.layers.push(
        LayerEntry::new("side orn", Layer::Tiling(t)).with_window(Window::around(90.0, 150.0)),
    );
    verdict("half-round, ornament windowed to the shoulders", &b, &lib);

    // A flat band with a flange at the edge: a real annular face to decorate.
    let mut f = RingDesign::default();
    f.profile.apply_style(ProfileStyle::HalfRound);
    f.profile.flange = Flange { enabled: true, v_pos: 0.0, extent_mm: 1.1, thickness_mm: 0.9, edge_round_mm: 0.15 };
    let fc = f.field_context();
    let fsig = SignetLayer::fitted_to(&fc);
    f.layers.layers.push(LayerEntry::new("signet", Layer::Signet(fsig)));
    let t = ornament(orn, &f, 0.08, 0.14, 0.35);
    f.layers.layers.push(
        LayerEntry::new("side orn", Layer::Tiling(t)).with_window(Window::around(90.0, 150.0)),
    );
    verdict("flanged edge, ornament on the flange", &f, &lib);

    // Ornament everywhere except the head, so it cannot creep onto the table.
    let (mut g, sg) = signet_ring(ProfileStyle::HalfRound);
    let t = ornament(orn, &g, 0.11, 0.18, 0.35);
    g.layers.layers.push(
        LayerEntry::new("side orn", Layer::Tiling(t)).with_window(Window::except(90.0, 90.0)),
    );
    verdict("half-round, ornament everywhere but the head", &g, &lib);
    dump("draft by region, ornamented sides (windowed):", &regions(&b, &lib, &sa));
    let _ = sg;

    // --- 4. What the window actually gates. ---------------------------------
    println!("\nwindow strength around the ring (span 150 deg at the top):");
    let w = Window::around(90.0, 150.0);
    for k in 0..12 {
        let deg = k as f64 * 30.0;
        let m = w.mask(Uv { u: c.u_of_theta(deg), v: 0.0 }, &c);
        println!("  {deg:>5.0} deg  {m:.3}  {}", "#".repeat((m * 30.0).round() as usize));
    }
}

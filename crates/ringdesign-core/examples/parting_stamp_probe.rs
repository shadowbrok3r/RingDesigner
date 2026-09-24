//! A holly leaf struck across the parting line on a factory table: its outline's monotone defect against the ray release.
//! cargo run -p ringdesign-core --release --example parting_stamp_probe -- [OUT_DIR]
use anyhow::Result;
use ringdesign_core::{AlphaLibrary, BuildParams, RingDesign, mesh, render, setting::Stamp, skin::{Atlas, Hide}};
use std::f64::consts::PI;

#[path = "common/probe.rs"]
mod probe;

/// A fesswise holly leaf: stalk at -x, tip at +x, midrib on y = 0.
#[derive(Clone, Copy)]
struct Leaf {
    len: f64,
    half_w: f64,
    spines: usize,
    spine_h: f64,
    spine_w: f64,
    /// Each spine's lean from the pull toward the tip, degrees.
    lean: f64,
    /// How far the margin dips toward the midrib between spines, mm.
    bay: f64,
    /// Half-height of the square ends at stalk and tip, mm.
    tip_half: f64,
}

const HOLLY: Leaf = Leaf { len: 9.0, half_w: 2.4, spines: 3, spine_h: 1.0, spine_w: 0.8, lean: 0.0, bay: 0.3, tip_half: 0.09 };

impl Leaf {
    fn envelope(&self, x: f64) -> f64 {
        let s = ((x + self.len * 0.5) / self.len).clamp(0.0, 1.0);
        self.tip_half.max(self.half_w * (PI * s).sin().powf(0.75))
    }
    /// The margin above the midrib, stalk to tip.
    fn upper(&self) -> Vec<[f64; 2]> {
        let spines: Vec<f64> = (1..=self.spines).map(|k| -self.len * 0.5 + self.len * k as f64 / (self.spines + 1) as f64).collect();
        let bases: Vec<(f64, f64)> = spines.iter().map(|c| (c - self.spine_w * 0.5, c + self.spine_w * 0.5)).collect();
        let dip = |x: f64| {
            let mut from = -self.len * 0.5;
            for (a, b) in &bases {
                if x < *a {
                    return self.bay * (PI * (x - from) / (a - from)).sin().max(0.0);
                }
                from = *b;
            }
            self.bay * (PI * (x - from) / (self.len * 0.5 - from)).sin().max(0.0)
        };
        let margin = |x: f64| (self.envelope(x) - dip(x)).max(self.tip_half);
        let mut out = vec![[-self.len * 0.5, 0.0], [-self.len * 0.5, self.tip_half]];
        let mut x = -self.len * 0.5;
        for (k, (a, b)) in bases.iter().enumerate() {
            while x + 0.1 < *a {
                x += 0.1;
                out.push([x, margin(x)]);
            }
            let (ya, yb) = (margin(*a), margin(*b));
            let lean = self.lean.to_radians();
            out.push([*a, ya]);
            out.push([spines[k] + self.spine_h * lean.sin(), 0.5 * (ya + yb) + self.spine_h * lean.cos()]);
            out.push([*b, yb]);
            x = *b;
        }
        while x + 0.1 < self.len * 0.5 {
            x += 0.1;
            out.push([x, margin(x)]);
        }
        out.push([self.len * 0.5, self.tip_half]);
        out.push([self.len * 0.5, 0.0]);
        out
    }
    /// The whole outline, counter-clockwise: the lower margin out to the tip, the upper back to the stalk.
    fn outline(&self) -> Vec<[f64; 2]> {
        let upper = self.upper();
        let mut out: Vec<[f64; 2]> = upper.iter().map(|p| [p[0], -p[1]]).filter(|p| p[1] != 0.0).collect();
        out.insert(0, [-self.len * 0.5, 0.0]);
        out.extend(upper.iter().rev().filter(|p| p[1] != 0.0).copied());
        out.dedup();
        out
    }
}

/// The monotone rule read off an outline: widest gap on a line along the finger, metal beyond it, gap area, and miss of z = 0.
struct Defect {
    gap_mm: f64,
    hang_mm: f64,
    area_mm2: f64,
    off_mm: f64,
}

fn crossings(outline: &[[f64; 2]], x: f64) -> Vec<f64> {
    let n = outline.len();
    let mut ys: Vec<f64> = (0..n)
        .filter_map(|i| {
            let (p, q) = (outline[i], outline[(i + 1) % n]);
            ((p[0] <= x) != (q[0] <= x)).then(|| p[1] + (q[1] - p[1]) * (x - p[0]) / (q[0] - p[0]))
        })
        .collect();
    ys.sort_by(f64::total_cmp);
    ys
}

fn defect(outline: &[[f64; 2]]) -> Defect {
    let (lo, hi) = outline.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
    let mut d = Defect { gap_mm: 0.0, hang_mm: 0.0, area_mm2: 0.0, off_mm: 0.0 };
    let step = 0.005;
    let mut x = lo + step * 0.5;
    while x < hi {
        let ys = crossings(outline, x);
        let pieces: Vec<(f64, f64)> = ys.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect();
        if !pieces.iter().any(|(a, b)| *a <= 0.0 && 0.0 <= *b) {
            d.off_mm = d.off_mm.max(pieces.iter().map(|(a, b)| a.abs().min(b.abs())).fold(f64::MAX, f64::min));
        }
        for w in pieces.windows(2) {
            let gap = w[1].0 - w[0].1;
            d.gap_mm = d.gap_mm.max(gap);
            d.hang_mm = d.hang_mm.max(if w[1].0 > 0.0 { w[1].1 - w[1].0 } else { w[0].1 - w[0].0 });
            d.area_mm2 += gap * step;
        }
        x += step;
    }
    d
}

/// The outline filled to the sand's rule: on every line along the finger, from its lowest point or z = 0 to its highest.
fn filled(outline: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let (lo, hi) = outline.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
    let xs: Vec<f64> = (0..=((hi - lo) / 0.05).ceil() as usize).map(|i| (lo + i as f64 * 0.05).min(hi - 1e-6).max(lo + 1e-6)).collect();
    let span: Vec<(f64, f64, f64)> = xs.iter().map(|x| {
        let ys = crossings(outline, *x);
        (*x, ys.first().copied().unwrap_or(0.0).min(0.0), ys.last().copied().unwrap_or(0.0).max(0.0))
    }).collect();
    let mut out: Vec<[f64; 2]> = span.iter().map(|s| [s.0, s.1]).collect();
    out.extend(span.iter().rev().map(|s| [s.0, s.2]));
    out.dedup();
    out
}

fn struck(base: &RingDesign, at: (f64, f64), outline: Vec<[f64; 2]>) -> RingDesign {
    let mut d = base.clone();
    d.stamps = vec![Stamp {
        name: "Holly leaf".into(),
        theta_deg: at.0,
        v_mm: at.1,
        rot_deg: 0.0,
        outline,
        height_mm: 0.4,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
    }];
    d
}

fn judge(label: &str, base: &RingDesign, at: (f64, f64), outline: Vec<[f64; 2]>, dy: f64, lib: &AlphaLibrary) -> Result<Option<mesh::BuildResult>> {
    let outline: Vec<[f64; 2]> = outline.iter().map(|p| [p[0], p[1] + dy]).collect();
    let k = defect(&outline);
    let d = struck(base, at, outline.clone());
    let params = BuildParams { theta_steps: 900, profile_steps: 448, refine: None, ..Default::default() };
    let built = mesh::try_build(&d, lib, params)?;
    let (inspection, fine) = probe::pull(&d, lib, params, 0.075)?;
    let coarse = probe::pull(&d, lib, BuildParams { theta_steps: 384, profile_steps: 192, ..params }, 0.1)?.0;
    println!(
        "{label:<30} {:>3} pts | gap {:.3} hang {:.3} area {:.4} off {:.3} | stamped {} {:?} | pull {} | 0.075 {} | 384x192 {}",
        outline.len(),
        k.gap_mm,
        k.hang_mm,
        k.area_mm2,
        k.off_mm,
        built.solids.stamped,
        built.solids.notes,
        probe::release_line(&inspection.release),
        probe::release_line(&fine),
        probe::release_line(&coarse.release),
    );
    Ok(Some(built))
}

fn main() -> Result<()> {
    let out = std::env::args().nth(1);
    let lib = AlphaLibrary::builtin();
    let base = probe::stock("006", true, None)?;
    let atlas = Atlas::of(&base, 2048, 768)?;
    let hide = Hide::of(&atlas);
    let at = hide.crest_at(&atlas, 0.0);
    println!("006 Square master, face {:.1} x {:.1} mm; the leaf's midrib at {:.3}°, v {:.4} mm", base.shank.head.length_mm, base.profile.width_mm, at.0, at.1);
    println!("== Spine lean, bays 0.3 mm (a spine overhangs once it leans past {:.1}°)", (HOLLY.spine_w * 0.5 / HOLLY.spine_h).asin().to_degrees());
    let mut hero = None;
    for lean in [0.0, 20.0, 25.0, 30.0, 40.0, -25.0, -40.0] {
        let b = judge(&format!("lean {lean:+}°"), &base, at, Leaf { lean, ..HOLLY }.outline(), 0.0, &lib)?;
        if lean == 20.0 {
            hero = b;
        }
    }
    println!("== Bay depth, spines upright");
    for bay in [0.0, 0.6] {
        judge(&format!("bays {bay} mm"), &base, at, Leaf { bay, ..HOLLY }.outline(), 0.0, &lib)?;
    }
    println!("== Off the parting line");
    for tip_half in [0.0, 0.09] {
        for dy in [0.02, 0.05, 0.1] {
            judge(&format!("ends {tip_half} mm, shifted {dy} mm"), &base, at, Leaf { tip_half, ..HOLLY }.outline(), dy, &lib)?;
        }
    }
    println!("== The fallback: a 40° leaf filled to the rule, its bays left to the bench");
    let wild = Leaf { lean: 40.0, ..HOLLY }.outline();
    judge("lean +40°, filled", &base, at, filled(&wild), 0.0, &lib)?;
    if let (Some(out), Some(built)) = (out, hero) {
        std::fs::create_dir_all(&out)?;
        for (view, yaw, pitch) in [("face", 0.0, std::f64::consts::FRAC_PI_2), ("hero", 0.48, 1.0)] {
            render::write_png(std::path::Path::new(&out).join(format!("holly-{view}.png")), &built.mesh, yaw, pitch, 1000, render::GOLD)?;
        }
    }
    Ok(())
}

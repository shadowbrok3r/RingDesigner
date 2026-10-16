//! The façade round the ring: a portal on the top, a different window in every bay, buttresses between, a gallery on the palm.
use crate::geom::*;
use crate::windows;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const L1: usize = 1;
pub const L2: usize = 2;
pub const L3: usize = 3;
pub const L4: usize = 4;

/// One bay of the façade, `x0..x1` round the ring.
pub struct Section {
    pub name: String,
    pub x0: f64,
    pub x1: f64,
}

pub struct Facade {
    pub circumference: f64,
    /// Half the art's height inside its border.
    pub art: f64,
    pub sections: Vec<Section>,
    pub pieces: Vec<Piece>,
    pub ornaments: Vec<Ornament>,
}

/// Bays round the ring from the portal's right edge, their widths on a 72 mm circumference; the gallery takes the rest.
const PLAN: [(&str, f64); 11] = [
    ("Buttress", 1.1),
    ("Rose window", 7.4),
    ("Buttress", 1.1),
    ("Spire", 5.0),
    ("Buttress", 1.1),
    ("Gallery", 0.0),
    ("Buttress", 1.1),
    ("Twin lancet window", 6.8),
    ("Buttress", 1.1),
    ("Flamboyant window", 7.4),
    ("Buttress", 1.1),
];
const PORTAL_W: f64 = 13.0;

pub fn facade(circumference: f64, art: f64) -> Facade {
    let s = circumference / 72.0;
    let fixed: f64 = PORTAL_W + PLAN.iter().map(|(_, w)| w).sum::<f64>();
    let gallery = circumference - fixed * s;
    let mut sections = vec![Section { name: "Portal".into(), x0: -0.5 * PORTAL_W * s, x1: 0.5 * PORTAL_W * s }];
    let mut x = 0.5 * PORTAL_W * s;
    let mut buttresses = 0;
    for (name, w) in PLAN {
        let w = if name == "Gallery" { gallery } else { w * s };
        let name = if name == "Buttress" {
            buttresses += 1;
            format!("Buttress {buttresses}")
        } else {
            name.to_string()
        };
        sections.push(Section { name, x0: x, x1: x + w });
        x += w;
    }
    let mut d = Draw::default();
    for sec in &sections {
        let (x0, x1) = (sec.x0, sec.x1);
        match sec.name.as_str() {
            "Portal" => windows::portal(&mut d, x0, x1, art),
            "Rose window" => rose(&mut d, x0, x1, art),
            "Spire" => spire(&mut d, x0, x1, art),
            "Gallery" => gallery_bay(&mut d, x0, x1, art),
            "Twin lancet window" => windows::twin(&mut d, x0, x1, art),
            "Flamboyant window" => windows::flamboyant(&mut d, x0, x1, art),
            _ => buttress(&mut d, x0, x1, art),
        }
    }
    Facade { circumference, art, sections, pieces: d.pieces, ornaments: d.ornaments }
}

/// A gable band `w` wide from `(xc ± half, y0)` to `(xc, top)`, crockets on its outer edges every `pitch`.
pub fn gable(d: &mut Draw, xc: f64, half: f64, y0: f64, top: f64, w: f64, pitch: f64) {
    let apex = [xc, top];
    let mut parts = Vec::new();
    for s in [-1.0, 1.0] {
        let foot = [xc + s * half, y0];
        parts.push(seg(foot, apex, w));
        let e = unit(sub(apex, foot));
        let out = if s < 0.0 { [-e[1], e[0]] } else { [e[1], -e[0]] };
        let l = len(sub(apex, foot));
        let mut t = 0.3;
        while t < l - 0.3 {
            parts.push(crocket(add(lerp(foot, apex, t / l), mul(out, 0.5 * w - 0.03)), out));
            t += pitch;
        }
    }
    d.solid(L1, union(parts));
}

fn buttress(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let xc = 0.5 * (x0 + x1);
    let foot = -y - 0.3;
    let pier = union(vec![
        rect(xc - 0.36, foot, xc + 0.36, 0.5),
        poly(vec![[xc - 0.36, 0.48], [xc + 0.36, 0.48], [xc + 0.29, 0.66], [xc - 0.29, 0.66]]),
        rect(xc - 0.29, 0.6, xc + 0.29, 1.05),
    ]);
    let niches = union(vec![lancet(xc - 0.17, xc + 0.17, -2.35, -1.55, 0.75), lancet(xc - 0.17, xc + 0.17, -1.05, -0.2, 0.75)]);
    d.pierced(L1, pier, niches);
    d.solid(L3, rect(xc - 0.3, -2.5, xc + 0.3, 0.3));
    d.solid(L1, pinnacle(xc, 0.95, 1.3, y + 0.15, 0.46));
}

fn rose(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let xc = 0.5 * (x0 + x1);
    let c = [xc, 0.0];
    let r_o = y + 0.05;
    let tier = |r0: f64, r1: f64| inter(vec![disc(c, r1), not(disc(c, r0))]);
    // Frame: an outer moulding over an inner ring.
    d.solid(L1, ring(c, r_o - 0.16, 0.32));
    d.solid(L2, ring(c, r_o - 0.45, 0.28));
    // Outer tier: sixteen trefoil-headed lights, every other spoke a major one.
    let outer = union((0..16).map(|k| radial_lancet(c, FRAC_PI_2 + TAU * f64::from(k) / 16.0, PI / 16.0, 0.2, 1.4, 1.8, 0.8)).collect());
    d.pierced(L2, tier(1.3, r_o - 0.5), outer);
    let spokes = (0..8)
        .map(|k| {
            let t = FRAC_PI_2 + PI / 16.0 + PI / 4.0 * f64::from(k);
            seg(polar(c, 1.3, t), polar(c, r_o - 0.3, t), 0.22)
        })
        .collect();
    d.solid(L1, union(spokes));
    d.solid(L1, ring(c, 1.25, 0.24));
    // Inner tier of eight lights, and the eye: a ring round a sexfoil.
    let inner = union((0..8).map(|k| radial_lancet(c, FRAC_PI_2 + PI / 8.0 + TAU * f64::from(k) / 8.0, PI / 8.0, 0.2, 0.64, 0.78, 0.8)).collect());
    d.pierced(L2, tier(0.3, 1.2), inner);
    d.solid(L2, ring(c, 0.52, 0.2));
    d.pierced(L3, disc(c, 0.5), foiled(c, 6, 0.36, FRAC_PI_2));
    // Spandrels: a trefoil roundel in each corner, and down each side two daggers round a quatrefoil.
    let hw = 0.5 * (x1 - x0);
    let (mut rings, mut lights) = (Vec::new(), Vec::new());
    for (sx, sy) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let r_rd = 0.58;
        let cy = sy * (y - r_rd + 0.06);
        let dist = r_o + r_rd - 0.12;
        let cx = xc + sx * (dist * dist - cy * cy).max(0.0).sqrt();
        let cx = if sx < 0.0 { cx.max(x0 + r_rd + 0.12) } else { cx.min(x1 - r_rd - 0.12) };
        rings.push(ring([cx, cy], r_rd - 0.1, 0.2));
        lights.push(foiled([cx, cy], 3, r_rd - 0.24, if sy > 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 }));
        lights.push(mouchette([xc + sx * (hw - 0.4), sy * 1.15], 0.19, [xc + sx * (r_o + 0.2), sy * 0.35], 0.08 * sx * sy));
    }
    for sx in [-1.0, 1.0] {
        lights.push(foiled([xc + sx * (hw - 0.42), 0.0], 4, 0.25, PI / 4.0));
    }
    d.solid(L1, union(rings));
    d.pierced(L3, inter(vec![rect(x0 - 0.06, -y - 0.06, x1 + 0.06, y + 0.06), not(disc(c, r_o - 0.2))]), union(lights));
}

fn spire(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let xc = 0.5 * (x0 + x1);
    let foot = -y - 0.3;
    // A niche of two lights at the foot.
    windows::two_lights(d, xc, -1.75, 1.05, 1.68, 0.28, foot, -y + 0.25);
    // A gable over the niche that runs on up as the spire.
    let (g0, g1, apex) = ([xc - 1.45, -1.25], [xc - 0.6, 0.7], [xc, y + 0.15]);
    let mirror = |p: V| [2.0 * xc - p[0], p[1]];
    let mut frame = vec![seg(g1, mirror(g1), 0.2)];
    for (a, b) in [(g0, g1), (g1, apex)] {
        for s in [false, true] {
            let (a, b) = if s { (mirror(a), mirror(b)) } else { (a, b) };
            frame.push(seg(a, b, 0.24));
            let e = unit(sub(b, a));
            let out = if s { [e[1], -e[0]] } else { [-e[1], e[0]] };
            let l = len(sub(b, a));
            let mut t = 0.75;
            while t < l - 0.25 {
                frame.push(crocket(add(lerp(a, b, t / l), mul(out, 0.1)), out));
                t += 0.36;
            }
        }
    }
    d.solid(L1, union(frame));
    let tympanum = inter(vec![poly(vec![g0, mirror(g0), mirror(g1), g1]), not(pointed(xc, -1.75, 1.2, 1.83))]);
    let tre = foiled([xc, 0.27], 3, 0.24, FRAC_PI_2);
    d.pierced(L3, tympanum.clone(), tre.clone());
    d.pierced(L4, tympanum, offset(tre, -0.07));
    let lights = union(vec![foiled([xc, 1.22], 4, 0.27, PI / 4.0), lancet(xc - 0.12, xc + 0.12, 1.68, 1.9, 0.9)]);
    d.pierced(L3, poly(vec![g1, apex, mirror(g1)]), lights);
    // Flanking pinnacles.
    for s in [-1.0, 1.0] {
        d.solid(L1, pinnacle(xc + s * 2.05, foot, 0.45, 2.2, 0.3));
    }
}

fn gallery_bay(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let foot = -y - 0.3;
    let n = ((x1 - x0) / 3.2).round().max(3.0) as usize;
    let pw = (x1 - x0) / n as f64;
    let (rail_y, rail_w) = (-0.25, 0.26);
    d.solid(L1, rect(x0 - 0.05, rail_y - 0.5 * rail_w, x1 + 0.05, rail_y + 0.5 * rail_w));
    for i in 0..n {
        let a = x0 + i as f64 * pw;
        let b = a + pw;
        let xc = 0.5 * (a + b);
        if i > 0 {
            d.solid(L1, rect(a - 0.14, foot, a + 0.14, y + 0.3));
        }
        // Balustrade: a roundel of four wheeling daggers between the rail and the border.
        let r = (0.5 * (y - rail_y - 0.5 * rail_w) + 0.04).min(0.5 * pw - 0.1);
        let c = [xc, y + 0.04 - r];
        d.solid(L1, ring(c, r - 0.12, 0.24));
        let ri = r - 0.24;
        let mut daggers: Vec<Shape> = (0..4)
            .map(|k| {
                let t = FRAC_PI_2 + f64::from(k) * FRAC_PI_2;
                mouchette(polar(c, 0.6 * ri, t), 0.2 * ri, polar(c, 0.55 * ri, t + 0.95), -0.08 * ri)
            })
            .collect();
        daggers.push(disc(c, 0.13));
        let daggers = union(daggers);
        d.pierced(L2, disc(c, ri + 0.03), daggers);
        let (pa, pb) = (if i == 0 { a - 0.35 } else { a }, if i + 1 == n { b + 0.35 } else { b });
        d.solid(L3, inter(vec![rect(pa, rail_y, pb, y + 0.06), not(disc(c, r - 0.05))]));
        // Arcade: two trefoil-headed arches sharing a colonnette.
        let (ys, rho) = (-1.75, 1.0);
        let mut arches = Vec::new();
        let mut lights = Vec::new();
        for s in [-1.0, 1.0] {
            let outer = if s < 0.0 { a + 0.24 } else { b - 0.24 };
            let (ax, a_c) = (0.5 * (outer + xc), 0.5 * (outer - xc).abs());
            arches.push(arch_band(ax, ys, a_c, rho, 0.2));
            arches.push(seg([outer, foot], [outer, ys], 0.2));
            let inner_top = ys + rise(a_c - 0.1, rho - 0.1) - 0.08;
            let lw = a_c - 0.16;
            let spring = inner_top - 1.12 * lw;
            lights.push(trefoil_lancet(ax - lw, ax + lw, -y + 0.25, spring));
        }
        arches.push(seg([xc, foot], [xc, ys], 0.2));
        d.solid(L2, union(arches));
        d.pierced(L3, rect(pa, foot, pb, rail_y), union(lights));
    }
}

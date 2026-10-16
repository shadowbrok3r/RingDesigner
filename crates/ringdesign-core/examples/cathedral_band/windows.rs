//! The pointed windows and the portal.
use crate::facade::{gable, L1, L2, L3, L4};
use crate::geom::*;
use std::f64::consts::{FRAC_PI_2, PI};

/// Height of a pointed arch's curve of half-width `a` and radius `rho` over `x`.
fn arch_y(xc: f64, ys: f64, a: f64, rho: f64, x: f64) -> f64 {
    let c = if x <= xc { xc - a + rho } else { xc + a - rho };
    ys + (rho * rho - (x - c) * (x - c)).max(0.0).sqrt()
}

/// Half-width of a pointed arch's curve at height `y`.
fn arch_half(xc: f64, ys: f64, a: f64, rho: f64, y: f64) -> f64 {
    let c = xc - a + rho;
    xc - (c - (rho * rho - (y - ys) * (y - ys)).max(0.0).sqrt())
}

/// The inside of a framed arch, grown under its frame, from `foot` up.
fn opening(xc: f64, ys: f64, a: f64, rho: f64, foot: f64) -> Shape {
    union(vec![pointed(xc, ys, a, rho), rect(xc - a, foot, xc + a, ys + 0.01)])
}

/// A pointed frame `fw` wide on jambs down to `foot`.
fn frame(xc: f64, ys: f64, a: f64, rho: f64, fw: f64, foot: f64) -> Shape {
    union(vec![arch_band(xc, ys, a, rho, fw), seg([xc - a, foot], [xc - a, ys], fw), seg([xc + a, foot], [xc + a, ys], fw)])
}

/// Diamond quarries of lead over the glass of `lights` below a transom at `y1`, symmetric about `xc`.
fn leading(lights: &Shape, xc: f64, y0: f64, y1: f64, half: f64) -> Shape {
    union(vec![
        quarries(inter(vec![lights.clone(), below(y1)]), [xc - half, y0], [xc + half, y1], 0.48, 0.62, 0.17, [xc, y0]),
        inter(vec![seg([xc - half, y1], [xc + half, y1], 0.17), lights.clone()]),
    ])
}

/// A heart: two lobes of radius `r` either side of `c` by `sep`, closing to a point `h` below it.
fn heart(c: V, r: f64, sep: f64, h: f64) -> Shape {
    let (x, y) = (c[0], c[1]);
    union(vec![
        disc([x - sep, y], r),
        disc([x + sep, y], r),
        poly(vec![[x - sep - 0.96 * r, y - 0.28 * r], [x, y - h], [x + sep + 0.96 * r, y - 0.28 * r], [x, y + 0.3 * r]]),
    ])
}

/// A framed window of two trefoil-headed lights and a quatrefoil oculus under a pointed arch, glazed in leaded quarries.
pub fn two_lights(d: &mut Draw, xc: f64, ys: f64, a: f64, rho: f64, fw: f64, foot: f64, sill: f64) {
    d.solid(L1, frame(xc, ys, a, rho, fw, foot));
    let (ai, ri) = (a - 0.5 * fw, rho - 0.5 * fw);
    let apex = ys + rise(ai, ri);
    let m = 0.11;
    let lw = 0.5 * (ai - m - 0.06);
    let mut lancets = Vec::new();
    let (mut heads, mut springs) = (f64::MIN, f64::MAX);
    for s in [-1.0, 1.0] {
        let lx = xc + s * (m + 0.03 + lw);
        let spring = arch_y(xc, ys, ai, ri, lx) - 0.14 - 1.12 * lw;
        heads = heads.max(spring + 0.5 * lw);
        springs = springs.min(spring);
        lancets.push(trefoil_lancet(lx - lw, lx + lw, sill, spring));
    }
    let mut ro = 0.5 * (apex - 0.05 - heads);
    let mut yo = apex - 0.05 - ro;
    while ro > 0.15 && arch_half(xc, ys, ai, ri, yo) < ro + 0.04 {
        ro -= 0.01;
        yo = apex - 0.05 - ro;
    }
    let oc = [xc, yo];
    d.solid(L2, union(vec![rect(xc - m, foot, xc + m, yo - ro + 0.08), ring(oc, ro - 0.1, 0.2)]));
    let eye = foiled(oc, 4, ro - 0.22, PI / 4.0);
    let lancets = union(lancets);
    d.pierced(L3, opening(xc, ys, ai + 0.06, ri + 0.06, foot), union(vec![lancets.clone(), eye.clone()]));
    d.pierced(L4, disc(oc, ro), offset(eye, -0.07));
    d.solid(L4, leading(&lancets, xc, sill, springs, ai));
}

pub fn portal(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let xc = 0.5 * (x0 + x1);
    let foot = -y - 0.3;
    let (a0, rho0, ys) = (1.7, 1.9, -2.0);
    // Three orders round the doorway: a roll, a hollow set with ballflowers, a roll.
    let order = |t: f64, w: f64| frame(xc, ys, a0 + t, rho0 + t, w, foot);
    d.solid(L2, order(0.14, 0.28));
    d.solid(L3, order(0.48, 0.44));
    d.solid(L2, order(0.82, 0.28));
    let (ab, rb) = (a0 + 0.48, rho0 + 0.48);
    let h = rise(ab, rb);
    let top = h.atan2(ab - rb);
    let n = ((PI - top) * rb / 0.44).round().max(1.0);
    for k in 0..n as usize {
        let t = PI - (k as f64 + 0.5) * (PI - top) / n;
        let cl = [xc - ab + rb, ys];
        let p = polar(cl, rb, t);
        d.ornament("Ballflower", L1, L3, disc(p, 0.1));
        d.ornament("Ballflower", L1, L3, disc([2.0 * xc - p[0], p[1]], 0.1));
    }
    for s in [-1.0, 1.0] {
        d.ornament("Ballflower", L1, L3, disc([xc + s * ab, -2.38], 0.1));
    }
    // Ogee hood, crocketed, with a finial into the border.
    let o = ogee(xc, ys, a0 + 1.07, 2.15 - ys, rho0 + 1.07);
    d.solid(L1, o.band(0.26, foot));
    d.solid(L1, union(o.outer_edge(0.26, 0.42, -0.6).into_iter().map(|(p, dir)| crocket(p, dir)).collect()));
    d.solid(L1, finial([xc, 2.05], y + 0.12));
    let roll_top = ys + rise(a0 + 0.95, rho0 + 0.95);
    let hood_in = o.c2[1] - ((o.rho2 + 0.13).powi(2) - (xc - o.c2[0]).powi(2)).max(0.0).sqrt();
    let reach = (0.5 * (hood_in - roll_top) - 0.08).min(0.24);
    let spandrel = inter(vec![o.inside(0.0), not(pointed(xc, ys, a0 + 0.9, rho0 + 0.9))]);
    let tre = foiled([xc, 0.5 * (roll_top + hood_in)], 3, reach, FRAC_PI_2);
    d.pierced(L3, spandrel.clone(), tre.clone());
    d.pierced(L4, spandrel, offset(tre, -0.07));
    // Tympanum over the lintel: a heart of two daggers between two trefoils.
    let (lintel_lo, lintel_hi) = (-1.25, -1.03);
    d.solid(L2, rect(xc - a0 - 0.06, lintel_lo, xc + a0 + 0.06, lintel_hi));
    let tympanum = inter(vec![opening(xc, ys, a0 + 0.06, rho0 + 0.06, foot), above(lintel_hi - 0.03)]);
    let mut lights = vec![heart([xc, -0.42], 0.2, 0.19, 0.52)];
    for s in [-1.0, 1.0] {
        lights.push(foiled([xc + s * 0.88, -0.83], 3, 0.19, FRAC_PI_2));
    }
    d.pierced(L3, tympanum, union(lights));
    d.solid(L3, seg([xc, -1.0], [xc, -0.15], 0.18));
    // Doors: two leaves of blind panels either side of a trumeau.
    let mut panels = Vec::new();
    for s in [-1.0, 1.0] {
        for (c0, c1) in [(0.33, 0.835), (1.015, 1.52)] {
            let (px0, px1) = if s < 0.0 { (xc - c1, xc - c0) } else { (xc + c0, xc + c1) };
            panels.push(rect(px0, -y + 0.18, px1, -2.115));
            panels.push(lancet(px0, px1, -1.935, -1.804, 0.8));
        }
    }
    d.pierced(L4, rect(xc - a0 - 0.06, foot, xc + a0 + 0.06, lintel_lo + 0.03), union(panels));
    d.solid(L2, rect(xc - 0.15, foot, xc + 0.15, lintel_lo + 0.02));
    // Pinnacles, then a blind window under a crocketed gable each side.
    for s in [-1.0, 1.0] {
        d.solid(L1, pinnacle(xc + s * 3.55, foot, 0.4, 2.35, 0.32));
        let px = xc + s * 5.1;
        two_lights(d, px, -0.6, 1.0, 1.5, 0.26, foot, -y + 0.25);
        gable(d, px, 1.3, 0.15, 2.4, 0.24, 0.4);
        d.solid(L1, finial([px, 2.3], y + 0.12));
        let plate = inter(vec![poly(vec![[px - 1.3, 0.15], [px + 1.3, 0.15], [px, 2.4]]), not(pointed(px, -0.6, 1.15, 1.65))]);
        let tre = foiled([px, 1.6], 3, 0.2, FRAC_PI_2);
        d.pierced(L3, plate.clone(), tre.clone());
        d.pierced(L4, plate, offset(tre, -0.07));
    }
}

pub fn twin(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let xc = 0.5 * (x0 + x1);
    let foot = -y - 0.3;
    let sill = -y + 0.25;
    let (ys, a, rho, fw) = (-0.9, 1.75, 2.6, 0.3);
    // A crocketed gable carrying a cross, its field behind the arch.
    gable(d, xc, 2.5, 0.0, 2.35, 0.26, 0.42);
    let (cy, arm) = (2.57, 0.3);
    d.solid(L1, union(vec![seg([xc, 2.3], [xc, y + 0.2], 0.2), seg([xc - arm, cy], [xc + arm, cy], 0.19), disc([xc - arm, cy], 0.1), disc([xc + arm, cy], 0.1)]));
    d.solid(L3, inter(vec![poly(vec![[xc - 2.5, 0.0], [xc + 2.5, 0.0], [xc, 2.35]]), not(pointed(xc, ys, a + 0.1, rho + 0.1))]));
    // The window: two lights under sub-arches and an oculus resting on them.
    d.solid(L1, frame(xc, ys, a, rho, fw, foot));
    let (ai, ri) = (a - 0.5 * fw, rho - 0.5 * fw);
    let (yss, rs) = (ys + 0.25, 1.25);
    let xs = 0.5 * (ai - 0.1);
    let a_s = xs;
    let mut bars = vec![rect(xc - 0.13, foot, xc + 0.13, yss)];
    let mut lancets = Vec::new();
    for s in [-1.0, 1.0] {
        let sx = xc + s * xs;
        bars.push(frame(sx, yss, a_s, rs, 0.2, foot));
        let (ais, ris) = (a_s - 0.1, rs - 0.1);
        let lw = ais - 0.06;
        let spring = yss + rise(ais, ris) - 0.1 - 1.12 * lw;
        lancets.push(trefoil_lancet(sx - lw, sx + lw, sill, spring));
    }
    // The oculus touches the arch inside and both sub-arches outside.
    let cl = [xc - ai + ri, ys];
    let cs = [xc + xs - a_s + rs, yss];
    let centre = |r: f64| ys + ((ri - r).powi(2) - (cl[0] - xc).powi(2)).max(0.0).sqrt();
    let gap = |r: f64| len(sub([xc, centre(r)], cs)) - (rs + 0.1) - r;
    let (mut lo, mut hi) = (0.2, 1.2);
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if gap(mid) > 0.0 { lo = mid } else { hi = mid }
    }
    let ro = lo + 0.02;
    let oc = [xc, centre(lo)];
    bars.push(ring(oc, ro - 0.1, 0.2));
    d.solid(L2, union(bars));
    let eye = foiled(oc, 4, ro - 0.24, PI / 4.0);
    let lancets = union(lancets);
    d.pierced(L3, opening(xc, ys, ai + 0.06, ri + 0.06, foot), union(vec![lancets.clone(), eye.clone()]));
    d.pierced(L4, disc(oc, ro), offset(eye, -0.07));
    let spring = yss + rise(a_s - 0.1, rs - 0.1) - 0.1 - 1.12 * (a_s - 0.16);
    d.solid(L4, leading(&lancets, xc, sill, spring, ai));
    for s in [-1.0, 1.0] {
        d.solid(L1, pinnacle(xc + s * 2.95, foot, 0.25, 2.3, 0.3));
    }
}

fn lancets_union(l: &[Shape]) -> Shape {
    union(l.to_vec())
}

pub fn flamboyant(d: &mut Draw, x0: f64, x1: f64, y: f64) {
    let xc = 0.5 * (x0 + x1);
    let foot = -y - 0.3;
    let sill = -y + 0.25;
    let (ys, a, rho, fw) = (-1.2, 1.95, 2.4, 0.3);
    d.solid(L1, frame(xc, ys, a, rho, fw, foot));
    let (ai, ri) = (a - 0.5 * fw, rho - 0.5 * fw);
    // Ogee hood over the frame, crocketed, with a finial.
    let o = ogee(xc, ys, a + 0.26, 2.35 - ys, rho + 0.26);
    d.solid(L1, o.band(0.24, foot));
    d.solid(L1, union(o.outer_edge(0.24, 0.4, -0.3).into_iter().map(|(p, dir)| crocket(p, dir)).collect()));
    d.solid(L1, finial([xc, 2.25], y + 0.12));
    let frame_top = ys + rise(a + 0.15, rho + 0.15);
    let hood_in = o.c2[1] - ((o.rho2 + 0.12).powi(2) - (xc - o.c2[0]).powi(2)).max(0.0).sqrt();
    let reach = (0.5 * (hood_in - frame_top) - 0.08).min(0.22);
    let spandrel = inter(vec![o.inside(0.0), not(pointed(xc, ys, a + 0.1, rho + 0.1))]);
    let tre = foiled([xc, 0.5 * (frame_top + hood_in)], 3, reach, FRAC_PI_2);
    d.pierced(L3, spandrel.clone(), tre.clone());
    d.pierced(L4, spandrel, offset(tre, -0.07));
    // Three cusped lights between two mullions.
    let m = 0.11;
    let lw = (2.0 * ai - 4.0 * m - 0.08) / 6.0;
    let mut lancets: Vec<Shape> = Vec::new();
    let mut mullions = Vec::new();
    for k in -1..=1 {
        let lx = xc + f64::from(k) * (2.0 * lw + 2.0 * m);
        lancets.push(trefoil_lancet(lx - lw, lx + lw, sill, ys + 0.22));
        if k != 1 {
            let mx = lx + lw + m;
            mullions.push(rect(mx - m, foot, mx + m, ys + 0.35));
        }
    }
    d.solid(L2, union(mullions));
    // Tracery head: a heart split by a bar over the middle light, a flame over each side light.
    let mut lights = vec![lancets_union(&lancets), heart([xc, 0.42], 0.25, 0.27, 0.62)];
    for s in [-1.0, 1.0] {
        lights.push(mouchette([xc + s * 1.1, 0.0], 0.22, [xc + s * 0.62, 0.72], -0.08 * s));
    }
    d.pierced(L3, opening(xc, ys, ai + 0.06, ri + 0.06, foot), union(lights));
    d.solid(L3, seg([xc, -0.3], [xc, 0.75], 0.18));
    d.solid(L4, leading(&lancets_union(&lancets), xc, sill, ys + 0.22, ai));
    for s in [-1.0, 1.0] {
        d.solid(L1, pinnacle(xc + s * 3.15, foot, 0.1, 2.3, 0.3));
    }
}

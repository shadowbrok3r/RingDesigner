//! The façade sampled bay by bay: every level's pockets as outlines, each cut down to the next level, and the raised ornaments.
use crate::facade::Facade;
use crate::field::{area, bounds, contours, densify, opened, simplify, Grid};
use crate::geom::{Piece, V, CAP};
use anyhow::{ensure, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Metal levels under the rails: 1 just under them, 5 the deepest floor.
pub const LEVELS: usize = 5;

/// A pocket cut from level 1 down to level `floor`.
#[derive(Clone, Debug)]
pub struct Opening {
    pub name: String,
    pub floor: usize,
    pub outline: Vec<V>,
    pub at: V,
}

/// An ornament struck back on the floor at level `floor`, its top at level `top`.
#[derive(Clone, Debug)]
pub struct Raised {
    pub name: String,
    pub top: usize,
    pub floor: usize,
    pub outline: Vec<V>,
    pub at: V,
}

pub struct Layout {
    pub facade: Facade,
    pub openings: Vec<Opening>,
    pub raised: Vec<Raised>,
}

pub struct Settings {
    pub step: f64,
    pub corner_r: f64,
    /// How far each deeper pocket stands inside the one above it where they would share a wall.
    pub eps: f64,
    pub margin: f64,
}

const FLOOR_NAMES: [&str; 6] = ["", "", "moulding", "tracery", "fine tracery", "glass"];

/// Per level 1 to 4, the signed distance to that level's metal: negative inside, the border counted at level 1.
fn level_fields(g: &Grid, pieces: &[Piece], art: f64) -> Vec<Vec<f64>> {
    let near: Vec<Vec<usize>> = (0..g.nx)
        .map(|i| {
            let x = g.at(i, 0)[0];
            pieces.iter().enumerate().filter(|(_, p)| { let (a, b) = p.metal.x_span(); x >= a - CAP && x <= b + CAP }).map(|(k, _)| k).collect()
        })
        .collect();
    let mut fields = vec![vec![0.0; g.nx * g.ny]; LEVELS - 1];
    let rows: Vec<usize> = (0..g.ny).collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let chunk = g.ny.div_ceil(threads);
    let done: Vec<Vec<(usize, Vec<[f64; LEVELS - 1]>)>> = std::thread::scope(|s| {
        let handles: Vec<_> = rows
            .chunks(chunk)
            .map(|js| {
                let near = &near;
                s.spawn(move || {
                    js.iter()
                        .map(|&j| {
                            let row = (0..g.nx)
                                .map(|i| {
                                    let q = g.at(i, j);
                                    let mut v = [CAP; LEVELS - 1];
                                    v[0] = art - q[1].abs();
                                    for &k in &near[i] {
                                        let p = &pieces[k];
                                        v[p.level - 1] = v[p.level - 1].min(p.metal.sd(q));
                                    }
                                    if i == 0 || j == 0 || i == g.nx - 1 || j == g.ny - 1 {
                                        v[0] = -1.0;
                                    }
                                    v
                                })
                                .collect();
                            (j, row)
                        })
                        .collect()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("field thread")).collect()
    });
    for (j, row) in done.into_iter().flatten() {
        for (i, v) in row.into_iter().enumerate() {
            for l in 0..LEVELS - 1 {
                fields[l][j * g.nx + i] = v[l];
            }
        }
    }
    fields
}

/// Components of the grid nodes where `field` is positive, four-connected; 0 marks the rest.
fn components(g: &Grid, field: &[f64]) -> (Vec<u32>, u32) {
    let mut label = vec![0u32; field.len()];
    let mut next = 0u32;
    let mut stack = Vec::new();
    for start in 0..field.len() {
        if field[start] <= 0.0 || label[start] != 0 {
            continue;
        }
        next += 1;
        label[start] = next;
        stack.push(start);
        while let Some(k) = stack.pop() {
            let (i, j) = (k % g.nx, k / g.nx);
            let mut visit = |n: usize| {
                if field[n] > 0.0 && label[n] == 0 {
                    label[n] = next;
                    stack.push(n);
                }
            };
            if i > 0 { visit(k - 1) }
            if i + 1 < g.nx { visit(k + 1) }
            if j > 0 { visit(k - g.nx) }
            if j + 1 < g.ny { visit(k + g.nx) }
        }
    }
    (label, next)
}

/// The component of the positive node nearest a contour's point.
fn label_at(g: &Grid, field: &[f64], label: &[u32], p: V) -> u32 {
    let i = (((p[0] - g.x0) / g.step).floor().max(0.0) as usize).min(g.nx - 2);
    let j = (((p[1] - g.y0) / g.step).floor().max(0.0) as usize).min(g.ny - 2);
    [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)].into_iter().map(|(i, j)| j * g.nx + i).filter(|k| field[*k] > 0.0).map(|k| label[k]).next().unwrap_or(0)
}

fn outline_of(pts: &[V]) -> Vec<V> {
    let mut tol = 0.001;
    let mut outline = densify(&simplify(pts, tol), 0.1);
    while outline.len() > 1000 {
        tol *= 1.4;
        outline = densify(&simplify(pts, tol), 0.1);
    }
    outline
}

pub fn layout(f: Facade, s: &Settings) -> Result<Layout> {
    let c = f.circumference;
    let mut openings = Vec::new();
    for sec in &f.sections {
        let (wx0, wx1) = (sec.x0 - s.margin, sec.x1 + s.margin);
        let mut near = Vec::new();
        for p in &f.pieces {
            let (a, b) = p.metal.x_span();
            for k in [-1.0, 0.0, 1.0] {
                if b + k * c >= wx0 - CAP && a + k * c <= wx1 + CAP {
                    near.push(Piece { level: p.level, metal: if k == 0.0 { p.metal.clone() } else { p.metal.shifted(k * c) } });
                }
            }
        }
        let g = Grid { x0: wx0, y0: -f.art - 0.15, step: s.step, nx: ((wx1 - wx0) / s.step).ceil() as usize + 1, ny: ((2.0 * f.art + 0.3) / s.step).ceil() as usize + 1 };
        let fields = level_fields(&g, &near, f.art);
        let mut base = fields[0].clone();
        let mut counts = BTreeMap::<usize, usize>::new();
        for lv in 1..LEVELS {
            let chain: Vec<f64> = if lv == 1 { base.clone() } else { base.iter().zip(&fields[lv - 1]).map(|(a, b)| a.min(*b)).collect() };
            let pocket = opened(&g, &chain, s.corner_r);
            // A pocket holding no metal of the next level is left to the deeper pocket inside it, so its wall is cut once.
            let (label, n_labels) = components(&g, &pocket);
            let mut holds = vec![lv + 1 == LEVELS; n_labels as usize + 1];
            if lv + 1 < LEVELS {
                for k in 0..pocket.len() {
                    if label[k] != 0 && fields[lv][k] <= 0.0 && pocket[k] - s.eps > 0.0 {
                        holds[label[k] as usize] = true;
                    }
                }
            }
            // The next level stands inside this one by `eps` only where this one's wall is cut.
            base = pocket.iter().zip(&label).map(|(v, l)| if *l != 0 && holds[*l as usize] { v - s.eps } else { *v }).collect();
            for pts in contours(&g, &pocket) {
                let a = area(&pts);
                if a < 0.0 {
                    ensure!(a > -5e-3, "{}: metal stands loose in a level-{lv} pocket near ({:.2}, {:.2}), {:.4} mm2", sec.name, pts[0][0], pts[0][1], -a);
                    continue;
                }
                if a < 0.012 || !holds[label_at(&g, &pocket, &label, pts[0]) as usize] {
                    continue;
                }
                let (lo, hi) = bounds(&pts);
                let at = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])];
                if at[0] < sec.x0 || at[0] >= sec.x1 {
                    continue;
                }
                ensure!(lo[0] > g.x0 + 0.05 && hi[0] < g.x1() - 0.05, "{}: a level-{lv} pocket runs off the sampled window, {lo:.2?} to {hi:.2?}", sec.name);
                let n = counts.entry(lv + 1).or_default();
                *n += 1;
                openings.push(Opening { name: format!("{}, {} {n}", sec.name, FLOOR_NAMES[lv + 1]), floor: lv + 1, outline: outline_of(&pts), at });
            }
        }
    }
    let mut raised = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for o in &f.ornaments {
        let (x0, x1) = o.shape.x_span();
        let xm = (0.5 * (x0 + x1)).rem_euclid(c);
        let sec = f.sections.iter().find(|s| (xm >= s.x0 && xm < s.x1) || (xm - c >= s.x0 && xm - c < s.x1)).map(|s| s.name.clone()).unwrap_or_default();
        let step = 0.004;
        let g = Grid { x0: x0 - 0.06, y0: -f.art - 0.2, step, nx: ((x1 - x0 + 0.12) / step).ceil() as usize + 1, ny: ((2.0 * f.art + 0.4) / step).ceil() as usize + 1 };
        let field: Vec<f64> = (0..g.ny).flat_map(|j| (0..g.nx).map(move |i| (i, j))).map(|(i, j)| -o.shape.sd(g.at(i, j))).collect();
        let loops = contours(&g, &field);
        ensure!(loops.len() == 1, "{sec}: an ornament is not one simple outline");
        let pts = &loops[0];
        let (lo, hi) = bounds(pts);
        let key = format!("{sec}, {}", o.name.to_lowercase());
        let n = counts.entry(key.clone()).or_default();
        *n += 1;
        raised.push(Raised { name: format!("{key} {n}"), top: o.top, floor: o.floor, outline: outline_of(pts), at: [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])] });
    }
    Ok(Layout { facade: f, openings, raised })
}

/// Patina tones by level, rails first.
const TONES: [[u8; 3]; 6] = [[236, 210, 146], [208, 172, 102], [156, 122, 66], [104, 81, 46], [60, 48, 31], [16, 14, 12]];

/// The façade unrolled between `x_lo` and `x_hi`, in the patina's tones.
pub fn write_png(path: &Path, l: &Layout, half: f64, x_lo: f64, x_hi: f64, px_per_mm: f64) -> Result<()> {
    let c = l.facade.circumference;
    let (w_mm, h_mm) = (x_hi - x_lo, 2.0 * half + 1.0);
    let (w, h) = ((w_mm * px_per_mm).ceil() as u32, (h_mm * px_per_mm).ceil() as u32);
    let mut img = image::RgbImage::from_pixel(w, h, image::Rgb(TONES[0]));
    let to_px = |q: V| -> (f64, f64) { ((q[0] - x_lo) * px_per_mm, (0.5 * h_mm - q[1]) * px_per_mm) };
    let band = ((0.5 * h_mm - half) * px_per_mm) as u32;
    for y in band..(h - band) {
        for x in 0..w {
            img.put_pixel(x, y, image::Rgb(TONES[1]));
        }
    }
    for floor in 2..=LEVELS {
        for o in l.openings.iter().filter(|o| o.floor == floor) {
            for k in [-1.0, 0.0, 1.0] {
                let poly: Vec<(f64, f64)> = o.outline.iter().map(|q| to_px([q[0] + k * c, q[1]])).collect();
                fill(&mut img, &poly, TONES[floor]);
            }
        }
    }
    for r in &l.raised {
        for k in [-1.0, 0.0, 1.0] {
            let poly: Vec<(f64, f64)> = r.outline.iter().map(|q| to_px([q[0] + k * c, q[1]])).collect();
            fill(&mut img, &poly, TONES[r.top]);
        }
    }
    img.save(path)?;
    Ok(())
}

fn fill(img: &mut image::RgbImage, poly: &[(f64, f64)], colour: [u8; 3]) {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let (lo, hi) = poly.iter().fold(((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)), |(lo, hi), q| ((lo.0.min(q.0), lo.1.min(q.1)), (hi.0.max(q.0), hi.1.max(q.1))));
    if hi.0 < 0.0 || lo.0 > w {
        return;
    }
    for y in (lo.1.floor().max(0.0) as u32)..(hi.1.ceil().min(h) as u32) {
        let yc = y as f64 + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a.1 > yc) != (b.1 > yc) {
                xs.push(a.0 + (yc - a.1) * (b.0 - a.0) / (b.1 - a.1));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks(2) {
            if let [x0, x1] = pair {
                for x in (x0.round().max(0.0) as u32)..(x1.round().min(w) as u32) {
                    img.put_pixel(x, y, image::Rgb(colour));
                }
            }
        }
    }
}
